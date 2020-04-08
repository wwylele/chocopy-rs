use crate::node::*;

pub fn error_dup(name: &str) -> String {
    format!(
        "Duplicate declaration of identifier in same scope: {}",
        name
    )
}

pub fn error_super_undef(name: &str) -> String {
    format!("Super-class not defined: {}", name)
}

pub fn error_super_not_class(name: &str) -> String {
    format!("Super-class must be a class: {}", name)
}

pub fn error_super_special(name: &str) -> String {
    format!("Cannot extend special class: {}", name)
}

pub fn error_method_self(name: &str) -> String {
    format!(
        "First parameter of the following method must be of the enclosing class: {}",
        name
    )
}

pub fn error_method_override(name: &str) -> String {
    format!("Method overridden with different type signature: {}", name)
}

pub fn error_attribute_redefine(name: &str) -> String {
    format!("Cannot re-define attribute: {}", name)
}

pub fn error_invalid_type(name: &str) -> String {
    format!("Invalid type annotation; there is no class named: {}", name)
}

pub fn error_shadow(name: &str) -> String {
    format!("Cannot shadow class name: {}", name)
}

pub fn error_nonlocal(name: &str) -> String {
    format!("Not a nonlocal variable: {}", name)
}

pub fn error_global(name: &str) -> String {
    format!("Not a global variable: {}", name)
}

pub fn error_return(name: &str) -> String {
    format!(
        "All paths in this function/method must have a return statement: {}",
        name
    )
}

pub fn error_variable(name: &str) -> String {
    format!("Not a variable: {}", name)
}

pub fn error_assign(left: &ValueType, right: &ValueType) -> String {
    format!("Expected type `{}`; got type `{}`", &left, &right)
}

pub fn error_nonlocal_assign(name: &str) -> String {
    format!(
        "Cannot assign to variable that is not explicitly declared in this scope: {}",
        name
    )
}

pub fn error_unary(operator: &str, operand: &ValueType) -> String {
    format!("Cannot apply operator `{}` on type `{}`", operator, operand)
}

pub fn error_binary(operator: &str, left: &ValueType, right: &ValueType) -> String {
    format!(
        "Cannot apply operator `{}` on types `{}` and `{}`",
        operator, left, right
    )
}

pub fn error_condition(condition: &ValueType) -> String {
    format!("Condition expression cannot be of type `{}`", condition)
}

pub fn error_member(t: &ValueType) -> String {
    format!("Cannot access member of non-class type `{}`", t)
}

pub fn error_call_count(expected: usize, got: usize) -> String {
    format!("Expected {} arguments; got {}", expected, got)
}

pub fn error_call_type(location: usize, expected: &ValueType, got: &ValueType) -> String {
    format!(
        "Expected type `{}`; got type `{}` in parameter {}",
        expected, got, location,
    )
}

pub fn error_index_left(left: &ValueType) -> String {
    format!("Cannot index into type `{}`", &left)
}

pub fn error_index_right(index: &ValueType) -> String {
    format!("Index is of non-integer type `{}`", &index)
}

pub fn error_attribute(name: &str, class_name: &str) -> String {
    format!(
        "There is no attribute named `{}` in class `{}`",
        name, class_name
    )
}

pub fn error_function(name: &str) -> String {
    format!("Not a function or class: {}", name)
}

pub fn error_method(method_name: &str, class_name: &str) -> String {
    format!(
        "There is no method named `{}` in class `{}`",
        method_name, class_name
    )
}

pub fn error_none_return(return_expected: &ValueType) -> String {
    format!("Expected type `{}`; got `None`", &return_expected)
}

pub fn error_iterable(iterable: &ValueType) -> String {
    format!("Cannot iterate over value of type `{}`", &iterable)
}

pub fn error_multi_assign() -> String {
    "Right-hand side of multiple assignment may not be [<None>]".to_owned()
}

pub fn error_top_return() -> String {
    "Return statement cannot appear at the top level".to_owned()
}

pub fn error_str_index_assign() -> String {
    "`str` is not a list type".to_owned()
}
