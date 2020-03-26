use crate::local_env::*;
use crate::location::*;
use enum_dispatch::*;
use lazy_static::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::*;
use std::fmt::{self, Display, Formatter};

pub type TypeLocalEnv = LocalEnv<FuncType, ValueType>;

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

pub struct ClassInfo {
    pub super_class: String,
    pub items: HashMap<String, Type>,
}

pub struct ClassEnv(pub HashMap<String, ClassInfo>);

#[enum_dispatch(Node)]
pub trait Node {
    fn base(&self) -> &NodeBase;
    fn base_mut(&mut self) -> &mut NodeBase;
    fn analyze(
        &mut self,
        errors: &mut Vec<Error>,
        o: &mut TypeLocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) -> Option<ValueType>;
}

macro_rules! impl_default_analyze {
    ($type:ty) => {
        impl $type {
            fn analyze_impl(
                &mut self,
                _: &mut Vec<Error>,
                _: &TypeLocalEnv,
                _: &ClassEnv,
                _: Option<&ValueType>,
            ) -> Option<ValueType> {
                None
            }
        }
    };
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

            fn analyze(
                &mut self,
                errors: &mut Vec<Error>,
                o: &mut TypeLocalEnv,
                m: &ClassEnv,
                r: Option<&ValueType>,
            ) -> Option<ValueType> {
                self.analyze_impl(errors, o, m, r)
            }
        }
    };
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields, tag = "kind")]
pub enum Ast {
    Program(Program),
}

impl Ast {
    pub fn program(&self) -> &Program {
        let Ast::Program(program) = self;
        program
    }

    pub fn program_mut(&mut self) -> &mut Program {
        let Ast::Program(program) = self;
        program
    }
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
    pub function: FuncId,
    pub args: Vec<Expr>,
}

impl_node!(CallExpr);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct ClassDef {
    #[serde(flatten)]
    pub base: NodeBase,
    pub name: Id,
    #[serde(rename = "superClass")]
    pub super_class: Id,
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
impl_default_analyze!(ClassType);

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
#[serde(deny_unknown_fields)]
pub struct CompilerError {
    #[serde(flatten)]
    pub base: NodeBase,
    pub message: String,
    #[serde(default, skip_serializing_if = "is_not")]
    pub syntax: bool,
}

impl_node!(CompilerError);
impl_default_analyze!(CompilerError);

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
    pub fn name_mut(&mut self) -> &mut Identifier {
        match self {
            Declaration::ClassDef(ClassDef { name, .. }) => name,
            Declaration::FuncDef(FuncDef { name, .. }) => name,
            Declaration::GlobalDecl(GlobalDecl { variable, .. }) => variable,
            Declaration::NonLocalDecl(NonLocalDecl { variable, .. }) => variable,
            Declaration::VarDef(VarDef {
                var: Tv::TypedVar(TypedVar { identifier, .. }),
                ..
            }) => identifier,
        }
        .id_mut()
    }
}

#[enum_dispatch(Node)]
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Error {
    CompilerError(CompilerError),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Errors {
    #[serde(flatten)]
    pub base: NodeBase,
    pub errors: Vec<Error>,
}

impl_node!(Errors);
impl_default_analyze!(Errors);

#[enum_dispatch(Node)]
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum ErrorInfo {
    Errors(Errors),
}

impl ErrorInfo {
    pub fn errors(&self) -> &Errors {
        let ErrorInfo::Errors(errors) = self;
        errors
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

impl Node for Expr {
    fn base(&self) -> &NodeBase {
        &self.content.base()
    }

    fn base_mut(&mut self) -> &mut NodeBase {
        self.content.base_mut()
    }

    fn analyze(
        &mut self,
        errors: &mut Vec<Error>,
        o: &mut TypeLocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) -> Option<ValueType> {
        let t = self.content.analyze(errors, o, m, r);
        self.inferred_type = t.clone();
        t
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
    pub identifier: TypedId,
    pub iterable: Expr,
    pub body: Vec<Stmt>,
}

impl_node!(ForStmt);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
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

impl_node!(FuncDef);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct FuncType {
    pub parameters: Vec<ValueType>,
    #[serde(rename = "returnType")]
    pub return_type: ValueType,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum FuncTypeWrapper {
    FuncType(FuncType),
}

impl FuncTypeWrapper {
    pub fn func_type(&self) -> &FuncType {
        let FuncTypeWrapper::FuncType(func_type) = self;
        func_type
    }
}

#[enum_dispatch(Node)]
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum FuncId {
    Identifier(FuncIdentifier),
}

impl FuncId {
    pub fn id(&self) -> &FuncIdentifier {
        let FuncId::Identifier(id) = self;
        id
    }
    pub fn id_mut(&mut self) -> &mut FuncIdentifier {
        let FuncId::Identifier(id) = self;
        id
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct FuncIdentifier {
    #[serde(rename = "inferredType", skip_serializing_if = "Option::is_none")]
    pub inferred_type: Option<FuncTypeWrapper>,
    #[serde(flatten)]
    pub base: NodeBase,
    pub name: String,
}

impl_node!(FuncIdentifier);
impl_default_analyze!(FuncIdentifier);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct GlobalDecl {
    #[serde(flatten)]
    pub base: NodeBase,
    pub variable: Id,
}

impl_node!(GlobalDecl);
impl_default_analyze!(GlobalDecl);

#[enum_dispatch(Node)]
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Id {
    Identifier(Identifier),
}

impl Id {
    pub fn id(&self) -> &Identifier {
        let Id::Identifier(id) = self;
        id
    }

    pub fn id_mut(&mut self) -> &mut Identifier {
        let Id::Identifier(id) = self;
        id
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Identifier {
    #[serde(flatten)]
    pub base: NodeBase,
    pub name: String,
}

impl_node!(Identifier);

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
impl_default_analyze!(ListType);

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

impl Node for Literal {
    fn base(&self) -> &NodeBase {
        &self.content.base()
    }

    fn base_mut(&mut self) -> &mut NodeBase {
        self.content.base_mut()
    }

    fn analyze(
        &mut self,
        errors: &mut Vec<Error>,
        o: &mut TypeLocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) -> Option<ValueType> {
        let t = self.content.analyze(errors, o, m, r);
        self.inferred_type = t.clone();
        t
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
    pub member: Id,
}

impl_node!(MemberExpr);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct TypedMemberExpr {
    #[serde(rename = "inferredType", skip_serializing_if = "Option::is_none")]
    pub inferred_type: Option<FuncTypeWrapper>,
    #[serde(flatten)]
    pub base: NodeBase,
    pub object: Expr,
    pub member: Id,
}

impl_node!(TypedMemberExpr);
impl_default_analyze!(TypedMemberExpr);

#[enum_dispatch(Node)]
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Method {
    MemberExpr(TypedMemberExpr),
}

impl Method {
    pub fn member(&self) -> &TypedMemberExpr {
        let Method::MemberExpr(member) = self;
        member
    }
    pub fn member_mut(&mut self) -> &mut TypedMemberExpr {
        let Method::MemberExpr(member) = self;
        member
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct MethodCallExpr {
    #[serde(flatten)]
    pub base: NodeBase,
    pub method: Method,
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
    pub variable: Id,
}

impl_node!(NonLocalDecl);
impl_default_analyze!(NonLocalDecl);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Program {
    #[serde(flatten)]
    pub base: NodeBase,
    pub declarations: Vec<Declaration>,
    pub statements: Vec<Stmt>,
    pub errors: ErrorInfo,
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
pub enum Tv {
    TypedVar(TypedVar),
}

impl Tv {
    pub fn tv(&self) -> &TypedVar {
        let Tv::TypedVar(tv) = self;
        tv
    }

    pub fn tv_mut(&mut self) -> &mut TypedVar {
        let Tv::TypedVar(tv) = self;
        tv
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Type {
    ValueType(ValueType),
    FuncType(FuncType),
}

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

#[enum_dispatch(Node)]
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum TypedId {
    Identifier(TypedIdentifier),
}

impl TypedId {
    pub fn id(&self) -> &TypedIdentifier {
        let TypedId::Identifier(id) = self;
        id
    }
    pub fn id_mut(&mut self) -> &mut TypedIdentifier {
        let TypedId::Identifier(id) = self;
        id
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct TypedIdentifier {
    #[serde(rename = "inferredType", skip_serializing_if = "Option::is_none")]
    pub inferred_type: Option<ValueType>,
    #[serde(flatten)]
    pub base: NodeBase,
    pub name: String,
}

impl_node!(TypedIdentifier);
impl_default_analyze!(TypedIdentifier);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct TypedVar {
    #[serde(flatten)]
    pub base: NodeBase,
    pub identifier: Id,
    #[serde(rename = "type")]
    pub type_: TypeAnnotation,
}

impl_node!(TypedVar);
impl_default_analyze!(TypedVar);

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

impl From<ValueType> for Type {
    fn from(t: ValueType) -> Type {
        Type::ValueType(t)
    }
}

impl TryFrom<Type> for ValueType {
    type Error = ();
    fn try_from(t: Type) -> Result<ValueType, ()> {
        match t {
            Type::ValueType(c) => Ok(c),
            _ => Err(()),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct VarDef {
    #[serde(flatten)]
    pub base: NodeBase,
    pub var: Tv,
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
