use super::error::*;
use super::local_env::*;
use crate::node::*;
use std::collections::HashMap;
use std::convert::*;

// Only for variable
impl Identifier {
    pub fn analyze_impl(
        &mut self,
        errors: &mut Vec<Error>,
        o: &mut LocalEnv,
        _m: &ClassEnv,
        _r: Option<&ValueType>,
    ) -> Option<ValueType> {
        match o.get(&self.name) {
            None | Some((Type::FuncType(_), _)) => {
                let msg = error_variable(&self.name);
                self.base_mut().error_msg = Some(msg);
                errors.push(error_from(self));
                Some(TYPE_OBJECT.clone())
            }
            Some((t, _)) => Some(t.try_into().unwrap()),
        }
    }
}

impl AssignStmt {
    pub fn analyze_impl(
        &mut self,
        errors: &mut Vec<Error>,
        o: &mut LocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) -> Option<ValueType> {
        let right: ValueType = self.value.analyze(errors, o, m, r).unwrap();

        // We don't do `for target in &mut self.targets` because of mut ref conflict
        for i in 0..self.targets.len() {
            let left: ValueType = self.targets[i].analyze(errors, o, m, r).unwrap();
            if let ExprContent::Identifier(Identifier { name, .. }) = &self.targets[i].content {
                if let Some((_, Assignable(false))) = o.get(name) {
                    let msg = error_nonlocal_assign(name);
                    self.targets[i].base_mut().error_msg = Some(msg);
                    errors.push(error_from(&self.targets[i]));
                }
            } else if let ExprContent::IndexExpr(index_expr) = &self.targets[i].content {
                if index_expr.list.inferred_type.as_ref().unwrap() == &TYPE_STR.clone().into() {
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

        None
    }
}

impl VarDef {
    pub fn analyze_impl(
        &mut self,
        errors: &mut Vec<Error>,
        o: &mut LocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) -> Option<ValueType> {
        let right = self.value.analyze(errors, o, m, r).unwrap();
        let left = ValueType::from_annotation(&self.var.tv().type_);
        if !m.is_compatible(&right, &left) {
            let msg = error_assign(&left, &right);
            self.base_mut().error_msg = Some(msg);
            errors.push(error_from(self));
        }
        None
    }
}

impl ExprStmt {
    pub fn analyze_impl(
        &mut self,
        errors: &mut Vec<Error>,
        o: &mut LocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) -> Option<ValueType> {
        self.expr.analyze(errors, o, m, r)
    }
}

impl BooleanLiteral {
    pub fn analyze_impl(
        &mut self,
        _errors: &mut Vec<Error>,
        _o: &mut LocalEnv,
        _m: &ClassEnv,
        _r: Option<&ValueType>,
    ) -> Option<ValueType> {
        Some(TYPE_BOOL.clone())
    }
}

impl IntegerLiteral {
    pub fn analyze_impl(
        &mut self,
        _errors: &mut Vec<Error>,
        _o: &mut LocalEnv,
        _m: &ClassEnv,
        _r: Option<&ValueType>,
    ) -> Option<ValueType> {
        Some(TYPE_INT.clone())
    }
}

impl StringLiteral {
    pub fn analyze_impl(
        &mut self,
        _errors: &mut Vec<Error>,
        _o: &mut LocalEnv,
        _m: &ClassEnv,
        _r: Option<&ValueType>,
    ) -> Option<ValueType> {
        Some(TYPE_STR.clone())
    }
}

impl NoneLiteral {
    pub fn analyze_impl(
        &mut self,
        _errors: &mut Vec<Error>,
        _o: &mut LocalEnv,
        _m: &ClassEnv,
        _r: Option<&ValueType>,
    ) -> Option<ValueType> {
        Some(TYPE_NONE.clone())
    }
}

impl UnaryExpr {
    pub fn analyze_impl(
        &mut self,
        errors: &mut Vec<Error>,
        o: &mut LocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) -> Option<ValueType> {
        let operand: ValueType = self.operand.analyze(errors, o, m, r).unwrap();
        match self.operator {
            UnaryOp::Negative => {
                if operand != *TYPE_INT {
                    let msg = error_unary("-", &operand);
                    self.base_mut().error_msg = Some(msg);
                    errors.push(error_from(self));
                }
                Some(TYPE_INT.clone())
            }
            UnaryOp::Not => {
                if operand != *TYPE_BOOL {
                    let msg = error_unary("not", &operand);
                    self.base_mut().error_msg = Some(msg);
                    errors.push(error_from(self));
                }
                Some(TYPE_BOOL.clone())
            }
        }
    }
}

impl BinaryExpr {
    pub fn analyze_impl(
        &mut self,
        errors: &mut Vec<Error>,
        o: &mut LocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) -> Option<ValueType> {
        let left: ValueType = self.left.analyze(errors, o, m, r).unwrap();
        let right: ValueType = self.right.analyze(errors, o, m, r).unwrap();

        let mut error = false;
        let output = match self.operator {
            BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                if left != *TYPE_INT || right != *TYPE_INT {
                    error = true;
                }
                Some(TYPE_INT.clone())
            }
            BinaryOp::Or | BinaryOp::And => {
                if left != *TYPE_BOOL || right != *TYPE_BOOL {
                    error = true;
                }
                Some(TYPE_BOOL.clone())
            }
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                if left != *TYPE_INT || right != *TYPE_INT {
                    error = true;
                }
                Some(TYPE_BOOL.clone())
            }
            BinaryOp::Is => {
                let is_basic =
                    |t: &ValueType| *t == *TYPE_INT || *t == *TYPE_BOOL || *t == *TYPE_STR;
                if is_basic(&left) || is_basic(&right) {
                    error = true;
                }
                Some(TYPE_BOOL.clone())
            }
            BinaryOp::Add => {
                if left == *TYPE_INT || right == *TYPE_INT {
                    if left != right {
                        error = true;
                    }
                    Some(TYPE_INT.clone())
                } else if left == *TYPE_STR {
                    if left != right {
                        error = true;
                        Some(TYPE_OBJECT.clone())
                    } else {
                        Some(TYPE_STR.clone())
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
                    Some(ValueType::ListValueType(ListValueType { element_type }))
                } else {
                    error = true;
                    Some(TYPE_OBJECT.clone())
                }
            }
            BinaryOp::Eq | BinaryOp::Ne => {
                if left != *TYPE_INT && left != *TYPE_STR && left != *TYPE_BOOL {
                    error = true
                } else if left != right {
                    error = true
                }
                Some(TYPE_BOOL.clone())
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
    pub fn analyze_impl(
        &mut self,
        errors: &mut Vec<Error>,
        o: &mut LocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) -> Option<ValueType> {
        let condition = self.condition.analyze(errors, o, m, r).unwrap();
        if condition != *TYPE_BOOL {
            let msg = error_condition(&condition);
            self.base_mut().error_msg = Some(msg);
            errors.push(error_from(self))
        }
        let then_type = self.then_expr.analyze(errors, o, m, r).unwrap();
        let else_type = self.else_expr.analyze(errors, o, m, r).unwrap();
        Some(m.join(&then_type, &else_type))
    }
}

impl ListExpr {
    pub fn analyze_impl(
        &mut self,
        errors: &mut Vec<Error>,
        o: &mut LocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) -> Option<ValueType> {
        if self.elements.len() == 0 {
            return Some(TYPE_EMPTY.clone());
        }
        let mut element_type = self.elements[0].analyze(errors, o, m, r).unwrap();
        for element in self.elements.iter_mut().skip(1) {
            element_type = m.join(&element_type, &element.analyze(errors, o, m, r).unwrap());
        }

        let element_type = Box::new(element_type);
        Some(ValueType::ListValueType(ListValueType { element_type }))
    }
}

impl IndexExpr {
    pub fn analyze_impl(
        &mut self,
        errors: &mut Vec<Error>,
        o: &mut LocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) -> Option<ValueType> {
        let left = self.list.analyze(errors, o, m, r).unwrap();
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

        let index = self.index.analyze(errors, o, m, r).unwrap();
        if index != *TYPE_INT && self.base().error_msg.is_none() {
            let msg = error_index_right(&index);
            self.base_mut().error_msg = Some(msg);
            errors.push(error_from(self));
        }

        Some(element_type)
    }
}

impl MemberExpr {
    pub fn analyze_impl(
        &mut self,
        errors: &mut Vec<Error>,
        o: &mut LocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) -> Option<ValueType> {
        let class_type = self.object.analyze(errors, o, m, r).unwrap();
        let class_info =
            if let ValueType::ClassValueType(ClassValueType { class_name }) = &class_type {
                m.get(class_name)
            } else {
                let msg = error_member(&class_type);
                self.base_mut().error_msg = Some(msg);
                errors.push(error_from(self));
                return Some(TYPE_OBJECT.clone());
            };

        let name = &self.member.id().name;

        let member = class_info
            .items
            .get(name)
            .cloned()
            .map(TryInto::try_into)
            .map(Result::ok)
            .flatten();

        if member.is_none() {
            let msg = error_attribute(name, &class_type);
            self.base_mut().error_msg = Some(msg);
            errors.push(error_from(self));
            return Some(TYPE_OBJECT.clone());
        }

        member
    }
}

impl CallExpr {
    pub fn analyze_impl(
        &mut self,
        errors: &mut Vec<Error>,
        o: &mut LocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) -> Option<ValueType> {
        let args: Vec<_> = self
            .args
            .iter_mut()
            .map(|arg| arg.analyze(errors, o, m, r).unwrap())
            .collect();

        let id = self.function.id_mut();
        let function = match o.get(&id.name) {
            Some((Type::FuncType(f), _)) => f,
            _ => {
                let msg = error_function(&id.name);
                self.base_mut().error_msg = Some(msg);
                errors.push(error_from(self));
                return Some(TYPE_OBJECT.clone());
            }
        };

        // Reference program: don't attach type to constructor
        if !m.contains(&id.name) {
            id.inferred_type = Some(Type::FuncType(function.clone()));
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

        Some(function.return_type.clone())
    }
}

impl MethodCallExpr {
    pub fn analyze_impl(
        &mut self,
        errors: &mut Vec<Error>,
        o: &mut LocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) -> Option<ValueType> {
        let args: Vec<_> = self
            .args
            .iter_mut()
            .map(|arg| arg.analyze(errors, o, m, r).unwrap())
            .collect();

        let member = self.method.member_mut();
        let class = member.object.analyze(errors, o, m, r).unwrap();
        let class_name = if let ValueType::ClassValueType(ClassValueType { class_name }) = class {
            class_name
        } else {
            let msg = error_member(&class);
            self.base_mut().error_msg = Some(msg);
            errors.push(error_from(self));
            return Some(TYPE_OBJECT.clone());
        };

        let method_name = &member.member.id().name;

        let method = match m.get(&class_name).items.get(method_name) {
            Some(Type::FuncType(f)) => f,
            _ => {
                let msg = error_method(method_name, &class_name);
                self.base_mut().error_msg = Some(msg);
                errors.push(error_from(self));
                return Some(TYPE_OBJECT.clone());
            }
        };

        member.inferred_type = Some(Type::FuncType(method.clone()));

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

        Some(method.return_type.clone())
    }
}

impl ReturnStmt {
    pub fn analyze_impl(
        &mut self,
        errors: &mut Vec<Error>,
        o: &mut LocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) -> Option<ValueType> {
        // Reference program: do not analyze the expression on top-level return
        if let Some(return_expected) = r {
            let return_type = if let Some(value) = &mut self.value {
                value.analyze(errors, o, m, r).unwrap()
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

        None
    }
}

fn analyze(
    statements: &mut [Stmt],
    errors: &mut Vec<Error>,
    o: &mut LocalEnv,
    m: &ClassEnv,
    r: Option<&ValueType>,
) {
    for statement in statements {
        statement.analyze(errors, o, m, r);
    }
}

impl IfStmt {
    pub fn analyze_impl(
        &mut self,
        errors: &mut Vec<Error>,
        o: &mut LocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) -> Option<ValueType> {
        let condition = self.condition.analyze(errors, o, m, r).unwrap();
        if condition != *TYPE_BOOL {
            let msg = error_condition(&condition);
            self.base_mut().error_msg = Some(msg);
            errors.push(error_from(self));
        }

        analyze(&mut self.then_body, errors, o, m, r);
        analyze(&mut self.else_body, errors, o, m, r);

        None
    }
}

impl WhileStmt {
    pub fn analyze_impl(
        &mut self,
        errors: &mut Vec<Error>,
        o: &mut LocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) -> Option<ValueType> {
        let condition = self.condition.analyze(errors, o, m, r).unwrap();
        if condition != *TYPE_BOOL {
            let msg = error_condition(&condition);
            self.base_mut().error_msg = Some(msg);
            errors.push(error_from(self));
        }

        analyze(&mut self.body, errors, o, m, r);

        None
    }
}

impl ForStmt {
    pub fn analyze_impl(
        &mut self,
        errors: &mut Vec<Error>,
        o: &mut LocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) -> Option<ValueType> {
        // Eh, the error handling is a mess in the reference program

        let iterable = self.iterable.analyze(errors, o, m, r).unwrap();
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
            let id = self.identifier.id_mut();
            let variable = match o.get(&id.name) {
                None | Some((Type::FuncType(_), _)) => None,
                Some((t, assignable)) => Some((t.try_into().unwrap(), assignable)),
            };

            if let Some((variable, Assignable(assignable))) = variable {
                if m.is_compatible(element_type, &variable) {
                    id.inferred_type = Some(variable.into()); // yes, we attach the type here
                    if !assignable {
                        let msg = error_nonlocal_assign(&id.name);
                        id.base_mut().error_msg = Some(msg); // and this error is attached to the identifier
                        errors.push(error_from(id));
                    }
                } else {
                    let msg = error_assign(&variable, element_type);
                    self.base_mut().error_msg = Some(msg);
                    errors.push(error_from(self));
                }
            } else {
                let msg = error_variable(&id.name);
                self.base_mut().error_msg = Some(msg);
                errors.push(error_from(self));
            }
        }

        analyze(&mut self.body, errors, o, m, r);

        None
    }
}

impl FuncDef {
    pub fn analyze_impl(
        &mut self,
        errors: &mut Vec<Error>,
        o: &mut LocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) -> Option<ValueType> {
        let frame: HashMap<String, EnvSlot> = self
            .declarations
            .iter()
            .map(|decl| match decl {
                Declaration::FuncDef(f) => (
                    f.name.id().name.clone(),
                    EnvSlot::Func(FuncType {
                        parameters: f
                            .params
                            .iter()
                            .map(|tv| ValueType::from_annotation(&tv.tv().type_))
                            .collect(),
                        return_type: ValueType::from_annotation(&f.return_type),
                    }),
                ),
                Declaration::VarDef(v) => {
                    let tv = v.var.tv();
                    (
                        tv.identifier.id().name.clone(),
                        EnvSlot::Local(ValueType::from_annotation(&tv.type_)),
                    )
                }
                Declaration::GlobalDecl(v) => (v.variable.id().name.clone(), EnvSlot::Global),
                Declaration::NonLocalDecl(v) => (v.variable.id().name.clone(), EnvSlot::NonLocal),
                _ => panic!(),
            })
            .chain(self.params.iter().map(|param| {
                let tv = param.tv();
                (
                    tv.identifier.id().name.clone(),
                    EnvSlot::Local(ValueType::from_annotation(&tv.type_)),
                )
            }))
            .collect();

        let mut handle = o.push(frame);
        for decl in &mut self.declarations {
            decl.analyze(errors, handle.inner(), m, r);
        }
        analyze(
            &mut self.statements,
            errors,
            handle.inner(),
            m,
            Some(&ValueType::from_annotation(&self.return_type)),
        );

        None
    }
}

impl ClassDef {
    pub fn analyze_impl(
        &mut self,
        errors: &mut Vec<Error>,
        o: &mut LocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) -> Option<ValueType> {
        for decl in &mut self.declarations {
            decl.analyze(errors, o, m, r);
        }

        None
    }
}

impl Program {
    pub fn analyze_impl(
        &mut self,
        errors: &mut Vec<Error>,
        o: &mut LocalEnv,
        m: &ClassEnv,
        r: Option<&ValueType>,
    ) -> Option<ValueType> {
        for decl in &mut self.declarations {
            decl.analyze(errors, o, m, r);
        }
        analyze(&mut self.statements, errors, o, m, r);
        None
    }
}
