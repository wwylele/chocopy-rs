use crate::node::*;
use std::collections::{HashMap, HashSet};
use std::convert::*;

fn error_dup(name: &str) -> String {
    format!(
        "Duplicate declaration of identifier in same scope: {}",
        name
    )
}

fn error_super_undef(name: &str) -> String {
    format!("Super-class not defined: {}", name)
}

fn error_super_not_class(name: &str) -> String {
    format!("Super-class must be a class: {}", name)
}

fn error_super_special(name: &str) -> String {
    format!("Cannot extend special class: {}", name)
}

fn error_method_self(name: &str) -> String {
    format!(
        "First parameter of the following method must be of the enclosing class: {}",
        name
    )
}

fn error_method_override(name: &str) -> String {
    format!("Method overridden with different type signature: {}", name)
}

fn error_attribute_redefine(name: &str) -> String {
    format!("Cannot re-define attribute: {}", name)
}

fn error_invalid_type(name: &str) -> String {
    format!("Invalid type annotation; there is no class named: {}", name)
}

fn error_shadow(name: &str) -> String {
    format!("Cannot shadow class name: {}", name)
}

fn error_nonlocal(name: &str) -> String {
    format!("Not a nonlocal variable: {}", name)
}

fn error_global(name: &str) -> String {
    format!("Not a global variable: {}", name)
}

fn error_from(node: &impl Node) -> Error {
    let base = node.base();
    Error::CompilerError(CompilerError {
        base: NodeBase::from_location(base.location),
        message: base.error_msg.clone().unwrap(),
        syntax: false,
    })
}

struct ClassInfo {
    super_class: String,
    items: HashMap<String, Type>,
}

fn check_var_def(v: &mut VarDef, errors: &mut Vec<Error>, classes: &HashMap<String, ClassInfo>) {
    let tv = v.var.tv_mut();
    let core_type = tv.type_.core_type_mut();
    if !classes.contains_key(&core_type.class_name) {
        let msg = error_invalid_type(&core_type.class_name);
        core_type.base_mut().error_msg = Some(msg);
        errors.push(error_from(core_type));
    }
}

fn check_func(
    f: &mut FuncDef,
    errors: &mut Vec<Error>,
    classes: &HashMap<String, ClassInfo>,
    globals: &HashSet<String>,
    nonlocals: &HashSet<String>,
) {
    let mut locals = HashSet::new();
    let mut id_set = HashSet::new();
    // Check parameter type, collision and shadowing
    // semantic rule: 1(param), 2(param), 11(param)
    for param in &mut f.params {
        let core_type = param.tv_mut().type_.core_type_mut();
        if !classes.contains_key(&core_type.class_name) {
            let msg = error_invalid_type(&core_type.class_name);
            core_type.base_mut().error_msg = Some(msg);
            errors.push(error_from(core_type));
        }

        let id = param.tv_mut().identifier.id_mut();
        if classes.contains_key(&id.name) {
            let msg = error_shadow(&id.name);
            id.base_mut().error_msg = Some(msg);
            errors.push(error_from(id));
        }
        if id_set.contains(&id.name) {
            let msg = error_dup(&id.name);
            id.base_mut().error_msg = Some(msg);
            errors.push(error_from(id));
        }
        locals.insert(id.name.clone());
        id_set.insert(id.name.clone());
    }

    // Check return type
    // semantic rule: 11(return)
    let core_type = f.return_type.core_type_mut();
    if !classes.contains_key(&core_type.class_name) {
        let msg = error_invalid_type(&core_type.class_name);
        core_type.base_mut().error_msg = Some(msg);
        errors.push(error_from(core_type));
    }

    // semantic rule: 1, 2(local/function), 3, 11(local)
    for decl in &mut f.declarations {
        let name = decl.name_mut();
        if id_set.contains(&name.name) {
            let msg = error_dup(&name.name);
            name.base_mut().error_msg = Some(msg);
            errors.push(error_from(name));
        }
        id_set.insert(name.name.clone());

        match decl {
            Declaration::VarDef(v) => {
                let var = &mut v.var;
                let core_type = var.tv_mut().type_.core_type_mut();
                if !classes.contains_key(&core_type.class_name) {
                    let msg = error_invalid_type(&core_type.class_name);
                    core_type.base_mut().error_msg = Some(msg);
                    errors.push(error_from(core_type));
                }

                let id = var.tv_mut().identifier.id_mut();
                if classes.contains_key(&id.name) {
                    let msg = error_shadow(&id.name);
                    id.base_mut().error_msg = Some(msg);
                    errors.push(error_from(id));
                }
                locals.insert(id.name.clone());
            }
            Declaration::FuncDef(f) => {
                let id = f.name.id_mut();
                if classes.contains_key(&id.name) {
                    let msg = error_shadow(&id.name);
                    id.base_mut().error_msg = Some(msg);
                    errors.push(error_from(id));
                }
            }
            Declaration::NonLocalDecl(v) => {
                let id = v.variable.id_mut();
                if !nonlocals.contains(&id.name) {
                    let msg = error_nonlocal(&id.name);
                    id.base_mut().error_msg = Some(msg);
                    errors.push(error_from(id));
                }
            }
            Declaration::GlobalDecl(v) => {
                let id = v.variable.id_mut();
                if !globals.contains(&id.name) {
                    let msg = error_global(&id.name);
                    id.base_mut().error_msg = Some(msg);
                    errors.push(error_from(id));
                }
            }
            _ => unreachable!(),
        }
    }

    // TODO
    // semantic rule: 8, 9

    // recursion
    let nonlocals = nonlocals.union(&locals).cloned().collect();
    for decl in &mut f.declarations {
        if let Declaration::FuncDef(f) = decl {
            check_func(f, errors, classes, globals, &nonlocals);
        }
    }
}

pub fn check(mut ast: Ast) -> Ast {
    let mut errors = vec![];

    let mut id_set = HashSet::new();
    id_set.insert("str".to_owned());
    id_set.insert("bool".to_owned());
    id_set.insert("int".to_owned());
    id_set.insert("print".to_owned());
    id_set.insert("input".to_owned());
    id_set.insert("len".to_owned());

    let mut classes: HashMap<String, ClassInfo> = HashMap::new();

    let add_basic_type = |classes: &mut HashMap<String, ClassInfo>, name: &str| {
        classes.insert(
            name.to_owned(),
            ClassInfo {
                super_class: "object".to_owned(),
                items: std::iter::once((
                    "__init__".to_owned(),
                    Type::FuncType(FuncType {
                        parameters: vec![ValueType::ClassValueType(ClassValueType {
                            class_name: name.to_owned(),
                        })],
                        return_type: ValueType::ClassValueType(ClassValueType {
                            class_name: "<None>".to_owned(),
                        }),
                    }),
                ))
                .collect(),
            },
        );
    };

    add_basic_type(&mut classes, "object");

    // Pass A
    // semantic rule: 1(global/class), 4, 5, 6, 7
    // collects class info
    for decl in &mut ast.program_mut().declarations {
        // Global identifier collision check
        let name = decl.name_mut();
        if !id_set.insert(name.name.clone()) {
            let msg = error_dup(&name.name);
            name.base_mut().error_msg = Some(msg);
            errors.push(error_from(name));
        }

        if let Declaration::ClassDef(class_def) = decl {
            let class_name = &class_def.name.id().name;
            let super_name = &class_def.super_class.id().name;
            let super_class = if let Some(super_class) = classes.get(super_name) {
                super_class
            } else {
                let msg = if let "int" | "str" | "bool" = super_name.as_str() {
                    error_super_special
                } else if id_set.contains(super_name) {
                    error_super_not_class
                } else {
                    error_super_undef
                }(super_name);
                class_def.super_class.base_mut().error_msg = Some(msg);
                errors.push(error_from(&class_def.super_class));
                classes.get("object").unwrap()
            };

            // Inherit items and modify method self argument type
            let mut items = super_class.items.clone();
            for (_, item_type) in &mut items {
                if let Type::FuncType(FuncType { parameters, .. }) = item_type {
                    parameters[0] = ValueType::ClassValueType(ClassValueType {
                        class_name: class_name.clone(),
                    });
                }
            }

            // Check and insert new items
            let mut id_set = HashSet::new();
            for item_decl in &mut class_def.declarations {
                let name_str = item_decl.name_mut().name.clone();

                // Class scope identifier collision check
                if !id_set.insert(name_str.clone()) {
                    let msg = error_dup(&name_str);
                    let name = item_decl.name_mut();
                    name.base_mut().error_msg = Some(msg);
                    errors.push(error_from(name));
                    continue;
                }

                match item_decl {
                    Declaration::FuncDef(func) => {
                        let parameters: Vec<_> = func
                            .params
                            .iter()
                            .map(|t| ValueType::from_annotation(&t.tv().type_))
                            .collect();
                        let return_type = ValueType::from_annotation(&func.return_type);

                        let name = item_decl.name_mut();

                        // Self parameter check
                        if parameters.get(0)
                            != Some(&ValueType::ClassValueType(ClassValueType {
                                class_name: class_name.clone(),
                            }))
                        {
                            let msg = error_method_self(&name_str);
                            name.base_mut().error_msg = Some(msg);
                            errors.push(error_from(name));
                        }

                        let item_type = Type::FuncType(FuncType {
                            parameters,
                            return_type,
                        });

                        // Override check
                        match items.insert(name_str.clone(), item_type.clone()) {
                            None => (),
                            Some(t @ Type::FuncType(_)) => {
                                if t != item_type {
                                    let msg = error_method_override(&name_str);
                                    name.base_mut().error_msg = Some(msg);
                                    errors.push(error_from(name));
                                }
                            }
                            _ => {
                                let msg = error_attribute_redefine(&name_str);
                                name.base_mut().error_msg = Some(msg);
                                errors.push(error_from(name));
                            }
                        }
                    }
                    Declaration::VarDef(var) => {
                        // Redefinition check
                        if items
                            .insert(
                                name_str.clone(),
                                ValueType::from_annotation(&var.var.tv().type_).into(),
                            )
                            .is_some()
                        {
                            let name = item_decl.name_mut();
                            let msg = error_attribute_redefine(&name_str);
                            name.base_mut().error_msg = Some(msg);
                            errors.push(error_from(name));
                        }
                    }
                    _ => unreachable!(),
                }
            }
            classes.insert(
                class_name.clone(),
                ClassInfo {
                    super_class: class_def.super_class.id().name.clone(),
                    items,
                },
            );
        }
    }

    add_basic_type(&mut classes, "str");
    add_basic_type(&mut classes, "int");
    add_basic_type(&mut classes, "bool");
    add_basic_type(&mut classes, "<None>");

    // Pass B
    // semantic rules: 11(global/class variable)
    // collects global variables
    let mut globals = HashSet::new();
    for decl in &mut ast.program_mut().declarations {
        if let Declaration::VarDef(v) = decl {
            check_var_def(v, &mut errors, &classes);
            globals.insert(v.var.tv().identifier.id().name.clone());
        } else if let Declaration::ClassDef(c) = decl {
            for decl in &mut c.declarations {
                if let Declaration::VarDef(v) = decl {
                    check_var_def(v, &mut errors, &classes);
                }
            }
        }
    }

    // Pass C
    // semantic rules: 1(function), 2, 3, 8, 9, 11(function)
    for decl in &mut ast.program_mut().declarations {
        if let Declaration::FuncDef(f) = decl {
            check_func(f, &mut errors, &classes, &globals, &HashSet::new())
        } else if let Declaration::ClassDef(c) = decl {
            for decl in &mut c.declarations {
                if let Declaration::FuncDef(f) = decl {
                    check_func(f, &mut errors, &classes, &globals, &HashSet::new())
                }
            }
        }
    }

    ast.program_mut().errors = ErrorInfo::Errors(Errors {
        base: NodeBase::new(0, 0, 0, 0),
        errors,
    });
    ast
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{stdout, Write};
    #[test]
    fn sample() {
        let mut passed = true;
        let test_dirs = ["../chocopy-wars/src/test/data/pa2/sample"];
        for dir in &test_dirs {
            println!("Testing Directory {}", dir);
            let mut files = std::fs::read_dir(dir)
                .unwrap()
                .map(|f| f.unwrap())
                .filter(|f| f.file_name().to_str().unwrap().ends_with(".ast"))
                .map(|f| f.path())
                .collect::<Vec<_>>();
            files.sort();

            for ast_file in files {
                let mut typed_file = ast_file.clone();
                let mut file_name = ast_file.file_name().unwrap().to_owned();
                print!("Testing {} ---- ", file_name.to_str().unwrap());
                stdout().flush().unwrap();
                file_name.push(".typed");
                typed_file.set_file_name(file_name);
                let ast_string = String::from_utf8(std::fs::read(ast_file).unwrap()).unwrap();
                let typed_string = String::from_utf8(std::fs::read(typed_file).unwrap()).unwrap();
                let ast = serde_json::from_str::<Ast>(&ast_string).unwrap();
                let typed = serde_json::from_str::<Ast>(&typed_string).unwrap();
                if check(ast) == typed {
                    println!("\x1b[32mOK\x1b[0m");
                } else {
                    println!("\x1b[31mError\x1b[0m");
                    //passed = false;
                }
            }
        }
        assert_eq!(passed, true);
    }
}
