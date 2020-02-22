use super::token::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct NodeBase {
    pub location: Location,
}

impl NodeBase {
    pub fn new(sr: u32, sc: u32, er: u32, ec: u32) -> NodeBase {
        NodeBase {
            location: Location::new(sr, sc, er, ec),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind")]
pub enum Ast {
    Program(Program),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct AssignStmt {
    #[serde(flatten)]
    pub base: NodeBase,
    pub targets: Vec<Expr>,
    pub value: Expr,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct BinaryExpr {
    #[serde(flatten)]
    pub base: NodeBase,
    pub left: Expr,
    pub operator: String,
    pub right: Expr,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct BooleanLiteral {
    #[serde(flatten)]
    pub base: NodeBase,
    pub value: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct CallExpr {
    #[serde(flatten)]
    pub base: NodeBase,
    pub function: Id,
    pub args: Vec<Expr>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct ClassDef {
    #[serde(flatten)]
    pub base: NodeBase,
    pub name: Id,
    #[serde(rename = "superClass")]
    pub super_class: Id,
    pub declarations: Vec<Declaration>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct ClassType {
    #[serde(flatten)]
    pub base: NodeBase,
    #[serde(rename = "className")]
    pub class_name: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct CompilerError {
    #[serde(flatten)]
    pub base: NodeBase,
    pub message: String,
    pub syntax: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind")]
pub enum Declaration {
    ClassDef(ClassDef),
    FuncDef(FuncDef),
    GlobalDecl(GlobalDecl),
    NonLocalDecl(NonLocalDecl),
    VarDef(VarDef),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind")]
pub enum Error {
    CompilerError(CompilerError),
}

impl Error {
    pub fn unexpected(token: ComplexToken) -> Error {
        Error::CompilerError(CompilerError {
            base: NodeBase {
                location: token.location,
            },
            message: "unexptected token".to_owned(),
            syntax: true,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Errors {
    #[serde(flatten)]
    pub base: NodeBase,
    pub errors: Vec<Error>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind")]
pub enum ErrorInfo {
    Errors(Errors),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind")]
pub enum Expr {
    BinaryExpr(Box<BinaryExpr>),
    IntegerLiteral(IntegerLiteral),
    BooleanLiteral(BooleanLiteral),
    CallExpr(CallExpr),
    Identifier(Identifier),
    IfExpr(Box<IfExpr>),
    IndexExpr(Box<IndexExpr>),
    ListExpr(ListExpr),
    MemberExpr(Box<MemberExpr>),
    MethodCallExpr(Box<MethodCallExpr>),
    NoneLiteral(NoneLiteral),
    StringLiteral(StringLiteral),
    UnaryExpr(Box<UnaryExpr>),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct ExprStmt {
    #[serde(flatten)]
    pub base: NodeBase,
    pub expr: Expr,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct ForStmt {
    #[serde(flatten)]
    pub base: NodeBase,
    pub identifier: Id,
    pub iterable: Expr,
    pub body: Vec<Stmt>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct FuncDef {
    #[serde(flatten)]
    pub base: NodeBase,
    pub name: Id,
    pub params: Vec<Tv>,
    #[serde(rename = "returnType")]
    pub return_type: TypeAnnotation,
    pub declarations: Vec<Declaration>,
    pub statements: Vec<Stmt>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct GlobalDecl {
    #[serde(flatten)]
    pub base: NodeBase,
    pub variable: Id,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind")]
pub enum Id {
    Identifier(Identifier),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Identifier {
    #[serde(flatten)]
    pub base: NodeBase,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct IfExpr {
    #[serde(flatten)]
    pub base: NodeBase,
    pub condition: Expr,
    #[serde(rename = "thenExpr")]
    pub then_expr: Expr,
    #[serde(rename = "elseExpr")]
    pub else_expr: Expr,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct IfStmt {
    #[serde(flatten)]
    pub base: NodeBase,
    pub condition: Expr,
    #[serde(rename = "thenBody")]
    pub then_body: Vec<Stmt>,
    #[serde(rename = "elseBody")]
    pub else_body: Vec<Stmt>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct IndexExpr {
    #[serde(flatten)]
    pub base: NodeBase,
    pub list: Expr,
    pub index: Expr,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct IntegerLiteral {
    #[serde(flatten)]
    pub base: NodeBase,
    pub value: i32,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct ListExpr {
    #[serde(flatten)]
    pub base: NodeBase,
    pub elements: Vec<Expr>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct ListType {
    #[serde(flatten)]
    pub base: NodeBase,
    #[serde(rename = "elementType")]
    pub element_type: TypeAnnotation,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind")]
pub enum Literal {
    IntegerLiteral(IntegerLiteral),
    BooleanLiteral(BooleanLiteral),
    NoneLiteral(NoneLiteral),
    StringLiteral(StringLiteral),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct MemberExpr {
    #[serde(flatten)]
    pub base: NodeBase,
    pub object: Expr,
    pub member: Id,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind")]
pub enum Method {
    MemberExpr(MemberExpr),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct MethodCallExpr {
    #[serde(flatten)]
    pub base: NodeBase,
    pub method: Method,
    pub args: Vec<Expr>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct NoneLiteral {
    #[serde(flatten)]
    pub base: NodeBase,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct NonLocalDecl {
    #[serde(flatten)]
    pub base: NodeBase,
    pub variable: Id,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Program {
    #[serde(flatten)]
    pub base: NodeBase,
    pub declarations: Vec<Declaration>,
    pub statements: Vec<Stmt>,
    pub errors: ErrorInfo,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct ReturnStmt {
    #[serde(flatten)]
    pub base: NodeBase,
    pub value: Option<Expr>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind")]
pub enum Stmt {
    ExprStmt(ExprStmt),
    AssignStmt(AssignStmt),
    ForStmt(ForStmt),
    IfStmt(IfStmt),
    ReturnStmt(ReturnStmt),
    WhileStmt(WhileStmt),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct StringLiteral {
    #[serde(flatten)]
    pub base: NodeBase,
    pub value: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind")]
pub enum Tv {
    TypedVar(TypedVar),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind")]
pub enum TypeAnnotation {
    ClassType(ClassType),
    ListType(Box<ListType>),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct TypedVar {
    #[serde(flatten)]
    pub base: NodeBase,
    pub identifier: Id,
    #[serde(rename = "type")]
    pub type_: TypeAnnotation,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct UnaryExpr {
    #[serde(flatten)]
    pub base: NodeBase,
    pub operator: String,
    pub operand: Expr,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct VarDef {
    #[serde(flatten)]
    pub base: NodeBase,
    pub var: Tv,
    pub value: Literal,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct WhileStmt {
    #[serde(flatten)]
    pub base: NodeBase,
    pub condition: Expr,
    pub body: Vec<Stmt>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn serialize() {
        let program = Ast::Program(Program {
            base: NodeBase::new(1, 1, 1, 10),
            declarations: vec![Declaration::VarDef(VarDef {
                base: NodeBase::new(0, 0, 0, 0),
                var: Tv::TypedVar(TypedVar {
                    base: NodeBase::new(0, 0, 0, 0),
                    identifier: Id::Identifier(Identifier {
                        base: NodeBase::new(0, 0, 0, 0),
                        name: "a".to_owned(),
                    }),
                    type_: TypeAnnotation::ClassType(ClassType {
                        base: NodeBase::new(0, 0, 0, 0),
                        class_name: "a".to_owned(),
                    }),
                }),
                value: Literal::BooleanLiteral(BooleanLiteral {
                    base: NodeBase::new(0, 0, 0, 0),
                    value: true,
                }),
            })],
            statements: vec![Stmt::ExprStmt(ExprStmt {
                base: NodeBase::new(1, 1, 1, 9),
                expr: Expr::BinaryExpr(Box::new(BinaryExpr {
                    base: NodeBase::new(1, 1, 1, 9),
                    left: Expr::BinaryExpr(Box::new(BinaryExpr {
                        base: NodeBase::new(1, 1, 1, 5),
                        left: Expr::IntegerLiteral(IntegerLiteral {
                            base: NodeBase::new(1, 1, 1, 1),
                            value: 1,
                        }),
                        operator: "+".to_owned(),
                        right: Expr::IntegerLiteral(IntegerLiteral {
                            base: NodeBase::new(1, 5, 1, 5),
                            value: 2,
                        }),
                    })),
                    operator: "+".to_owned(),
                    right: Expr::IntegerLiteral(IntegerLiteral {
                        base: NodeBase::new(1, 9, 1, 9),
                        value: 3,
                    }),
                })),
            })],
            errors: ErrorInfo::Errors(Errors {
                base: NodeBase::new(0, 0, 0, 0),
                errors: vec![],
            }),
        });

        let json = serde_json::to_string_pretty(&program).unwrap();
        let recover = serde_json::from_str(&json).unwrap();
        assert_eq!(program, recover);
        println!("{}", json);
    }
}
