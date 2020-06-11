use super::token::*;
use crate::location::*;
use crate::node::*;
use std::cmp::Ordering;
use std::collections::vec_deque::VecDeque;

fn unexpected(token: ComplexToken) -> CompilerError {
    CompilerError {
        base: NodeBase::from_location(token.location),
        message: "unexptected token".to_owned(),
        syntax: true,
    }
}

macro_rules! parse_expr_unary {
    ($name:ident, $parse_next:ident, $operator_token:expr => $operator_name:expr) => {
        fn $name(&mut self) -> Option<Expr> {
            let start = self.next_pos();

            let token = self.take();
            let expr = if token.token == $operator_token {
                let expr = self.$parse_next()?;

                let end = self.prev_pos().unwrap_or(start);
                Expr::UnaryExpr(Box::new(UnaryExpr {
                    base: NodeBase::from_positions(start, end),
                    operator: $operator_name,
                    operand: expr,
                }))
            } else {
                self.push_back(token);
                self.$parse_next()?
            };
            Some(expr)
        }
    };
}

macro_rules! parse_expr_binary {
    ($name:ident, $parse_next:ident, $($operator_token:pat => $operator_name:expr),*) => {
        fn $name(&mut self) -> Option<Expr> {
            let start = self.next_pos();

            let mut expr = self.$parse_next()?;

            loop {
                let token = self.take();
                let operator = match token.token {
                    $( $operator_token => $operator_name, )*
                    _ => {
                        self.push_back(token);
                        break;
                    }
                };

                let right = self.$parse_next()?;

                let end = self.prev_pos().unwrap_or(start);

                expr = Expr::BinaryExpr(Box::new(BinaryExpr{
                    base: NodeBase::from_positions(start, end),
                    left: expr,
                    operator,
                    right
                }))
            }
            Some(expr)
        }
    };
}

struct Parser<F> {
    receiver: F,
    buffer: Vec<ComplexToken>,
    prev_pos_buf: VecDeque<Position>,
    eof: Option<ComplexToken>,
    errors: Vec<CompilerError>,
}

impl<F: Iterator<Item = ComplexToken>> Parser<F> {
    fn new(receiver: F) -> Parser<F> {
        Parser {
            receiver,
            buffer: vec![],
            prev_pos_buf: VecDeque::new(),
            eof: None,
            errors: vec![],
        }
    }

    fn take(&mut self) -> ComplexToken {
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
        let token = self.receiver.next().unwrap();
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
    fn skip_to_next_line(&mut self) {
        loop {
            let token = self.take();
            match token.token {
                Token::Eof => {
                    self.push_back(token);
                    return;
                }
                Token::NewLine => break,
                _ => (),
            }
        }
        let token = self.take();
        if token.token != Token::Indent {
            self.push_back(token);
            return;
        }
        let mut level = 1;
        loop {
            let token = self.take();
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

    fn next_pos(&mut self) -> Position {
        let next = self.take();
        let start = next.location.start;
        self.push_back(next);
        start
    }

    fn prev_pos(&self) -> Option<Position> {
        self.prev_pos_buf.back().cloned()
    }

    fn eat(&mut self, expected_token: Token) -> Option<()> {
        let token = self.take();
        if token.token != expected_token {
            self.errors.push(unexpected(token));
            return None;
        }
        Some(())
    }

    fn take_id(&mut self) -> Option<Identifier> {
        let token = self.take();
        if let Token::Identifier(name) = token.token {
            Some(Identifier {
                base: NodeBase::from_location(token.location),
                name,
            })
        } else {
            self.errors.push(unexpected(token));
            None
        }
    }

    fn parse_expr1(&mut self) -> Option<Expr> {
        let start = self.next_pos();

        // Parse "expr if expr else expr"

        let then_expr = self.parse_expr2()?;

        let token = self.take();
        if token.token != Token::If {
            self.push_back(token);
            return Some(then_expr);
        }

        let condition = self.parse_expr1()?;

        self.eat(Token::Else)?;

        let else_expr = self.parse_expr1()?;

        let end = self.prev_pos().unwrap_or(start);

        Some(Expr::IfExpr(Box::new(IfExpr {
            base: NodeBase::from_positions(start, end),
            condition,
            then_expr,
            else_expr,
        })))
    }

    parse_expr_binary!(parse_expr2, parse_expr3, Token::Or => BinaryOp::Or);
    parse_expr_binary!(parse_expr3, parse_expr4, Token::And => BinaryOp::And);
    parse_expr_unary!(parse_expr4, parse_expr5, Token::Not => UnaryOp::Not);

    fn parse_expr5(&mut self) -> Option<Expr> {
        let start = self.next_pos();

        let left = self.parse_expr6()?;

        let token = self.take();
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
                return Some(left);
            }
        };

        let right = self.parse_expr6()?;
        let end = self.prev_pos().unwrap_or(start);

        Some(Expr::BinaryExpr(Box::new(BinaryExpr {
            base: NodeBase::from_positions(start, end),
            left,
            operator,
            right,
        })))
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

    fn parse_expr9(&mut self) -> Option<Expr> {
        let start = self.next_pos();

        // Parse "expr . id", "expr [ expr ]", "expr ( expr,* )"
        let mut expr = self.parse_expr10()?;

        loop {
            let token = self.take();
            match token.token {
                Token::LeftPar => {
                    let mut args = vec![];

                    let token_head = self.take();
                    if token_head.token != Token::RightPar {
                        self.push_back(token_head);
                        loop {
                            if let Some(arg) = self.parse_expr1() {
                                args.push(arg);
                            }
                            let token = self.take();
                            match token.token {
                                Token::Comma => (),
                                Token::RightPar => break,
                                _ => {
                                    self.errors.push(unexpected(token));
                                    return None;
                                }
                            }
                        }
                    }

                    let end = self.prev_pos().unwrap_or(start);
                    let base = NodeBase::from_positions(start, end);
                    expr = match expr.content {
                        ExprContent::Variable(function) => Expr::CallExpr(CallExpr {
                            base,
                            function: Function {
                                inferred_type: None,
                                base: function.base,
                                name: function.name,
                            },
                            args,
                        }),
                        ExprContent::MemberExpr(method) => {
                            Expr::MethodCallExpr(Box::new(MethodCallExpr {
                                base,
                                method: Method {
                                    inferred_type: None,
                                    base: method.base,
                                    object: method.object,
                                    member: method.member,
                                },
                                args,
                            }))
                        }
                        _ => {
                            self.errors.push(unexpected(token));
                            return None;
                        }
                    }
                }
                Token::LeftSquare => {
                    let index = self.parse_expr1()?;
                    self.eat(Token::RightSquare)?;
                    let end = self.prev_pos().unwrap_or(start);

                    expr = Expr::IndexExpr(Box::new(IndexExpr {
                        base: NodeBase::from_positions(start, end),
                        list: expr,
                        index,
                    }));
                }
                Token::Dot => {
                    let member = self.take_id()?;
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

        Some(expr)
    }

    fn parse_expr10(&mut self) -> Option<Expr> {
        let start = self.next_pos();

        // Parse atomic expression, (), and []
        let token = self.take();
        let end = self.prev_pos().unwrap_or(start);
        let base = NodeBase::from_positions(start, end);
        let expr = match token.token {
            Token::Identifier(name) => Expr::Variable(Variable { base, name }),
            Token::None => Expr::NoneLiteral(NoneLiteral { base }),
            Token::True => Expr::BooleanLiteral(BooleanLiteral { base, value: true }),
            Token::False => Expr::BooleanLiteral(BooleanLiteral { base, value: false }),
            Token::Number(value) => Expr::IntegerLiteral(IntegerLiteral { base, value }),
            Token::StringLiteral(value) | Token::IdString(value) => {
                Expr::StringLiteral(StringLiteral { base, value })
            }
            Token::LeftPar => {
                let expr = self.parse_expr1()?;
                self.eat(Token::RightPar)?;
                expr
            }
            Token::LeftSquare => {
                let mut elements = vec![];

                let token = self.take();
                if token.token != Token::RightSquare {
                    self.push_back(token);
                    loop {
                        if let Some(element) = self.parse_expr1() {
                            elements.push(element);
                        }
                        let token = self.take();
                        match token.token {
                            Token::Comma => (),
                            Token::RightSquare => break,
                            _ => {
                                self.errors.push(unexpected(token));
                                return None;
                            }
                        }
                    }
                }

                let end = self.prev_pos().unwrap_or(start);
                let base = NodeBase::from_positions(start, end);
                Expr::ListExpr(ListExpr { base, elements })
            }
            _ => {
                self.errors.push(unexpected(token));
                return None;
            }
        };

        Some(expr)
    }

    fn parse_assign_or_expr_stmt(&mut self) -> Option<Stmt> {
        let mut expr_list = vec![];

        let start = self.next_pos();
        let mut end;
        loop {
            expr_list.push(self.parse_expr1()?);

            end = self.prev_pos().unwrap_or(start);
            let token = self.take();
            match token.token {
                Token::Assign => match expr_list.last().map(|e| &e.content) {
                    Some(ExprContent::Variable(_))
                    | Some(ExprContent::MemberExpr(_))
                    | Some(ExprContent::IndexExpr(_)) => (),
                    _ => {
                        self.errors.push(unexpected(token));
                        return None;
                    }
                },
                Token::NewLine => break,
                _ => {
                    self.errors.push(unexpected(token));
                    return None;
                }
            }
        }
        let base = NodeBase::from_positions(start, end);
        match expr_list.len().cmp(&1) {
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
        }
    }

    fn parse_return(&mut self) -> Option<ReturnStmt> {
        let start = self.next_pos();

        self.eat(Token::Return)?;

        let token = self.take();
        let value = if token.token == Token::NewLine {
            self.push_back(token);
            None
        } else {
            self.push_back(token);
            Some(self.parse_expr1()?)
        };

        let end = self.prev_pos().unwrap_or(start);

        self.eat(Token::NewLine)?;

        Some(ReturnStmt {
            base: NodeBase::from_positions(start, end),
            value,
        })
    }

    fn parse_block(&mut self) -> Option<Vec<Stmt>> {
        self.eat(Token::Colon)?;
        self.eat(Token::NewLine)?;
        self.eat(Token::Indent)?;
        let body = self.parse_stmt_list();
        self.eat(Token::Dedent)?;
        Some(body)
    }

    fn parse_while(&mut self) -> Option<WhileStmt> {
        let start = self.next_pos();
        self.eat(Token::While)?;
        let condition = self.parse_expr1()?;
        let body = self.parse_block()?;
        let end = self.prev_pos().unwrap_or(start);
        Some(WhileStmt {
            base: NodeBase::from_positions(start, end),
            condition,
            body,
        })
    }

    fn parse_for(&mut self) -> Option<ForStmt> {
        let start = self.next_pos();

        self.eat(Token::For)?;

        let token = self.take();
        let identifier = if let Token::Identifier(name) = token.token {
            ForTarget {
                inferred_type: None,
                base: NodeBase::from_location(token.location),
                name,
            }
        } else {
            self.errors.push(unexpected(token));
            return None;
        };

        self.eat(Token::In)?;

        let iterable = self.parse_expr1()?;
        let body = self.parse_block()?;

        let end = self.prev_pos().unwrap_or(start);

        Some(ForStmt {
            base: NodeBase::from_positions(start, end),
            identifier,
            iterable,
            body,
        })
    }

    fn parse_if(&mut self) -> Option<IfStmt> {
        let start = self.next_pos();

        let token = self.take();
        if token.token != Token::If && token.token != Token::Elif {
            self.errors.push(unexpected(token));
            return None;
        }

        let condition = self.parse_expr1()?;
        let then_body = self.parse_block()?;

        let token = self.take();
        let else_body = match token.token {
            Token::Else => self.parse_block()?,
            Token::Elif => {
                self.push_back(token);
                vec![Stmt::IfStmt(self.parse_if()?)]
            }
            _ => {
                self.push_back(token);
                vec![]
            }
        };

        let end = self.prev_pos().unwrap_or(start);

        Some(IfStmt {
            base: NodeBase::from_positions(start, end),
            condition,
            then_body,
            else_body,
        })
    }

    fn parse_stmt_list(&mut self) -> Vec<Stmt> {
        let mut stmt_list = vec![];

        loop {
            let token = self.take();
            match token.token {
                Token::Eof | Token::Dedent => {
                    self.push_back(token);
                    break;
                }
                Token::Pass => {
                    let token = self.take();
                    if token.token != Token::NewLine {
                        self.errors.push(unexpected(token));
                        self.skip_to_next_line();
                    }
                }
                Token::Return => {
                    self.push_back(token);
                    if let Some(return_stmt) = self.parse_return() {
                        stmt_list.push(Stmt::ReturnStmt(return_stmt));
                    } else {
                        self.skip_to_next_line();
                    }
                }
                Token::While => {
                    self.push_back(token);
                    if let Some(while_stmt) = self.parse_while() {
                        stmt_list.push(Stmt::WhileStmt(while_stmt));
                    } else {
                        self.skip_to_next_line();
                    }
                }
                Token::For => {
                    self.push_back(token);
                    if let Some(for_stmt) = self.parse_for() {
                        stmt_list.push(Stmt::ForStmt(for_stmt));
                    } else {
                        self.skip_to_next_line();
                    }
                }
                Token::If => {
                    self.push_back(token);
                    if let Some(if_stmt) = self.parse_if() {
                        stmt_list.push(Stmt::IfStmt(if_stmt));
                    } else {
                        self.skip_to_next_line();
                    }
                }
                _ => {
                    self.push_back(token);
                    if let Some(stmt) = self.parse_assign_or_expr_stmt() {
                        stmt_list.push(stmt);
                    } else {
                        self.skip_to_next_line();
                    }
                }
            }
        }

        stmt_list
    }

    fn parse_decl_in_class(&mut self) -> Option<Vec<Declaration>> {
        let mut declarations = vec![];

        let token = self.take();
        if token.token == Token::Pass {
            self.eat(Token::NewLine)?;
        } else {
            // Parse "[func_def|var_def]* }"
            self.push_back(token);

            loop {
                let token = self.take();
                match token.token {
                    Token::Dedent => {
                        self.push_back(token);
                        break;
                    }
                    Token::Def => {
                        self.push_back(token);
                        if let Some(func_def) = self.parse_func_def() {
                            declarations.push(Declaration::FuncDef(func_def));
                        } else {
                            self.skip_to_next_line();
                        }
                    }
                    _ => {
                        self.push_back(token);
                        if let Some(var_def) = self.parse_var_def() {
                            declarations.push(Declaration::VarDef(var_def));
                        } else {
                            self.skip_to_next_line();
                        }
                    }
                }
            }
        }
        Some(declarations)
    }

    fn parse_class_def(&mut self) -> Option<ClassDef> {
        let start = self.next_pos();

        // Parse "class ID ( ID ) : \n {"
        self.eat(Token::Class)?;
        let name = self.take_id()?;
        self.eat(Token::LeftPar)?;
        let super_class = self.take_id()?;
        self.eat(Token::RightPar)?;
        self.eat(Token::Colon)?;
        self.eat(Token::NewLine)?;
        self.eat(Token::Indent)?;

        // Parse body
        let declarations = self.parse_decl_in_class()?;

        // end at NEWLINE, excluding DEDENT
        let end = self.prev_pos().unwrap_or(start);

        self.eat(Token::Dedent)?;

        Some(ClassDef {
            base: NodeBase::from_positions(start, end),
            name,
            super_class,
            declarations,
        })
    }

    fn parse_decl_in_func(&mut self) -> Option<Vec<Declaration>> {
        let mut declarations = vec![];

        loop {
            let head = self.take();
            match head.token {
                Token::Def => {
                    self.push_back(head);
                    if let Some(func_def) = self.parse_func_def() {
                        declarations.push(Declaration::FuncDef(func_def));
                    } else {
                        self.skip_to_next_line();
                    }
                }
                scope @ Token::Global | scope @ Token::Nonlocal => {
                    let start = head.location.start;
                    let variable = self.take_id()?;
                    let end = self.prev_pos().unwrap_or(start);

                    let token = self.take();
                    if token.token != Token::NewLine {
                        self.errors.push(unexpected(token));
                        self.skip_to_next_line();
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
                    let second = self.take();
                    if second.token == Token::Colon {
                        self.push_back(second);
                        self.push_back(head);
                        if let Some(var_def) = self.parse_var_def() {
                            declarations.push(Declaration::VarDef(var_def));
                        } else {
                            self.skip_to_next_line();
                        }
                    } else {
                        self.push_back(second);
                        self.push_back(head);
                        break;
                    }
                }
            }
        }

        Some(declarations)
    }

    fn parse_func_def(&mut self) -> Option<FuncDef> {
        let start = self.next_pos();

        // Parse "def ID ("
        self.eat(Token::Def)?;
        let name = self.take_id()?;
        self.eat(Token::LeftPar)?;

        // Parse "typed_var,* )"
        let token = self.take();
        let mut params = vec![];
        if token.token != Token::RightPar {
            self.push_back(token);
            loop {
                let typed_var = self.parse_typed_var()?;
                params.push(typed_var);

                let token = self.take();
                match token.token {
                    Token::Comma => (),
                    Token::RightPar => break,
                    _ => {
                        self.errors.push(unexpected(token));
                        return None;
                    }
                }
            }
        }

        // Parse `-> type`? : \n {
        let token = self.take();
        let return_type = match token.token {
            Token::Colon => TypeAnnotation::ClassType(ClassType {
                base: NodeBase::from_location(token.location),
                class_name: "<None>".to_owned(),
            }),
            Token::Arrow => {
                let return_type = self.parse_type_annotation()?;

                self.eat(Token::Colon)?;

                return_type
            }
            _ => {
                self.errors.push(unexpected(token));
                return None;
            }
        };

        self.eat(Token::NewLine)?;
        self.eat(Token::Indent)?;

        // Parse declarations
        let declarations = self.parse_decl_in_func()?;

        // Parse statements
        let statements = self.parse_stmt_list();

        let end = self.prev_pos().unwrap_or(start); // exludes DEDENT

        self.eat(Token::Dedent)?;

        Some(FuncDef {
            base: NodeBase::from_positions(start, end),
            name,
            params,
            return_type,
            declarations,
            statements,
        })
    }
    fn parse_var_def(&mut self) -> Option<VarDef> {
        let start = self.next_pos();

        // Parse "typed_var = literal \n"
        let typed_var = self.parse_typed_var()?;

        self.eat(Token::Assign)?;

        let token = self.take();
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
                self.errors.push(unexpected(token));
                return None;
            }
        };

        // end excludes NEWLINE
        let end = self.prev_pos().unwrap_or(start);

        self.eat(Token::NewLine)?;

        Some(VarDef {
            base: NodeBase::from_positions(start, end),
            var: typed_var,
            value,
        })
    }

    fn parse_type_annotation(&mut self) -> Option<TypeAnnotation> {
        let start = self.next_pos();

        let token = self.take();
        match token.token {
            Token::Identifier(class_name) | Token::IdString(class_name) => {
                let end = self.prev_pos().unwrap_or(start);

                Some(TypeAnnotation::ClassType(ClassType {
                    base: NodeBase::from_positions(start, end),
                    class_name,
                }))
            }
            Token::LeftSquare => {
                let element_type = self.parse_type_annotation()?;

                self.eat(Token::RightSquare)?;

                let end = self.prev_pos().unwrap_or(start);

                Some(TypeAnnotation::ListType(Box::new(ListType {
                    base: NodeBase::from_positions(start, end),
                    element_type,
                })))
            }
            _ => {
                self.errors.push(unexpected(token));
                None
            }
        }
    }

    fn parse_typed_var(&mut self) -> Option<TypedVar> {
        let start = self.next_pos();

        // Parse "ID : type"
        let identifier = self.take_id()?;
        self.eat(Token::Colon)?;
        let type_ = self.parse_type_annotation()?;

        let end = self.prev_pos().unwrap_or(start);

        Some(TypedVar {
            base: NodeBase::from_positions(start, end),
            identifier,
            type_,
        })
    }

    fn parse_program(mut self) -> Program {
        let mut declarations = vec![];
        let mut statements = None;

        let start = self.next_pos();
        let mut end = start; // end excludes EOF

        loop {
            let head = self.take();
            match head.token {
                Token::Eof => break,
                Token::Class => {
                    self.push_back(head);
                    if let Some(class_def) = self.parse_class_def() {
                        declarations.push(Declaration::ClassDef(class_def));
                    } else {
                        self.skip_to_next_line();
                    }

                    end = self.prev_pos().unwrap_or(start);
                }
                Token::Def => {
                    self.push_back(head);
                    if let Some(func_def) = self.parse_func_def() {
                        declarations.push(Declaration::FuncDef(func_def));
                    } else {
                        self.skip_to_next_line();
                    }

                    end = self.prev_pos().unwrap_or(start);
                }
                _ => {
                    let second = self.take();
                    if second.token == Token::Colon {
                        self.push_back(second);
                        self.push_back(head);
                        if let Some(var_def) = self.parse_var_def() {
                            declarations.push(Declaration::VarDef(var_def));
                        } else {
                            self.skip_to_next_line();
                        }

                        end = self.prev_pos().unwrap_or(start);
                    } else {
                        self.push_back(second);
                        self.push_back(head);
                        let stmt_list = self.parse_stmt_list();

                        statements = Some(stmt_list);
                        end = self.prev_pos().unwrap_or(start);

                        loop {
                            let token = self.take();
                            if token.token == Token::Eof {
                                break;
                            } else {
                                self.errors.push(unexpected(token));
                            }
                        }
                        break;
                    }
                }
            }
        }

        let statements = statements.unwrap_or_default();

        Program {
            base: NodeBase::from_positions(start, end),
            declarations,
            statements,
            errors: Errors {
                base: NodeBase::new(0, 0, 0, 0),
                errors: self.errors,
            },
        }
    }
}

pub fn parse(get_token: impl Iterator<Item = ComplexToken>) -> Program {
    let parser = Parser::new(get_token);
    parser.parse_program()
}
