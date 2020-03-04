use crate::location::*;
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

fn error_from(node: &impl Node) -> Error {
    let base = node.base();
    Error::CompilerError(CompilerError {
        base: NodeBase::from_location(base.location),
        message: base.error_msg.clone().unwrap(),
        syntax: false,
    })
}

pub fn check(mut ast: Ast) -> Ast {
    let mut errors = vec![];
    struct ClassInfo {
        super_class: String,
        items: HashMap<String, Type>,
    }

    let mut id_set = HashSet::new();
    id_set.insert("str".to_owned());
    id_set.insert("bool".to_owned());
    id_set.insert("int".to_owned());
    id_set.insert("print".to_owned());
    id_set.insert("input".to_owned());
    id_set.insert("len".to_owned());

    let mut classes = HashMap::new();
    classes.insert(
        "object".to_owned(),
        ClassInfo {
            super_class: "".to_owned(),
            items: std::iter::once((
                "__init__".to_owned(),
                Type::FuncType(FuncType {
                    parameters: vec![ValueType::ClassValueType(ClassValueType {
                        class_name: "object".to_owned(),
                    })],
                    return_type: ValueType::ClassValueType(ClassValueType {
                        class_name: "<None>".to_owned(),
                    }),
                }),
            ))
            .collect(),
        },
    );

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

                // Class scope collision check
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
