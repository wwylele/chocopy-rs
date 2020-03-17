use super::token::*;
use crate::location::*;
use crate::node::*;
use std::cmp::Ordering;
use std::collections::vec_deque::VecDeque;
use std::future::Future;
use std::pin::Pin;

impl Error {
    pub fn unexpected(token: ComplexToken) -> Error {
        Error::CompilerError(CompilerError {
            base: NodeBase::from_location(token.location),
            message: "unexptected token".to_owned(),
            syntax: true,
        })
    }
}

macro_rules! parse_expr_unary {
    ($name:ident, $parse_next:ident, $operator_token:expr => $operator_str:expr) => {
        async fn $name(
            &mut self,
        ) -> (Option<Expr>, Vec<Error>) {
            let mut errors = vec![];
            let start = self.next_pos().await;

            let token = self.take().await;
            let expr = if token.token == $operator_token {
                let (expr, mut error) = self.$parse_next().await;
                errors.append(&mut error);
                let end = self.prev_pos().unwrap_or(start);
                if let Some(operand) = expr {
                    Expr::UnaryExpr(Box::new(UnaryExpr {
                        base: NodeBase::from_positions(start, end),
                        operator: $operator_str,
                        operand,
                    }))
                } else {
                    return (None, errors);
                }
            } else {
                self.push_back(token);
                let (expr, mut error) = self.$parse_next().await;
                errors.append(&mut error);
                if let Some(expr) = expr {
                    expr
                } else {
                    return (None, errors);
                }
            };
            (Some(expr), errors)
        }
    };
}

macro_rules! parse_expr_binary {
    ($name:ident, $parse_next:ident, $($operator_token:pat => $operator_str:expr),*) => {
        async fn $name(
            &mut self,
        ) -> (Option<Expr>, Vec<Error>) {
            let mut errors = vec![];
            let start = self.next_pos().await;

            let (expr, mut error) = self.$parse_next().await;
            errors.append(&mut error);
            let mut expr = if let Some(expr) = expr {
                expr
            } else {
                return (None, errors)
            };

            loop {
                let token = self.take().await;
                let operator = match token.token {
                    $( $operator_token => $operator_str, )*
                    _ => {
                        self.push_back(token);
                        break;
                    }
                };

                let (right, mut error) = self.$parse_next().await;
                errors.append(&mut error);
                let right = if let Some(right) = right {
                    right
                } else {
                    return (None, errors);
                };

                let end = self.prev_pos().unwrap_or(start);

                expr = Expr::BinaryExpr(Box::new(BinaryExpr{
                    base: NodeBase::from_positions(start, end),
                    left: expr,
                    operator,
                    right
                }))
            }
            (Some(expr), errors)
        }
    };
}

type BoxedFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

struct BufferedReceiver<F> {
    receiver: F,
    buffer: Vec<ComplexToken>,
    prev_pos_buf: VecDeque<Position>,
    eof: Option<ComplexToken>,
}

impl<Ft: Future<Output = ComplexToken>, F: FnMut() -> Ft> BufferedReceiver<F> {
    fn new(receiver: F) -> BufferedReceiver<F> {
        BufferedReceiver {
            receiver,
            buffer: vec![],
            prev_pos_buf: VecDeque::new(),
            eof: None,
        }
    }

    async fn take(&mut self) -> ComplexToken {
        if self.prev_pos_buf.len() > 3 {
            self.prev_pos_buf.pop_front();
        }
        if let Some(token) = self.buffer.pop() {
            self.prev_pos_buf.push_back(token.location.end);
            return token;
        }
        if let Some(token) = self.eof.clone() {
            self.prev_pos_buf.push_back(token.location.end);
            return token;
        }
        let token = (self.receiver)().await;
        self.prev_pos_buf.push_back(token.location.end);
        if token.token == Token::Eof {
            self.eof = Some(token.clone());
        }
        token
    }

    fn push_back(&mut self, v: ComplexToken) {
        self.prev_pos_buf.pop_back();
        self.buffer.push(v);
    }

    // For error recovery. Skip pass the next NEWLINE token,
    // and skip the following INDEND..DEDENT block if any.
    async fn skip_to_next_line(&mut self) {
        loop {
            let token = self.take().await;
            match token.token {
                Token::Eof => {
                    self.push_back(token);
                    return;
                }
                Token::NewLine => break,
                _ => (),
            }
        }
        let token = self.take().await;
        if token.token != Token::Indent {
            self.push_back(token);
            return;
        }
        let mut level = 1;
        loop {
            let token = self.take().await;
            match token.token {
                Token::Eof => {
                    self.push_back(token);
                    return;
                }
                Token::Dedent => {
                    level -= 1;
                    if level == 0 {
                        return;
                    }
                }
                Token::Indent => {
                    level += 1;
                }
                _ => (),
            }
        }
    }

    async fn next_pos(&mut self) -> Position {
        let next = self.take().await;
        let start = next.location.start;
        self.push_back(next);
        start
    }

    fn prev_pos(&self) -> Option<Position> {
        self.prev_pos_buf.back().cloned()
    }

    fn parse_expr1<'a>(&'a mut self) -> BoxedFuture<'a, (Option<Expr>, Vec<Error>)> {
        Box::pin(async move {
            let mut errors = vec![];
            let start = self.next_pos().await;

            // Parse "expr if expr else expr"

            let (then_expr, mut error) = self.parse_expr2().await;
            errors.append(&mut error);
            let then_expr = if let Some(then_expr) = then_expr {
                then_expr
            } else {
                return (None, errors);
            };

            let token = self.take().await;
            if token.token != Token::If {
                self.push_back(token);
                return (Some(then_expr), errors);
            }

            let (condition, mut error) = self.parse_expr1().await;
            errors.append(&mut error);
            let condition = if let Some(condition) = condition {
                condition
            } else {
                return (None, errors);
            };

            let token = self.take().await;
            if token.token != Token::Else {
                errors.push(Error::unexpected(token));
                return (None, errors);
            }

            let (else_expr, mut error) = self.parse_expr1().await;
            errors.append(&mut error);
            let else_expr = if let Some(else_expr) = else_expr {
                else_expr
            } else {
                return (None, errors);
            };

            let end = self.prev_pos().unwrap_or(start);

            (
                Some(Expr::IfExpr(Box::new(IfExpr {
                    base: NodeBase::from_positions(start, end),
                    condition,
                    then_expr,
                    else_expr,
                }))),
                errors,
            )
        })
    }

    parse_expr_binary!(parse_expr2, parse_expr3, Token::Or => BinaryOp::Or);
    parse_expr_binary!(parse_expr3, parse_expr4, Token::And => BinaryOp::And);
    parse_expr_unary!(parse_expr4, parse_expr5, Token::Not => UnaryOp::Not);

    async fn parse_expr5(&mut self) -> (Option<Expr>, Vec<Error>) {
        let mut errors = vec![];
        let start = self.next_pos().await;

        let (left, mut error) = self.parse_expr6().await;
        errors.append(&mut error);
        let left = if let Some(left) = left {
            left
        } else {
            return (None, errors);
        };

        let token = self.take().await;
        let operator = match token.token {
            Token::Equal => BinaryOp::Eq,
            Token::NotEqual => BinaryOp::Ne,
            Token::Less => BinaryOp::Lt,
            Token::Greater => BinaryOp::Gt,
            Token::LessEqual => BinaryOp::Le,
            Token::GreaterEqual => BinaryOp::Ge,
            Token::Is => BinaryOp::Is,
            _ => {
                self.push_back(token);
                return (Some(left), errors);
            }
        };

        let (right, mut error) = self.parse_expr6().await;
        errors.append(&mut error);
        let right = if let Some(right) = right {
            right
        } else {
            return (None, errors);
        };

        let end = self.prev_pos().unwrap_or(start);
        (
            Some(Expr::BinaryExpr(Box::new(BinaryExpr {
                base: NodeBase::from_positions(start, end),
                left,
                operator,
                right,
            }))),
            errors,
        )
    }

    parse_expr_binary!(parse_expr6, parse_expr7,
        Token::Plus => BinaryOp::Add,
        Token::Minus => BinaryOp::Sub
    );

    parse_expr_binary!(parse_expr7, parse_expr8,
        Token::Multiply => BinaryOp::Mul,
        Token::Divide => BinaryOp::Div,
        Token::Mod => BinaryOp::Mod
    );

    parse_expr_unary!(parse_expr8, parse_expr9, Token::Minus => UnaryOp::Negative);

    async fn parse_expr9(&mut self) -> (Option<Expr>, Vec<Error>) {
        let mut errors = vec![];
        let start = self.next_pos().await;

        // Parse "expr . id", "expr [ expr ]", "expr ( expr,* )"
        let (expr, mut error) = self.parse_expr10().await;
        errors.append(&mut error);
        let mut expr = if let Some(expr) = expr {
            expr
        } else {
            return (None, errors);
        };

        loop {
            let token = self.take().await;
            match token.token {
                Token::LeftPar => {
                    let mut args = vec![];

                    let token_head = self.take().await;
                    if token_head.token != Token::RightPar {
                        self.push_back(token_head);
                        loop {
                            let (arg, mut error) = self.parse_expr1().await;
                            errors.append(&mut error);
                            if let Some(arg) = arg {
                                args.push(arg);
                            }
                            let token = self.take().await;
                            match token.token {
                                Token::Comma => (),
                                Token::RightPar => break,
                                _ => {
                                    errors.push(Error::unexpected(token));
                                    return (None, errors);
                                }
                            }
                        }
                    }

                    let end = self.prev_pos().unwrap_or(start);
                    let base = NodeBase::from_positions(start, end);
                    expr = match expr.content {
                        ExprContent::Identifier(function) => Expr::CallExpr(CallExpr {
                            base,
                            function: FuncId::Identifier(FuncIdentifier {
                                inferred_type: None,
                                base: function.base,
                                name: function.name,
                            }),
                            args,
                        }),
                        ExprContent::MemberExpr(method) => {
                            Expr::MethodCallExpr(Box::new(MethodCallExpr {
                                base,
                                method: Method::MemberExpr(TypedMemberExpr {
                                    inferred_type: None,
                                    base: method.base,
                                    object: method.object,
                                    member: method.member,
                                }),
                                args,
                            }))
                        }
                        _ => {
                            errors.push(Error::unexpected(token));
                            return (None, errors);
                        }
                    }
                }
                Token::LeftSquare => {
                    let (index, mut error) = self.parse_expr1().await;
                    errors.append(&mut error);
                    let index = if let Some(index) = index {
                        index
                    } else {
                        return (None, errors);
                    };

                    let token = self.take().await;
                    if token.token != Token::RightSquare {
                        errors.push(Error::unexpected(token));
                        return (None, errors);
                    }

                    let end = self.prev_pos().unwrap_or(start);

                    expr = Expr::IndexExpr(Box::new(IndexExpr {
                        base: NodeBase::from_positions(start, end),
                        list: expr,
                        index,
                    }));
                }
                Token::Dot => {
                    let token = self.take().await;
                    let member = if let Token::Identifier(name) = token.token {
                        Id::Identifier(Identifier {
                            base: NodeBase::from_location(token.location),
                            name,
                        })
                    } else {
                        errors.push(Error::unexpected(token));
                        return (None, errors);
                    };

                    let end = self.prev_pos().unwrap_or(start);

                    expr = Expr::MemberExpr(Box::new(MemberExpr {
                        base: NodeBase::from_positions(start, end),
                        object: expr,
                        member,
                    }));
                }
                _ => {
                    self.push_back(token);
                    break;
                }
            }
        }

        (Some(expr), errors)
    }

    async fn parse_expr10(&mut self) -> (Option<Expr>, Vec<Error>) {
        let mut errors = vec![];
        let start = self.next_pos().await;

        // Parse atomic expression, (), and []
        let token = self.take().await;
        let end = self.prev_pos().unwrap_or(start);
        let base = NodeBase::from_positions(start, end);
        let expr = match token.token {
            Token::Identifier(name) => Expr::Identifier(Identifier { base, name }),
            Token::None => Expr::NoneLiteral(NoneLiteral { base }),
            Token::True => Expr::BooleanLiteral(BooleanLiteral { base, value: true }),
            Token::False => Expr::BooleanLiteral(BooleanLiteral { base, value: false }),
            Token::Number(value) => Expr::IntegerLiteral(IntegerLiteral { base, value }),
            Token::StringLiteral(value) | Token::IdString(value) => {
                Expr::StringLiteral(StringLiteral { base, value })
            }
            Token::LeftPar => {
                let (expr, mut error) = self.parse_expr1().await;
                errors.append(&mut error);
                if let Some(expr) = expr {
                    let token = self.take().await;
                    if token.token != Token::RightPar {
                        errors.push(Error::unexpected(token));
                        return (None, errors);
                    }
                    expr
                } else {
                    return (None, errors);
                }
            }
            Token::LeftSquare => {
                let mut elements = vec![];

                let token = self.take().await;
                if token.token != Token::RightSquare {
                    self.push_back(token);
                    loop {
                        let (element, mut error) = self.parse_expr1().await;
                        errors.append(&mut error);
                        if let Some(element) = element {
                            elements.push(element);
                        }
                        let token = self.take().await;
                        match token.token {
                            Token::Comma => (),
                            Token::RightSquare => break,
                            _ => {
                                errors.push(Error::unexpected(token));
                                return (None, errors);
                            }
                        }
                    }
                }

                let end = self.prev_pos().unwrap_or(start);
                let base = NodeBase::from_positions(start, end);
                Expr::ListExpr(ListExpr { base, elements })
            }
            _ => {
                errors.push(Error::unexpected(token));
                return (None, errors);
            }
        };

        (Some(expr), errors)
    }

    async fn parse_assign_or_expr_stmt(&mut self) -> (Option<Stmt>, Vec<Error>) {
        let mut expr_list = vec![];
        let mut errors = vec![];
        let start = self.next_pos().await;
        let mut end;
        loop {
            let (expr, mut error) = self.parse_expr1().await;
            errors.append(&mut error);
            if let Some(expr) = expr {
                expr_list.push(expr);
            } else {
                return (None, errors);
            }

            end = self.prev_pos().unwrap_or(start);
            let token = self.take().await;
            match token.token {
                Token::Assign => match expr_list.last().map(|e| &e.content) {
                    Some(ExprContent::Identifier(_))
                    | Some(ExprContent::MemberExpr(_))
                    | Some(ExprContent::IndexExpr(_)) => (),
                    _ => {
                        errors.push(Error::unexpected(token));
                        return (None, errors);
                    }
                },
                Token::NewLine => break,
                _ => {
                    errors.push(Error::unexpected(token));
                    return (None, errors);
                }
            }
        }
        let base = NodeBase::from_positions(start, end);
        let stmt = match expr_list.len().cmp(&1) {
            Ordering::Equal => Some(Stmt::ExprStmt(ExprStmt {
                base,
                expr: expr_list.pop().unwrap(),
            })),
            Ordering::Greater => {
                let value = expr_list.pop().unwrap();
                Some(Stmt::AssignStmt(AssignStmt {
                    base,
                    targets: expr_list,
                    value,
                }))
            }
            _ => None,
        };
        (stmt, errors)
    }

    async fn parse_return(&mut self) -> (Option<ReturnStmt>, Vec<Error>) {
        let mut errors = vec![];
        let start = self.next_pos().await;

        let token = self.take().await;
        if token.token != Token::Return {
            errors.push(Error::unexpected(token));
            return (None, errors);
        }

        let token = self.take().await;
        let value = if token.token == Token::NewLine {
            self.push_back(token);
            None
        } else {
            self.push_back(token);
            let (expr, mut error) = self.parse_expr1().await;
            errors.append(&mut error);
            if let Some(expr) = expr {
                Some(expr)
            } else {
                return (None, errors);
            }
        };

        let end = self.prev_pos().unwrap_or(start);

        let token = self.take().await;
        if token.token != Token::NewLine {
            errors.push(Error::unexpected(token));
            return (None, errors);
        }

        (
            Some(ReturnStmt {
                base: NodeBase::from_positions(start, end),
                value,
            }),
            errors,
        )
    }

    async fn parse_block(&mut self) -> (Option<Vec<Stmt>>, Vec<Error>) {
        let mut errors = vec![];

        let token = self.take().await;
        if token.token != Token::Colon {
            errors.push(Error::unexpected(token));
            return (None, errors);
        }

        let token = self.take().await;
        if token.token != Token::NewLine {
            errors.push(Error::unexpected(token));
            return (None, errors);
        }

        let token = self.take().await;
        if token.token != Token::Indent {
            errors.push(Error::unexpected(token));
            return (None, errors);
        }

        let (body, mut error) = self.parse_stmt_list().await;
        errors.append(&mut error);

        let token = self.take().await;
        if token.token != Token::Dedent {
            errors.push(Error::unexpected(token));
            return (None, errors);
        }

        (Some(body), errors)
    }

    async fn parse_while(&mut self) -> (Option<WhileStmt>, Vec<Error>) {
        let mut errors = vec![];
        let start = self.next_pos().await;

        let token = self.take().await;
        if token.token != Token::While {
            errors.push(Error::unexpected(token));
            return (None, errors);
        }

        let (expr, mut error) = self.parse_expr1().await;
        errors.append(&mut error);
        let condition = if let Some(expr) = expr {
            expr
        } else {
            return (None, errors);
        };

        let (body, mut error) = self.parse_block().await;
        errors.append(&mut error);
        let body = if let Some(body) = body {
            body
        } else {
            return (None, errors);
        };

        let end = self.prev_pos().unwrap_or(start);
        (
            Some(WhileStmt {
                base: NodeBase::from_positions(start, end),
                condition,
                body,
            }),
            errors,
        )
    }

    async fn parse_for(&mut self) -> (Option<ForStmt>, Vec<Error>) {
        let mut errors = vec![];
        let start = self.next_pos().await;

        let token = self.take().await;
        if token.token != Token::For {
            errors.push(Error::unexpected(token));
            return (None, errors);
        }

        let token = self.take().await;
        let identifier = if let Token::Identifier(name) = token.token {
            TypedId::Identifier(TypedIdentifier {
                inferred_type: None,
                base: NodeBase::from_location(token.location),
                name,
            })
        } else {
            errors.push(Error::unexpected(token));
            return (None, errors);
        };

        let token = self.take().await;
        if token.token != Token::In {
            errors.push(Error::unexpected(token));
            return (None, errors);
        }

        let (expr, mut error) = self.parse_expr1().await;
        errors.append(&mut error);
        let iterable = if let Some(expr) = expr {
            expr
        } else {
            return (None, errors);
        };

        let (body, mut error) = self.parse_block().await;
        errors.append(&mut error);
        let body = if let Some(body) = body {
            body
        } else {
            return (None, errors);
        };

        let end = self.prev_pos().unwrap_or(start);
        (
            Some(ForStmt {
                base: NodeBase::from_positions(start, end),
                identifier,
                iterable,
                body,
            }),
            errors,
        )
    }

    fn parse_if<'a>(&'a mut self) -> BoxedFuture<'a, (Option<IfStmt>, Vec<Error>)> {
        Box::pin(async move {
            let mut errors = vec![];
            let start = self.next_pos().await;

            let token = self.take().await;
            if token.token != Token::If && token.token != Token::Elif {
                errors.push(Error::unexpected(token));
                return (None, errors);
            }

            let (expr, mut error) = self.parse_expr1().await;
            errors.append(&mut error);
            let condition = if let Some(expr) = expr {
                expr
            } else {
                return (None, errors);
            };

            let (then_body, mut error) = self.parse_block().await;
            errors.append(&mut error);
            let then_body = if let Some(then_body) = then_body {
                then_body
            } else {
                return (None, errors);
            };

            let token = self.take().await;
            let else_body = match token.token {
                Token::Else => {
                    let (else_body, mut error) = self.parse_block().await;
                    errors.append(&mut error);
                    if let Some(else_body) = else_body {
                        else_body
                    } else {
                        return (None, errors);
                    }
                }
                Token::Elif => {
                    self.push_back(token);
                    let (else_body, mut error) = self.parse_if().await;
                    errors.append(&mut error);
                    if let Some(else_body) = else_body {
                        vec![Stmt::IfStmt(else_body)]
                    } else {
                        return (None, errors);
                    }
                }
                _ => {
                    self.push_back(token);
                    vec![]
                }
            };

            let end = self.prev_pos().unwrap_or(start);
            (
                Some(IfStmt {
                    base: NodeBase::from_positions(start, end),
                    condition,
                    then_body,
                    else_body,
                }),
                errors,
            )
        })
    }

    fn parse_stmt_list<'a>(&'a mut self) -> BoxedFuture<'a, (Vec<Stmt>, Vec<Error>)> {
        Box::pin(async move {
            let mut stmt_list = vec![];
            let mut errors = vec![];

            loop {
                let token = self.take().await;
                match token.token {
                    Token::Eof | Token::Dedent => {
                        self.push_back(token);
                        break;
                    }
                    Token::Pass => {
                        let token = self.take().await;
                        if token.token != Token::NewLine {
                            errors.push(Error::unexpected(token));
                            self.skip_to_next_line().await;
                        }
                    }
                    Token::Return => {
                        self.push_back(token);
                        let (return_stmt, mut error) = self.parse_return().await;
                        errors.append(&mut error);
                        if let Some(return_stmt) = return_stmt {
                            stmt_list.push(Stmt::ReturnStmt(return_stmt));
                        } else {
                            self.skip_to_next_line().await;
                        }
                    }
                    Token::While => {
                        self.push_back(token);
                        let (while_stmt, mut error) = self.parse_while().await;
                        errors.append(&mut error);
                        if let Some(while_stmt) = while_stmt {
                            stmt_list.push(Stmt::WhileStmt(while_stmt));
                        } else {
                            self.skip_to_next_line().await;
                        }
                    }
                    Token::For => {
                        self.push_back(token);
                        let (for_stmt, mut error) = self.parse_for().await;
                        errors.append(&mut error);
                        if let Some(for_stmt) = for_stmt {
                            stmt_list.push(Stmt::ForStmt(for_stmt));
                        } else {
                            self.skip_to_next_line().await;
                        }
                    }
                    Token::If => {
                        self.push_back(token);
                        let (if_stmt, mut error) = self.parse_if().await;
                        errors.append(&mut error);
                        if let Some(if_stmt) = if_stmt {
                            stmt_list.push(Stmt::IfStmt(if_stmt));
                        } else {
                            self.skip_to_next_line().await;
                        }
                    }
                    _ => {
                        self.push_back(token);
                        let (stmt, mut error) = self.parse_assign_or_expr_stmt().await;
                        errors.append(&mut error);
                        if let Some(stmt) = stmt {
                            stmt_list.push(stmt);
                        } else {
                            self.skip_to_next_line().await;
                        }
                    }
                }
            }

            (stmt_list, errors)
        })
    }

    async fn parse_decl_in_class(&mut self) -> (Option<Vec<Declaration>>, Vec<Error>) {
        let mut declarations = vec![];
        let mut errors = vec![];

        let token = self.take().await;
        if token.token == Token::Pass {
            let token = self.take().await;
            if token.token != Token::NewLine {
                errors.push(Error::unexpected(token));
                return (None, errors);
            }
        } else {
            // Parse "[func_def|var_def]* }"
            self.push_back(token);

            loop {
                let token = self.take().await;
                match token.token {
                    Token::Dedent => {
                        self.push_back(token);
                        break;
                    }
                    Token::Def => {
                        self.push_back(token);
                        let (func_def, mut error) = self.parse_func_def().await;
                        if let Some(func_def) = func_def {
                            declarations.push(Declaration::FuncDef(func_def));
                        } else {
                            self.skip_to_next_line().await;
                        }
                        errors.append(&mut error);
                    }
                    _ => {
                        self.push_back(token);
                        let (var_def, mut error) = self.parse_var_def().await;
                        if let Some(var_def) = var_def {
                            declarations.push(Declaration::VarDef(var_def));
                        } else {
                            self.skip_to_next_line().await;
                        }
                        errors.append(&mut error);
                    }
                }
            }
        }
        (Some(declarations), errors)
    }

    async fn parse_class_def(&mut self) -> (Option<ClassDef>, Vec<Error>) {
        let mut errors = vec![];
        let start = self.next_pos().await;

        // Parse "class ID ( ID ) : \n {"
        let token = self.take().await;
        if token.token != Token::Class {
            errors.push(Error::unexpected(token));
            return (None, errors);
        }

        let token = self.take().await;
        let name = if let Token::Identifier(name) = token.token {
            Id::Identifier(Identifier {
                base: NodeBase::from_location(token.location),
                name,
            })
        } else {
            errors.push(Error::unexpected(token));
            return (None, errors);
        };

        let token = self.take().await;
        if token.token != Token::LeftPar {
            errors.push(Error::unexpected(token));
            return (None, errors);
        }

        let token = self.take().await;
        let super_class = if let Token::Identifier(name) = token.token {
            Id::Identifier(Identifier {
                base: NodeBase::from_location(token.location),
                name,
            })
        } else {
            errors.push(Error::unexpected(token));
            return (None, errors);
        };

        let token = self.take().await;
        if token.token != Token::RightPar {
            errors.push(Error::unexpected(token));
            return (None, errors);
        }

        let token = self.take().await;
        if token.token != Token::Colon {
            errors.push(Error::unexpected(token));
            return (None, errors);
        }

        let token = self.take().await;
        if token.token != Token::NewLine {
            errors.push(Error::unexpected(token));
            return (None, errors);
        }

        let token = self.take().await;
        if token.token != Token::Indent {
            errors.push(Error::unexpected(token));
            return (None, errors);
        }

        // Parse body
        let (declarations, mut error) = self.parse_decl_in_class().await;
        errors.append(&mut error);
        let declarations = if let Some(declarations) = declarations {
            declarations
        } else {
            return (None, errors);
        };

        // end at NEWLINE, excluding DEDENT
        let end = self.prev_pos().unwrap_or(start);

        let token = self.take().await;
        if token.token != Token::Dedent {
            errors.push(Error::unexpected(token));
            return (None, errors);
        }

        (
            Some(ClassDef {
                base: NodeBase::from_positions(start, end),
                name,
                super_class,
                declarations,
            }),
            errors,
        )
    }

    async fn parse_decl_in_func(&mut self) -> (Option<Vec<Declaration>>, Vec<Error>) {
        let mut declarations = vec![];
        let mut errors = vec![];

        loop {
            let head = self.take().await;
            match head.token {
                Token::Def => {
                    self.push_back(head);
                    let (func_def, mut error) = self.parse_func_def().await;
                    if let Some(func_def) = func_def {
                        declarations.push(Declaration::FuncDef(func_def));
                    } else {
                        self.skip_to_next_line().await;
                    }
                    errors.append(&mut error);
                }
                scope @ Token::Global | scope @ Token::Nonlocal => {
                    let start = head.location.start;
                    let token = self.take().await;
                    let variable = if let Token::Identifier(name) = token.token {
                        Id::Identifier(Identifier {
                            base: NodeBase::from_location(token.location),
                            name,
                        })
                    } else {
                        errors.push(Error::unexpected(token));
                        self.skip_to_next_line().await;
                        continue;
                    };

                    let end = self.prev_pos().unwrap_or(start);

                    let token = self.take().await;
                    if token.token != Token::NewLine {
                        errors.push(Error::unexpected(token));
                        self.skip_to_next_line().await;
                        continue;
                    }

                    let base = NodeBase::from_positions(start, end);

                    let declaration = if scope == Token::Global {
                        Declaration::GlobalDecl(GlobalDecl { base, variable })
                    } else {
                        Declaration::NonLocalDecl(NonLocalDecl { base, variable })
                    };

                    declarations.push(declaration);
                }
                _ => {
                    let second = self.take().await;
                    match second.token {
                        Token::Colon => {
                            self.push_back(second);
                            self.push_back(head);
                            let (var_def, mut error) = self.parse_var_def().await;
                            if let Some(var_def) = var_def {
                                declarations.push(Declaration::VarDef(var_def));
                            } else {
                                self.skip_to_next_line().await;
                            }
                            errors.append(&mut error);
                        }
                        _ => {
                            self.push_back(second);
                            self.push_back(head);
                            break;
                        }
                    }
                }
            }
        }

        (Some(declarations), errors)
    }

    fn parse_func_def<'a>(&'a mut self) -> BoxedFuture<'a, (Option<FuncDef>, Vec<Error>)> {
        Box::pin(async move {
            let mut errors = vec![];
            let start = self.next_pos().await;

            // Parse "def ID ("
            let token = self.take().await;
            if token.token != Token::Def {
                errors.push(Error::unexpected(token));
                return (None, errors);
            }

            let token = self.take().await;
            let name = if let Token::Identifier(name) = token.token {
                Id::Identifier(Identifier {
                    base: NodeBase::from_location(token.location),
                    name,
                })
            } else {
                errors.push(Error::unexpected(token));
                return (None, errors);
            };

            let token = self.take().await;
            if token.token != Token::LeftPar {
                errors.push(Error::unexpected(token));
                return (None, errors);
            }

            // Parse "typed_var,* )"
            let token = self.take().await;
            let mut params = vec![];
            if token.token != Token::RightPar {
                self.push_back(token);
                loop {
                    let (typed_var, mut error) = self.parse_typed_var().await;
                    errors.append(&mut error);
                    let typed_var = if let Some(typed_var) = typed_var {
                        typed_var
                    } else {
                        return (None, errors);
                    };
                    params.push(Tv::TypedVar(typed_var));

                    let token = self.take().await;
                    match token.token {
                        Token::Comma => (),
                        Token::RightPar => break,
                        _ => {
                            errors.push(Error::unexpected(token));
                            return (None, errors);
                        }
                    }
                }
            }

            // Parse `-> type`? : \n {
            let token = self.take().await;
            let return_type = match token.token {
                Token::Colon => TypeAnnotation::ClassType(ClassType {
                    base: NodeBase::from_location(token.location),
                    class_name: "<None>".to_owned(),
                }),
                Token::Arrow => {
                    let (return_type, mut error) = self.parse_type_annotation().await;
                    errors.append(&mut error);
                    let return_type = if let Some(return_type) = return_type {
                        return_type
                    } else {
                        return (None, errors);
                    };

                    let token = self.take().await;
                    if token.token != Token::Colon {
                        errors.push(Error::unexpected(token));
                        return (None, errors);
                    }

                    return_type
                }
                _ => {
                    errors.push(Error::unexpected(token));
                    return (None, errors);
                }
            };

            let token = self.take().await;
            if token.token != Token::NewLine {
                errors.push(Error::unexpected(token));
                return (None, errors);
            }

            let token = self.take().await;
            if token.token != Token::Indent {
                errors.push(Error::unexpected(token));
                return (None, errors);
            }

            // Parse declarations
            let (declarations, mut error) = self.parse_decl_in_func().await;
            errors.append(&mut error);
            let declarations = if let Some(declarations) = declarations {
                declarations
            } else {
                return (None, errors);
            };

            // Parse statements
            let (stmt_list, mut error) = self.parse_stmt_list().await;
            errors.append(&mut error);
            let statements = stmt_list;

            let end = self.prev_pos().unwrap_or(start); // exludes DEDENT

            let token = self.take().await;
            if token.token != Token::Dedent {
                errors.push(Error::unexpected(token));
                return (None, errors);
            }

            (
                Some(FuncDef {
                    base: NodeBase::from_positions(start, end),
                    name,
                    params,
                    return_type,
                    declarations,
                    statements,
                }),
                errors,
            )
        })
    }
    async fn parse_var_def(&mut self) -> (Option<VarDef>, Vec<Error>) {
        let mut errors = vec![];
        let start = self.next_pos().await;

        // Parse "typed_var = literal \n"
        let (typed_var, mut error) = self.parse_typed_var().await;
        errors.append(&mut error);
        let typed_var = if let Some(typed_var) = typed_var {
            typed_var
        } else {
            return (None, errors);
        };

        let token = self.take().await;
        if token.token != Token::Assign {
            errors.push(Error::unexpected(token));
            return (None, errors);
        }

        let token = self.take().await;
        let base = NodeBase::from_location(token.location);
        let value = match token.token {
            Token::None => Literal::NoneLiteral(NoneLiteral { base }),
            Token::True => Literal::BooleanLiteral(BooleanLiteral { base, value: true }),
            Token::False => Literal::BooleanLiteral(BooleanLiteral { base, value: false }),
            Token::Number(value) => Literal::IntegerLiteral(IntegerLiteral { base, value }),
            Token::StringLiteral(value) | Token::IdString(value) => {
                Literal::StringLiteral(StringLiteral { base, value })
            }
            _ => {
                errors.push(Error::unexpected(token));
                return (None, errors);
            }
        };

        // end excludes NEWLINE
        let end = self.prev_pos().unwrap_or(start);

        let token = self.take().await;
        if token.token != Token::NewLine {
            errors.push(Error::unexpected(token));
            return (None, errors);
        }

        (
            Some(VarDef {
                base: NodeBase::from_positions(start, end),
                var: Tv::TypedVar(typed_var),
                value,
            }),
            errors,
        )
    }

    fn parse_type_annotation<'a>(
        &'a mut self,
    ) -> BoxedFuture<'a, (Option<TypeAnnotation>, Vec<Error>)> {
        Box::pin(async move {
            let mut errors = vec![];
            let start = self.next_pos().await;

            let token = self.take().await;
            match token.token {
                Token::Identifier(class_name) | Token::IdString(class_name) => {
                    let end = self.prev_pos().unwrap_or(start);
                    (
                        Some(TypeAnnotation::ClassType(ClassType {
                            base: NodeBase::from_positions(start, end),
                            class_name,
                        })),
                        errors,
                    )
                }
                Token::LeftSquare => {
                    let (element_type, mut error) = self.parse_type_annotation().await;
                    errors.append(&mut error);
                    let element_type = if let Some(element_type) = element_type {
                        element_type
                    } else {
                        return (None, errors);
                    };

                    let token = self.take().await;
                    if token.token != Token::RightSquare {
                        errors.push(Error::unexpected(token));
                        return (None, errors);
                    }

                    let end = self.prev_pos().unwrap_or(start);
                    (
                        Some(TypeAnnotation::ListType(Box::new(ListType {
                            base: NodeBase::from_positions(start, end),
                            element_type,
                        }))),
                        errors,
                    )
                }
                _ => {
                    errors.push(Error::unexpected(token));
                    (None, errors)
                }
            }
        })
    }

    async fn parse_typed_var(&mut self) -> (Option<TypedVar>, Vec<Error>) {
        let mut errors = vec![];
        let start = self.next_pos().await;

        // Parse "ID : type"
        let token = self.take().await;
        let identifier = if let Token::Identifier(name) = token.token {
            Id::Identifier(Identifier {
                base: NodeBase::from_location(token.location),
                name,
            })
        } else {
            errors.push(Error::unexpected(token));
            return (None, errors);
        };

        let token = self.take().await;
        if token.token != Token::Colon {
            errors.push(Error::unexpected(token));
            return (None, errors);
        }

        let (type_, mut error) = self.parse_type_annotation().await;
        errors.append(&mut error);
        let type_ = if let Some(type_) = type_ {
            type_
        } else {
            return (None, errors);
        };

        let end = self.prev_pos().unwrap_or(start);

        (
            Some(TypedVar {
                base: NodeBase::from_positions(start, end),
                identifier,
                type_,
            }),
            errors,
        )
    }
}

pub async fn parse<GetTokenFuture: Future<Output = ComplexToken>>(
    get_token: impl FnMut() -> GetTokenFuture,
) -> Ast {
    let mut tokens = BufferedReceiver::new(get_token);

    let mut declarations = vec![];
    let mut statements = None;
    let mut errors = vec![];

    let start = tokens.next_pos().await;
    let mut end = start; // end excludes EOF

    loop {
        let head = tokens.take().await;
        match head.token {
            Token::Eof => break,
            Token::Class => {
                tokens.push_back(head);
                let (class_def, mut error) = tokens.parse_class_def().await;
                if let Some(class_def) = class_def {
                    declarations.push(Declaration::ClassDef(class_def));
                } else {
                    tokens.skip_to_next_line().await;
                }
                errors.append(&mut error);
                end = tokens.prev_pos().unwrap_or(start);
            }
            Token::Def => {
                tokens.push_back(head);
                let (func_def, mut error) = tokens.parse_func_def().await;
                if let Some(func_def) = func_def {
                    declarations.push(Declaration::FuncDef(func_def));
                } else {
                    tokens.skip_to_next_line().await;
                }
                errors.append(&mut error);
                end = tokens.prev_pos().unwrap_or(start);
            }
            _ => {
                let second = tokens.take().await;
                match second.token {
                    Token::Colon => {
                        tokens.push_back(second);
                        tokens.push_back(head);
                        let (var_def, mut error) = tokens.parse_var_def().await;
                        if let Some(var_def) = var_def {
                            declarations.push(Declaration::VarDef(var_def));
                        } else {
                            tokens.skip_to_next_line().await;
                        }
                        errors.append(&mut error);
                        end = tokens.prev_pos().unwrap_or(start);
                    }
                    _ => {
                        tokens.push_back(second);
                        tokens.push_back(head);
                        let (stmt_list, mut error) = tokens.parse_stmt_list().await;
                        errors.append(&mut error);
                        statements = Some(stmt_list);
                        end = tokens.prev_pos().unwrap_or(start);

                        loop {
                            let token = tokens.take().await;
                            if token.token == Token::Eof {
                                break;
                            } else {
                                errors.push(Error::unexpected(token));
                            }
                        }
                        break;
                    }
                }
            }
        }
    }

    let statements = statements.unwrap_or_default();

    Ast::Program(Program {
        base: NodeBase::from_positions(start, end),
        declarations,
        statements,
        errors: ErrorInfo::Errors(Errors {
            base: NodeBase::new(0, 0, 0, 0),
            errors,
        }),
    })
}
