use super::class_env::*;
use super::error::*;
use crate::local_env::*;
use crate::node::*;
use std::collections::HashMap;

type TypeLocalEnv = LocalEnv<FuncType, ValueType>;

impl Expr {
    pub fn analyze(
        &mut self,
        errors: &mut Vec<CompilerError>,
        o: &mut TypeLocalEnv,
        m: &ClassEnv,
    ) -> ValueType {
        let inferred_type = match &mut self.content {
            ExprContent::BinaryExpr(s) => s.analyze(errors, o, m),
            ExprContent::IntegerLiteral(s) => s.analyze(errors, o, m),
            ExprContent::BooleanLiteral(s) => s.analyze(errors, o, m),
            ExprContent::CallExpr(s) => s.analyze(errors, o, m),
            ExprContent::Identifier(s) => s.analyze(errors, o, m),
            ExprContent::IfExpr(s) => s.analyze(errors, o, m),
            ExprContent::IndexExpr(s) => s.analyze(errors, o, m),
            ExprContent::ListExpr(s) => s.analyze(errors, o, m),
            ExprContent::MemberExpr(s) => s.analyze(errors, o, m),
            ExprContent::MethodCallExpr(s) => s.analyze(errors, o, m),
            ExprContent::NoneLiteral(s) => s.analyze(errors, o, m),
            ExprContent::StringLiteral(s) => s.analyze(errors, o, m),
            ExprContent::UnaryExpr(s) => s.analyze(errors, o, m),
        };
        self.inferred_type = Some(inferred_type.clone());
        inferred_type
    }
}

impl Literal {
    pub fn analyze(
        &mut self,
        errors: &mut Vec<CompilerError>,
        o: &mut TypeLocalEnv,
        m: &ClassEnv,
    ) -> ValueType {
        let inferred_type = match &mut self.content {
            LiteralContent::IntegerLiteral(s) => s.analyze(errors, o, m),
            LiteralContent::BooleanLiteral(s) => s.analyze(errors, o, m),
            LiteralContent::NoneLiteral(s) => s.analyze(errors, o, m),
            LiteralContent::StringLiteral(s) => s.analyze(errors, o, m),
        };
        self.inferred_type = Some(inferred_type.clone());
        inferred_type
    }
}

// Only for variable
impl Identifier {
    pub fn analyze(
        &mut self,
        errors: &mut Vec<CompilerError>,
        o: &mut TypeLocalEnv,
        _m: &ClassEnv,
    ) -> ValueType {
        match o.get(&self.name) {
            None | Some(EnvSlot::Func(_)) => {
                let msg = error_variable(&self.name);
                self.base_mut().error_msg = Some(msg);
                errors.push(error_from(self));
                TYPE_OBJECT.clone()
            }
            Some(EnvSlot::Var(t, _)) => t.clone(),
        }
    }
}

impl AssignStmt {
    pub fn analyze(
        &mut self,
        errors: &mut Vec<CompilerError>,
        o: &mut TypeLocalEnv,
        m: &ClassEnv,
        _r: Option<&ValueType>,
    ) {
        let right: ValueType = self.value.analyze(errors, o, m);

        // We don't do `for target in &mut self.targets` because of mut ref conflict
        for i in 0..self.targets.len() {
            let left: ValueType = self.targets[i].analyze(errors, o, m);
            if let ExprContent::Identifier(Identifier { name, .. }) = &self.targets[i].content {
                if let Some(EnvSlot::Var(_, Assignable(false))) = o.get(name) {
                    let msg = error_nonlocal_assign(name);
                    self.targets[i].base_mut().error_msg = Some(msg);
                    errors.push(error_from(&self.targets[i]));
                }
            } else if let ExprContent::IndexExpr(index_expr) = &self.targets[i].content {
                if index_expr.list.inferred_type.as_ref().unwrap() == &*TYPE_STR
                    && self.targets[i].base_mut().error_msg.is_none()
                {
                    let msg = error_str_index_assign();
                    self.targets[i].base_mut().error_msg = Some(msg);
                    errors.push(error_from(&self.targets[i]));
                }
            }

            if !m.is_compatible(&right, &left) && self.base.error_msg.is_none() {
                let msg = error_assign(&left, &right);
                self.base_mut().error_msg = Some(msg);
                errors.push(error_from(self));
            }
        }

        if self.targets.len() > 1 && right == *TYPE_NONE_LIST && self.base().error_msg.is_none() {
            let msg = error_multi_assign();
            self.base_mut().error_msg = Some(msg);
            errors.push(error_from(self));
        }
    }
}

impl VarDef {
    pub fn analyze(&mut self, errors: &mut Vec<CompilerError>, o: &mut TypeLocalEnv, m: &ClassEnv) {
        let right = self.value.analyze(errors, o, m);
        let left = ValueType::from_annotation(&self.var.type_);
        if !m.is_compatible(&right, &left) {
            let msg = error_assign(&left, &right);
            self.base_mut().error_msg = Some(msg);
            errors.push(error_from(self));
        }
    }
}

impl ExprStmt {
    pub fn analyze(
        &mut self,
        errors: &mut Vec<CompilerError>,
        o: &mut TypeLocalEnv,
        m: &ClassEnv,
        _r: Option<&ValueType>,
    ) {
        self.expr.analyze(errors, o, m);
    }
}

impl BooleanLiteral {
    pub fn analyze(
        &mut self,
        _errors: &mut Vec<CompilerError>,
        _o: &mut TypeLocalEnv,
        _m: &ClassEnv,
    ) -> ValueType {
        TYPE_BOOL.clone()
    }
}

impl IntegerLiteral {
    pub fn analyze(
        &mut self,
        _errors: &mut Vec<CompilerError>,
        _o: &mut TypeLocalEnv,
        _m: &ClassEnv,
    ) -> ValueType {
        TYPE_INT.clone()
    }
}

impl StringLiteral {
    pub fn analyze(
        &mut self,
        _errors: &mut Vec<CompilerError>,
        _o: &mut TypeLocalEnv,
        _m: &ClassEnv,
    ) -> ValueType {
        TYPE_STR.clone()
    }
}

impl NoneLiteral {
    pub fn analyze(
        &mut self,
        _errors: &mut Vec<CompilerError>,
        _o: &mut TypeLocalEnv,
        _m: &ClassEnv,
    ) -> ValueType {
        TYPE_NONE.clone()
    }
}

impl UnaryExpr {
    pub fn analyze(
        &mut self,
        errors: &mut Vec<CompilerError>,
        o: &mut TypeLocalEnv,
        m: &ClassEnv,
    ) -> ValueType {
        let operand: ValueType = self.operand.analyze(errors, o, m);
        match self.operator {
            UnaryOp::Negative => {
                if operand != *TYPE_INT {
                    let msg = error_unary("-", &operand);
                    self.base_mut().error_msg = Some(msg);
                    errors.push(error_from(self));
                }
                TYPE_INT.clone()
            }
            UnaryOp::Not => {
                if operand != *TYPE_BOOL {
                    let msg = error_unary("not", &operand);
                    self.base_mut().error_msg = Some(msg);
                    errors.push(error_from(self));
                }
                TYPE_BOOL.clone()
            }
        }
    }
}

impl BinaryExpr {
    pub fn analyze(
        &mut self,
        errors: &mut Vec<CompilerError>,
        o: &mut TypeLocalEnv,
        m: &ClassEnv,
    ) -> ValueType {
        let left: ValueType = self.left.analyze(errors, o, m);
        let right: ValueType = self.right.analyze(errors, o, m);

        let mut error = false;
        let output = match self.operator {
            BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                if left != *TYPE_INT || right != *TYPE_INT {
                    error = true;
                }
                TYPE_INT.clone()
            }
            BinaryOp::Or | BinaryOp::And => {
                if left != *TYPE_BOOL || right != *TYPE_BOOL {
                    error = true;
                }
                TYPE_BOOL.clone()
            }
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                if left != *TYPE_INT || right != *TYPE_INT {
                    error = true;
                }
                TYPE_BOOL.clone()
            }
            BinaryOp::Is => {
                let is_basic =
                    |t: &ValueType| *t == *TYPE_INT || *t == *TYPE_BOOL || *t == *TYPE_STR;
                if is_basic(&left) || is_basic(&right) {
                    error = true;
                }
                TYPE_BOOL.clone()
            }
            BinaryOp::Add => {
                if left == *TYPE_INT || right == *TYPE_INT {
                    if left != right {
                        error = true;
                    }
                    TYPE_INT.clone()
                } else if left == *TYPE_STR {
                    if left != right {
                        error = true;
                        TYPE_OBJECT.clone()
                    } else {
                        TYPE_STR.clone()
                    }
                } else if let (
                    ValueType::ListValueType(ListValueType {
                        element_type: left_element,
                    }),
                    ValueType::ListValueType(ListValueType {
                        element_type: right_element,
                    }),
                ) = (&left, &right)
                {
                    let element_type = Box::new(m.join(&left_element, &right_element));
                    ValueType::ListValueType(ListValueType { element_type })
                } else {
                    error = true;
                    TYPE_OBJECT.clone()
                }
            }
            BinaryOp::Eq | BinaryOp::Ne => {
                if left != *TYPE_INT && left != *TYPE_STR && left != *TYPE_BOOL {
                    error = true
                } else if left != right {
                    error = true
                }
                TYPE_BOOL.clone()
            }
        };

        if error {
            let op_name = match self.operator {
                BinaryOp::Or => "or",
                BinaryOp::And => "and",
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Div => "//",
                BinaryOp::Mod => "%",
                BinaryOp::Eq => "==",
                BinaryOp::Ne => "!=",
                BinaryOp::Lt => "<",
                BinaryOp::Gt => ">",
                BinaryOp::Le => "<=",
                BinaryOp::Ge => ">=",
                BinaryOp::Is => "is",
            };
            let msg = error_binary(op_name, &left, &right);
            self.base_mut().error_msg = Some(msg);
            errors.push(error_from(self));
        }

        output
    }
}

impl IfExpr {
    pub fn analyze(
        &mut self,
        errors: &mut Vec<CompilerError>,
        o: &mut TypeLocalEnv,
        m: &ClassEnv,
    ) -> ValueType {
        let condition = self.condition.analyze(errors, o, m);
        if condition != *TYPE_BOOL {
            let msg = error_condition(&condition);
            self.base_mut().error_msg = Some(msg);
            errors.push(error_from(self))
        }
        let then_type = self.then_expr.analyze(errors, o, m);
        let else_type = self.else_expr.analyze(errors, o, m);
        m.join(&then_type, &else_type)
    }
}

impl ListExpr {
    pub fn analyze(
        &mut self,
        errors: &mut Vec<CompilerError>,
        o: &mut TypeLocalEnv,
        m: &ClassEnv,
    ) -> ValueType {
        if self.elements.len() == 0 {
            return TYPE_EMPTY.clone();
        }
        let mut element_type = self.elements[0].analyze(errors, o, m);
        for element in self.elements.iter_mut().skip(1) {
            element_type = m.join(&element_type, &element.analyze(errors, o, m));
        }

        let element_type = Box::new(element_type);
        ValueType::ListValueType(ListValueType { element_type })
    }
}

impl IndexExpr {
    pub fn analyze(
        &mut self,
        errors: &mut Vec<CompilerError>,
        o: &mut TypeLocalEnv,
        m: &ClassEnv,
    ) -> ValueType {
        let left = self.list.analyze(errors, o, m);
        let element_type = if let ValueType::ListValueType(ListValueType { element_type }) = left {
            *element_type
        } else if left == *TYPE_STR {
            TYPE_STR.clone()
        } else {
            let msg = error_index_left(&left);
            self.base_mut().error_msg = Some(msg);
            errors.push(error_from(self));
            TYPE_OBJECT.clone()
        };

        let index = self.index.analyze(errors, o, m);
        if index != *TYPE_INT && self.base().error_msg.is_none() {
            let msg = error_index_right(&index);
            self.base_mut().error_msg = Some(msg);
            errors.push(error_from(self));
        }

        element_type
    }
}

impl MemberExpr {
    pub fn analyze(
        &mut self,
        errors: &mut Vec<CompilerError>,
        o: &mut TypeLocalEnv,
        m: &ClassEnv,
    ) -> ValueType {
        let class_type = self.object.analyze(errors, o, m);
        let class_name =
            if let ValueType::ClassValueType(ClassValueType { class_name }) = &class_type {
                class_name
            } else {
                let msg = error_member(&class_type);
                self.base_mut().error_msg = Some(msg);
                errors.push(error_from(self));
                return TYPE_OBJECT.clone();
            };

        let name = &self.member.name;
        if let Some(member) = m.get_attribute(class_name, name) {
            member.clone()
        } else {
            let msg = error_attribute(name, &class_type);
            self.base_mut().error_msg = Some(msg);
            errors.push(error_from(self));
            TYPE_OBJECT.clone()
        }
    }
}

impl CallExpr {
    pub fn analyze(
        &mut self,
        errors: &mut Vec<CompilerError>,
        o: &mut TypeLocalEnv,
        m: &ClassEnv,
    ) -> ValueType {
        let args: Vec<_> = self
            .args
            .iter_mut()
            .map(|arg| arg.analyze(errors, o, m))
            .collect();

        let function = match o.get(&self.function.name) {
            Some(EnvSlot::Func(f)) => f,
            _ => {
                let msg = error_function(&self.function.name);
                self.base_mut().error_msg = Some(msg);
                errors.push(error_from(self));
                return TYPE_OBJECT.clone();
            }
        };

        // Reference program: don't attach type to constructor
        if !m.contains(&self.function.name) {
            self.function.inferred_type = Some(function.clone());
        }

        if function.parameters.len() != args.len() {
            let msg = error_call_count(function.parameters.len(), args.len());
            self.base_mut().error_msg = Some(msg);
            errors.push(error_from(self));
        } else {
            for (i, arg) in args.into_iter().enumerate() {
                if !m.is_compatible(&arg, &function.parameters[i]) {
                    let msg = error_call_type(i, &function.parameters[i], &arg);
                    self.base_mut().error_msg = Some(msg);
                    errors.push(error_from(self));
                    break;
                }
            }
        }

        function.return_type.clone()
    }
}

impl MethodCallExpr {
    pub fn analyze(
        &mut self,
        errors: &mut Vec<CompilerError>,
        o: &mut TypeLocalEnv,
        m: &ClassEnv,
    ) -> ValueType {
        let args: Vec<_> = self
            .args
            .iter_mut()
            .map(|arg| arg.analyze(errors, o, m))
            .collect();

        let member = &mut self.method;
        let class = member.object.analyze(errors, o, m);
        let class_name = if let ValueType::ClassValueType(ClassValueType { class_name }) = class {
            class_name
        } else {
            let msg = error_member(&class);
            self.base_mut().error_msg = Some(msg);
            errors.push(error_from(self));
            return TYPE_OBJECT.clone();
        };

        let method_name = &member.member.name;

        let method = if let Some(method) = m.get_method(&class_name, method_name) {
            method
        } else {
            let msg = error_method(method_name, &class_name);
            self.base_mut().error_msg = Some(msg);
            errors.push(error_from(self));
            return TYPE_OBJECT.clone();
        };

        member.inferred_type = Some(method.clone());

        if method.parameters.len() - 1 != args.len() {
            let msg = error_call_count(method.parameters.len() - 1, args.len());
            self.base_mut().error_msg = Some(msg);
            errors.push(error_from(self));
        } else {
            for (i, arg) in args.into_iter().enumerate() {
                if !m.is_compatible(&arg, &method.parameters[i + 1]) {
                    let msg = error_call_type(i + 1, &method.parameters[i + 1], &arg);
                    self.base_mut().error_msg = Some(msg);
                    errors.push(error_from(self));
                    break;
                }
            }
        }

        method.return_type.clone()
    }
}

impl ReturnStmt {
    pub fn analyze(
        &mut self,
        errors: &mut Vec<CompilerError>,
        o: &mut TypeLocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) {
        // Reference program: do not analyze the expression on top-level return
        if let Some(return_expected) = r {
            let return_type = if let Some(value) = &mut self.value {
                value.analyze(errors, o, m)
            } else {
                TYPE_NONE.clone()
            };
            if !m.is_compatible(&return_type, return_expected) {
                // Reference program has some inconsistency here
                let msg = if self.value.is_some() {
                    error_assign(return_expected, &return_type)
                } else {
                    error_none_return(&return_expected)
                };
                self.base_mut().error_msg = Some(msg);
                errors.push(error_from(self));
            }
        } else {
            let msg = error_top_return();
            self.base_mut().error_msg = Some(msg);
            errors.push(error_from(self));
        }
    }
}

impl IfStmt {
    pub fn analyze(
        &mut self,
        errors: &mut Vec<CompilerError>,
        o: &mut TypeLocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) {
        let condition = self.condition.analyze(errors, o, m);
        if condition != *TYPE_BOOL {
            let msg = error_condition(&condition);
            self.base_mut().error_msg = Some(msg);
            errors.push(error_from(self));
        }

        analyze_stmt(&mut self.then_body, errors, o, m, r);
        analyze_stmt(&mut self.else_body, errors, o, m, r);
    }
}

impl WhileStmt {
    pub fn analyze(
        &mut self,
        errors: &mut Vec<CompilerError>,
        o: &mut TypeLocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) {
        let condition = self.condition.analyze(errors, o, m);
        if condition != *TYPE_BOOL {
            let msg = error_condition(&condition);
            self.base_mut().error_msg = Some(msg);
            errors.push(error_from(self));
        }

        analyze_stmt(&mut self.body, errors, o, m, r);
    }
}

impl ForStmt {
    pub fn analyze(
        &mut self,
        errors: &mut Vec<CompilerError>,
        o: &mut TypeLocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) {
        // Eh, the error handling is a mess in the reference program

        let iterable = self.iterable.analyze(errors, o, m);
        let element_type = if iterable == *TYPE_STR {
            Some(&iterable)
        } else if let ValueType::ListValueType(ListValueType { element_type }) = &iterable {
            Some(&**element_type)
        } else {
            let msg = error_iterable(&iterable);
            self.base_mut().error_msg = Some(msg);
            errors.push(error_from(self));
            None
        };

        if let Some(element_type) = element_type {
            let variable = match o.get(&self.identifier.name) {
                None | Some(EnvSlot::Func(_)) => None,
                Some(EnvSlot::Var(t, assignable)) => Some((t.clone(), assignable)),
            };

            if let Some((variable, Assignable(assignable))) = variable {
                if m.is_compatible(element_type, &variable) {
                    self.identifier.inferred_type = Some(variable); // yes, we attach the type here
                    if !assignable {
                        let msg = error_nonlocal_assign(&self.identifier.name);
                        self.identifier.base_mut().error_msg = Some(msg); // and this error is attached to the identifier
                        errors.push(error_from(&mut self.identifier));
                    }
                } else {
                    let msg = error_assign(&variable, element_type);
                    self.base_mut().error_msg = Some(msg);
                    errors.push(error_from(self));
                }
            } else {
                let msg = error_variable(&self.identifier.name);
                self.base_mut().error_msg = Some(msg);
                errors.push(error_from(self));
            }
        }

        analyze_stmt(&mut self.body, errors, o, m, r);
    }
}

fn analyze_stmt(
    statements: &mut [Stmt],
    errors: &mut Vec<CompilerError>,
    o: &mut TypeLocalEnv,
    m: &ClassEnv,
    r: Option<&ValueType>,
) {
    for statement in statements {
        match statement {
            Stmt::ExprStmt(s) => s.analyze(errors, o, m, r),
            Stmt::AssignStmt(s) => s.analyze(errors, o, m, r),
            Stmt::IfStmt(s) => s.analyze(errors, o, m, r),
            Stmt::ForStmt(s) => s.analyze(errors, o, m, r),
            Stmt::WhileStmt(s) => s.analyze(errors, o, m, r),
            Stmt::ReturnStmt(s) => s.analyze(errors, o, m, r),
        }
    }
}

fn analyze_decl(
    declarations: &mut [Declaration],
    errors: &mut Vec<CompilerError>,
    o: &mut TypeLocalEnv,
    m: &ClassEnv,
) {
    for declaration in declarations {
        match declaration {
            Declaration::ClassDef(s) => s.analyze(errors, o, m),
            Declaration::FuncDef(s) => s.analyze(errors, o, m),
            Declaration::VarDef(s) => s.analyze(errors, o, m),
            _ => (),
        }
    }
}

impl FuncDef {
    pub fn analyze(&mut self, errors: &mut Vec<CompilerError>, o: &mut TypeLocalEnv, m: &ClassEnv) {
        let frame: HashMap<String, LocalSlot<FuncType, ValueType>> = self
            .declarations
            .iter()
            .map(|decl| match decl {
                Declaration::FuncDef(f) => (
                    f.name.name.clone(),
                    LocalSlot::Func(FuncType {
                        parameters: f
                            .params
                            .iter()
                            .map(|tv| ValueType::from_annotation(&tv.type_))
                            .collect(),
                        return_type: ValueType::from_annotation(&f.return_type),
                    }),
                ),
                Declaration::VarDef(v) => (
                    v.var.identifier.name.clone(),
                    LocalSlot::Var(ValueType::from_annotation(&v.var.type_)),
                ),
                Declaration::GlobalDecl(v) => (v.variable.name.clone(), LocalSlot::Global),
                Declaration::NonLocalDecl(v) => (v.variable.name.clone(), LocalSlot::NonLocal),
                _ => panic!(),
            })
            .chain(self.params.iter().map(|param| {
                (
                    param.identifier.name.clone(),
                    LocalSlot::Var(ValueType::from_annotation(&param.type_)),
                )
            }))
            .collect();

        let mut handle = o.push(frame);
        analyze_decl(&mut self.declarations, errors, handle.inner(), m);

        let return_type = ValueType::from_annotation(&self.return_type);
        let r = Some(&return_type);
        analyze_stmt(&mut self.statements, errors, handle.inner(), m, r);
    }
}

impl ClassDef {
    pub fn analyze(&mut self, errors: &mut Vec<CompilerError>, o: &mut TypeLocalEnv, m: &ClassEnv) {
        analyze_decl(&mut self.declarations, errors, o, m);
    }
}

impl Program {
    pub fn analyze(&mut self, errors: &mut Vec<CompilerError>, o: &mut TypeLocalEnv, m: &ClassEnv) {
        analyze_decl(&mut self.declarations, errors, o, m);
        analyze_stmt(&mut self.statements, errors, o, m, None);
    }
}
