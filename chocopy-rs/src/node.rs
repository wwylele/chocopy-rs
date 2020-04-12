use crate::location::*;
use enum_dispatch::*;
use lazy_static::*;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct NodeBase {
    pub location: Location,
    #[serde(rename = "errorMsg", skip_serializing_if = "Option::is_none")]
    pub error_msg: Option<String>,
}

impl NodeBase {
    pub fn new(sr: u32, sc: u32, er: u32, ec: u32) -> NodeBase {
        NodeBase {
            location: Location::new(sr, sc, er, ec),
            error_msg: None,
        }
    }

    pub fn from_positions(start: Position, end: Position) -> NodeBase {
        NodeBase {
            location: Location { start, end },
            error_msg: None,
        }
    }

    pub fn from_location(location: Location) -> NodeBase {
        NodeBase {
            location,
            error_msg: None,
        }
    }
}

#[enum_dispatch(Node)]
pub trait Node {
    fn base(&self) -> &NodeBase;
    fn base_mut(&mut self) -> &mut NodeBase;

    fn add_error(&mut self, errors: &mut Vec<CompilerError>, message: String) {
        let base = self.base_mut();
        base.error_msg = Some(message.clone());
        errors.push(CompilerError {
            base: NodeBase::from_location(base.location),
            message,
            syntax: false,
        })
    }
}

macro_rules! impl_node {
    ($type:ty) => {
        impl Node for $type {
            fn base(&self) -> &NodeBase {
                &self.base
            }

            fn base_mut(&mut self) -> &mut NodeBase {
                &mut self.base
            }
        }
    };
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct AssignStmt {
    #[serde(flatten)]
    pub base: NodeBase,
    pub targets: Vec<Expr>,
    pub value: Expr,
}

impl_node!(AssignStmt);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub enum BinaryOp {
    #[serde(rename = "or")]
    Or,
    #[serde(rename = "and")]
    And,
    #[serde(rename = "+")]
    Add,
    #[serde(rename = "-")]
    Sub,
    #[serde(rename = "*")]
    Mul,
    #[serde(rename = "//")]
    Div,
    #[serde(rename = "%")]
    Mod,
    #[serde(rename = "==")]
    Eq,
    #[serde(rename = "!=")]
    Ne,
    #[serde(rename = "<")]
    Lt,
    #[serde(rename = ">")]
    Gt,
    #[serde(rename = "<=")]
    Le,
    #[serde(rename = ">=")]
    Ge,
    #[serde(rename = "is")]
    Is,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct BinaryExpr {
    #[serde(flatten)]
    pub base: NodeBase,
    pub left: Expr,
    pub operator: BinaryOp,
    pub right: Expr,
}

impl_node!(BinaryExpr);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct BooleanLiteral {
    #[serde(flatten)]
    pub base: NodeBase,
    pub value: bool,
}

impl_node!(BooleanLiteral);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct CallExpr {
    #[serde(flatten)]
    pub base: NodeBase,
    pub function: FuncIdentifier,
    pub args: Vec<Expr>,
}

impl_node!(CallExpr);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct ClassDef {
    #[serde(flatten)]
    pub base: NodeBase,
    pub name: UntypedIdentifier,
    #[serde(rename = "superClass")]
    pub super_class: UntypedIdentifier,
    pub declarations: Vec<Declaration>,
}

impl_node!(ClassDef);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct ClassType {
    #[serde(flatten)]
    pub base: NodeBase,
    #[serde(rename = "className")]
    pub class_name: String,
}

impl_node!(ClassType);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct ClassValueType {
    #[serde(rename = "className")]
    pub class_name: String,
}

impl Display for ClassValueType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.class_name)
    }
}

fn is_not(b: &bool) -> bool {
    !*b
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind")]
pub struct CompilerError {
    #[serde(flatten)]
    pub base: NodeBase,
    pub message: String,
    #[serde(default, skip_serializing_if = "is_not")]
    pub syntax: bool,
}

impl_node!(CompilerError);

#[enum_dispatch(Node)]
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Declaration {
    ClassDef(ClassDef),
    FuncDef(FuncDef),
    GlobalDecl(GlobalDecl),
    NonLocalDecl(NonLocalDecl),
    VarDef(VarDef),
}

impl Declaration {
    pub fn name_mut(&mut self) -> &mut UntypedIdentifier {
        match self {
            Declaration::ClassDef(ClassDef { name, .. }) => name,
            Declaration::FuncDef(FuncDef { name, .. }) => name,
            Declaration::GlobalDecl(GlobalDecl { variable, .. }) => variable,
            Declaration::NonLocalDecl(NonLocalDecl { variable, .. }) => variable,
            Declaration::VarDef(VarDef {
                var: TypedVar { identifier, .. },
                ..
            }) => identifier,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind")]
pub struct Errors {
    #[serde(flatten)]
    pub base: NodeBase,
    pub errors: Vec<CompilerError>,
}

impl_node!(Errors);

impl Errors {
    pub fn sort(&mut self) {
        self.errors.sort_by_key(|error| error.base().location);
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
// #[serde(deny_unknown_fields)] // https://github.com/serde-rs/serde/issues/1358
pub struct Expr {
    #[serde(rename = "inferredType", skip_serializing_if = "Option::is_none")]
    pub inferred_type: Option<ValueType>,
    #[serde(flatten)]
    pub content: ExprContent,
}

impl Expr {
    pub fn get_type(&self) -> &ValueType {
        self.inferred_type
            .as_ref()
            .expect("Type should have been inferred")
    }
}

impl Node for Expr {
    fn base(&self) -> &NodeBase {
        &self.content.base()
    }

    fn base_mut(&mut self) -> &mut NodeBase {
        self.content.base_mut()
    }
}

macro_rules! expr_init {
    ($name:ident, $type:ty) => {
        pub fn $name(e: $type) -> Expr {
            Expr {
                inferred_type: None,
                content: ExprContent::$name(e),
            }
        }
    };
}

#[allow(non_snake_case)]
impl Expr {
    expr_init!(BinaryExpr, Box<BinaryExpr>);
    expr_init!(IntegerLiteral, IntegerLiteral);
    expr_init!(BooleanLiteral, BooleanLiteral);
    expr_init!(CallExpr, CallExpr);
    expr_init!(Identifier, Identifier);
    expr_init!(IfExpr, Box<IfExpr>);
    expr_init!(IndexExpr, Box<IndexExpr>);
    expr_init!(ListExpr, ListExpr);
    expr_init!(MemberExpr, Box<MemberExpr>);
    expr_init!(MethodCallExpr, Box<MethodCallExpr>);
    expr_init!(NoneLiteral, NoneLiteral);
    expr_init!(StringLiteral, StringLiteral);
    expr_init!(UnaryExpr, Box<UnaryExpr>);
}

#[enum_dispatch(Node)]
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum ExprContent {
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
#[serde(deny_unknown_fields)]
pub struct ExprStmt {
    #[serde(flatten)]
    pub base: NodeBase,
    pub expr: Expr,
}

impl_node!(ExprStmt);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct ForStmt {
    #[serde(flatten)]
    pub base: NodeBase,
    pub identifier: TypedIdentifier,
    pub iterable: Expr,
    pub body: Vec<Stmt>,
}

impl_node!(ForStmt);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct FuncDef {
    #[serde(flatten)]
    pub base: NodeBase,
    pub name: UntypedIdentifier,
    pub params: Vec<TypedVar>,
    #[serde(rename = "returnType")]
    pub return_type: TypeAnnotation,
    pub declarations: Vec<Declaration>,
    pub statements: Vec<Stmt>,
}

impl_node!(FuncDef);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind")]
pub struct FuncType {
    pub parameters: Vec<ValueType>,
    #[serde(rename = "returnType")]
    pub return_type: ValueType,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind", rename = "Identifier")]
pub struct FuncIdentifier {
    #[serde(rename = "inferredType", skip_serializing_if = "Option::is_none")]
    pub inferred_type: Option<FuncType>,
    #[serde(flatten)]
    pub base: NodeBase,
    pub name: String,
}

impl FuncIdentifier {
    pub fn get_type(&self) -> &FuncType {
        self.inferred_type
            .as_ref()
            .expect("Type should have been inferred")
    }
}

impl_node!(FuncIdentifier);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct GlobalDecl {
    #[serde(flatten)]
    pub base: NodeBase,
    pub variable: UntypedIdentifier,
}

impl_node!(GlobalDecl);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Identifier {
    #[serde(flatten)]
    pub base: NodeBase,
    pub name: String,
}

impl_node!(Identifier);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind", rename = "Identifier")]
pub struct UntypedIdentifier {
    #[serde(flatten)]
    pub base: NodeBase,
    pub name: String,
}

impl_node!(UntypedIdentifier);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct IfExpr {
    #[serde(flatten)]
    pub base: NodeBase,
    pub condition: Expr,
    #[serde(rename = "thenExpr")]
    pub then_expr: Expr,
    #[serde(rename = "elseExpr")]
    pub else_expr: Expr,
}

impl_node!(IfExpr);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct IfStmt {
    #[serde(flatten)]
    pub base: NodeBase,
    pub condition: Expr,
    #[serde(rename = "thenBody")]
    pub then_body: Vec<Stmt>,
    #[serde(rename = "elseBody")]
    pub else_body: Vec<Stmt>,
}

impl_node!(IfStmt);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct IndexExpr {
    #[serde(flatten)]
    pub base: NodeBase,
    pub list: Expr,
    pub index: Expr,
}

impl_node!(IndexExpr);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct IntegerLiteral {
    #[serde(flatten)]
    pub base: NodeBase,
    pub value: i32,
}

impl_node!(IntegerLiteral);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct ListExpr {
    #[serde(flatten)]
    pub base: NodeBase,
    pub elements: Vec<Expr>,
}

impl_node!(ListExpr);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct ListType {
    #[serde(flatten)]
    pub base: NodeBase,
    #[serde(rename = "elementType")]
    pub element_type: TypeAnnotation,
}

impl_node!(ListType);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct ListValueType {
    #[serde(rename = "elementType")]
    pub element_type: Box<ValueType>,
}

impl Display for ListValueType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "[{}]", self.element_type)
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
// #[serde(deny_unknown_fields)] // https://github.com/serde-rs/serde/issues/1358
pub struct Literal {
    #[serde(rename = "inferredType", skip_serializing_if = "Option::is_none")]
    pub inferred_type: Option<ValueType>,
    #[serde(flatten)]
    pub content: LiteralContent,
}

impl Literal {
    pub fn get_type(&self) -> &ValueType {
        self.inferred_type
            .as_ref()
            .expect("Type should have been inferred")
    }
}

impl Node for Literal {
    fn base(&self) -> &NodeBase {
        &self.content.base()
    }

    fn base_mut(&mut self) -> &mut NodeBase {
        self.content.base_mut()
    }
}

macro_rules! literal_init {
    ($name:ident) => {
        pub fn $name(e: $name) -> Literal {
            Literal {
                inferred_type: None,
                content: LiteralContent::$name(e),
            }
        }
    };
}

#[allow(non_snake_case)]
impl Literal {
    literal_init!(IntegerLiteral);
    literal_init!(BooleanLiteral);
    literal_init!(NoneLiteral);
    literal_init!(StringLiteral);
}

#[enum_dispatch(Node)]
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum LiteralContent {
    IntegerLiteral(IntegerLiteral),
    BooleanLiteral(BooleanLiteral),
    NoneLiteral(NoneLiteral),
    StringLiteral(StringLiteral),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct MemberExpr {
    #[serde(flatten)]
    pub base: NodeBase,
    pub object: Expr,
    pub member: UntypedIdentifier,
}

impl_node!(MemberExpr);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(rename = "MemberExpr", tag = "kind")]
pub struct TypedMemberExpr {
    #[serde(rename = "inferredType", skip_serializing_if = "Option::is_none")]
    pub inferred_type: Option<FuncType>,
    #[serde(flatten)]
    pub base: NodeBase,
    pub object: Expr,
    pub member: UntypedIdentifier,
}

impl TypedMemberExpr {
    pub fn get_type(&self) -> &FuncType {
        self.inferred_type
            .as_ref()
            .expect("Type should have been inferred")
    }
}

impl_node!(TypedMemberExpr);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct MethodCallExpr {
    #[serde(flatten)]
    pub base: NodeBase,
    pub method: TypedMemberExpr,
    pub args: Vec<Expr>,
}

impl_node!(MethodCallExpr);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct NoneLiteral {
    #[serde(flatten)]
    pub base: NodeBase,
}

impl_node!(NoneLiteral);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct NonLocalDecl {
    #[serde(flatten)]
    pub base: NodeBase,
    pub variable: UntypedIdentifier,
}

impl_node!(NonLocalDecl);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind")]
pub struct Program {
    #[serde(flatten)]
    pub base: NodeBase,
    pub declarations: Vec<Declaration>,
    pub statements: Vec<Stmt>,
    pub errors: Errors,
}

impl_node!(Program);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct ReturnStmt {
    #[serde(flatten)]
    pub base: NodeBase,
    pub value: Option<Expr>,
}

impl_node!(ReturnStmt);

#[enum_dispatch(Node)]
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Stmt {
    ExprStmt(ExprStmt),
    AssignStmt(AssignStmt),
    ForStmt(ForStmt),
    IfStmt(IfStmt),
    ReturnStmt(ReturnStmt),
    WhileStmt(WhileStmt),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct StringLiteral {
    #[serde(flatten)]
    pub base: NodeBase,
    pub value: String,
}

impl_node!(StringLiteral);

#[enum_dispatch(Node)]
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum TypeAnnotation {
    ClassType(ClassType),
    ListType(Box<ListType>),
}

impl TypeAnnotation {
    pub fn core_type_mut(&mut self) -> &mut ClassType {
        match self {
            TypeAnnotation::ClassType(c) => c,
            TypeAnnotation::ListType(l) => l.element_type.core_type_mut(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind", rename = "Identifier")]
pub struct TypedIdentifier {
    #[serde(rename = "inferredType", skip_serializing_if = "Option::is_none")]
    pub inferred_type: Option<ValueType>,
    #[serde(flatten)]
    pub base: NodeBase,
    pub name: String,
}

impl TypedIdentifier {
    pub fn get_type(&self) -> &ValueType {
        self.inferred_type
            .as_ref()
            .expect("Type should have been inferred")
    }
}

impl_node!(TypedIdentifier);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind")]
pub struct TypedVar {
    #[serde(flatten)]
    pub base: NodeBase,
    pub identifier: UntypedIdentifier,
    #[serde(rename = "type")]
    pub type_: TypeAnnotation,
}

impl_node!(TypedVar);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub enum UnaryOp {
    #[serde(rename = "-")]
    Negative,
    #[serde(rename = "not")]
    Not,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct UnaryExpr {
    #[serde(flatten)]
    pub base: NodeBase,
    pub operator: UnaryOp,
    pub operand: Expr,
}

impl_node!(UnaryExpr);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum ValueType {
    ClassValueType(ClassValueType),
    ListValueType(ListValueType),
}

impl Display for ValueType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ValueType::ClassValueType(v) => v.fmt(f),
            ValueType::ListValueType(v) => v.fmt(f),
        }
    }
}

impl ValueType {
    pub fn from_annotation(t: &TypeAnnotation) -> ValueType {
        match t {
            TypeAnnotation::ClassType(c) => ValueType::ClassValueType(ClassValueType {
                class_name: c.class_name.clone(),
            }),
            TypeAnnotation::ListType(c) => ValueType::ListValueType(ListValueType {
                element_type: Box::new(ValueType::from_annotation(&c.element_type)),
            }),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct VarDef {
    #[serde(flatten)]
    pub base: NodeBase,
    pub var: TypedVar,
    pub value: Literal,
}

impl_node!(VarDef);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct WhileStmt {
    #[serde(flatten)]
    pub base: NodeBase,
    pub condition: Expr,
    pub body: Vec<Stmt>,
}

impl_node!(WhileStmt);

lazy_static! {
    pub static ref TYPE_OBJECT: ValueType = ValueType::ClassValueType(ClassValueType {
        class_name: "object".to_owned(),
    });
    pub static ref TYPE_NONE: ValueType = ValueType::ClassValueType(ClassValueType {
        class_name: "<None>".to_owned(),
    });
    pub static ref TYPE_EMPTY: ValueType = ValueType::ClassValueType(ClassValueType {
        class_name: "<Empty>".to_owned(),
    });
    pub static ref TYPE_STR: ValueType = ValueType::ClassValueType(ClassValueType {
        class_name: "str".to_owned(),
    });
    pub static ref TYPE_INT: ValueType = ValueType::ClassValueType(ClassValueType {
        class_name: "int".to_owned(),
    });
    pub static ref TYPE_BOOL: ValueType = ValueType::ClassValueType(ClassValueType {
        class_name: "bool".to_owned(),
    });
    pub static ref TYPE_NONE_LIST: ValueType = ValueType::ListValueType(ListValueType {
        element_type: Box::new(TYPE_NONE.clone())
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn serialize() {
        let program = Program {
            base: NodeBase::new(1, 1, 1, 10),
            declarations: vec![Declaration::VarDef(VarDef {
                base: NodeBase::new(0, 0, 0, 0),
                var: TypedVar {
                    base: NodeBase::new(0, 0, 0, 0),
                    identifier: UntypedIdentifier {
                        base: NodeBase::new(0, 0, 0, 0),
                        name: "a".to_owned(),
                    },
                    type_: TypeAnnotation::ClassType(ClassType {
                        base: NodeBase::new(0, 0, 0, 0),
                        class_name: "a".to_owned(),
                    }),
                },
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
                        operator: BinaryOp::Add,
                        right: Expr::IntegerLiteral(IntegerLiteral {
                            base: NodeBase::new(1, 5, 1, 5),
                            value: 2,
                        }),
                    })),
                    operator: BinaryOp::Add,
                    right: Expr::IntegerLiteral(IntegerLiteral {
                        base: NodeBase::new(1, 9, 1, 9),
                        value: 3,
                    }),
                })),
            })],
            errors: Errors {
                base: NodeBase::new(0, 0, 0, 0),
                errors: vec![],
            },
        };

        let json = serde_json::to_string_pretty(&program).unwrap();
        let recover = serde_json::from_str(&json).unwrap();
        assert_eq!(program, recover);
        println!("{}", json);
    }
}
