mod analyze;
mod class_env;
mod error;

use crate::local_env::*;
use crate::node::*;
use class_env::*;
use error::*;
use std::collections::{HashMap, HashSet};

fn check_var_def(v: &mut VarDef, errors: &mut Vec<CompilerError>, classes: &ClassEnv) {
    let core_type = v.var.type_.core_type_mut();
    if !classes.contains(&core_type.class_name) {
        let msg = error_invalid_type(&core_type.class_name);
        core_type.add_error(errors, msg);
    }
}

fn always_return(statements: &[Stmt]) -> bool {
    for statement in statements {
        match statement {
            Stmt::ReturnStmt(_) => return true,
            Stmt::IfStmt(IfStmt {
                then_body,
                else_body,
                ..
            }) => {
                if always_return(then_body) && always_return(else_body) {
                    return true;
                }
            }
            _ => (),
        }
    }
    false
}

fn check_func(
    f: &mut FuncDef,
    errors: &mut Vec<CompilerError>,
    classes: &ClassEnv,
    globals: &HashSet<String>,
    nonlocals: &HashSet<String>,
) {
    let mut locals = HashSet::new();
    let mut id_set = HashSet::new();
    // Check parameter type, collision and shadowing
    // semantic rule: 1(param), 2(param), 11(param)
    for param in &mut f.params {
        let core_type = param.type_.core_type_mut();
        if !classes.contains(&core_type.class_name) {
            let msg = error_invalid_type(&core_type.class_name);
            core_type.add_error(errors, msg);
        }

        let id = &mut param.identifier;
        if classes.contains(&id.name) {
            let msg = error_shadow(&id.name);
            id.add_error(errors, msg);
        }
        if id_set.contains(&id.name) {
            let msg = error_dup(&id.name);
            id.add_error(errors, msg);
        }
        locals.insert(id.name.clone());
        id_set.insert(id.name.clone());
    }

    // Check return type
    // semantic rule: 11(return)
    let core_type = f.return_type.core_type_mut();
    if !classes.contains(&core_type.class_name) {
        let msg = error_invalid_type(&core_type.class_name);
        core_type.add_error(errors, msg);
    }

    let mut nonlocal_remove = HashSet::new();
    // semantic rule: 1, 2(local/function), 3, 11(local)
    for decl in &mut f.declarations {
        let name = decl.name_mut();
        if id_set.contains(&name.name) {
            let msg = error_dup(&name.name);
            name.add_error(errors, msg);
        }
        id_set.insert(name.name.clone());

        match decl {
            Declaration::VarDef(v) => {
                let var = &mut v.var;
                let core_type = var.type_.core_type_mut();
                if !classes.contains(&core_type.class_name) {
                    let msg = error_invalid_type(&core_type.class_name);
                    core_type.add_error(errors, msg);
                }

                let id = &mut var.identifier;
                if classes.contains(&id.name) {
                    let msg = error_shadow(&id.name);
                    id.add_error(errors, msg);
                }
                locals.insert(id.name.clone());
            }
            Declaration::FuncDef(f) => {
                let id = &mut f.name;
                if classes.contains(&id.name) {
                    let msg = error_shadow(&id.name);
                    id.add_error(errors, msg);
                }
                nonlocal_remove.insert(id.name.clone());
            }
            Declaration::NonLocalDecl(v) => {
                let id = &mut v.variable;
                if !nonlocals.contains(&id.name) {
                    let msg = error_nonlocal(&id.name);
                    id.add_error(errors, msg);
                }
            }
            Declaration::GlobalDecl(v) => {
                let id = &mut v.variable;
                if !globals.contains(&id.name) {
                    let msg = error_global(&id.name);
                    id.add_error(errors, msg);
                }
                nonlocal_remove.insert(id.name.clone());
            }
            _ => unreachable!(),
        }
    }

    // semantic rule: 9
    if let TypeAnnotation::ClassType(c) = &f.return_type {
        if let "int" | "str" | "bool" = c.class_name.as_str() {
            if !always_return(&f.statements) {
                let msg = error_return(&f.name.name);
                f.name.add_error(errors, msg);
            }
        }
    }

    // recursion
    let nonlocals = nonlocals
        .union(&locals)
        .filter(|v| !nonlocal_remove.contains(*v))
        .cloned()
        .collect();
    for decl in &mut f.declarations {
        if let Declaration::FuncDef(f) = decl {
            check_func(f, errors, classes, globals, &nonlocals);
        }
    }
}

pub fn check(mut ast: Program) -> Program {
    let mut errors = vec![];

    let mut id_set = HashSet::new();
    id_set.insert("str".to_owned());
    id_set.insert("bool".to_owned());
    id_set.insert("int".to_owned());
    id_set.insert("object".to_owned());
    id_set.insert("print".to_owned());
    id_set.insert("input".to_owned());
    id_set.insert("len".to_owned());

    let mut classes = ClassEnv::new();

    // Pass A
    // semantic rule: 1(global/class), 4, 5, 6, 7
    // collects class info
    for decl in &mut ast.declarations {
        // Global identifier collision check
        let name = decl.name_mut();
        if !id_set.insert(name.name.clone()) {
            let msg = error_dup(&name.name);
            name.add_error(&mut errors, msg);
        }

        if let Declaration::ClassDef(class_def) = decl {
            classes.add_class(class_def, &mut errors, &id_set);
        }
    }

    // Pass B
    // semantic rules: 11(global/class variable)
    // collects global variables
    let mut globals = HashSet::new();
    for decl in &mut ast.declarations {
        if let Declaration::VarDef(v) = decl {
            check_var_def(v, &mut errors, &classes);
            let name = &v.var.identifier.name;
            globals.insert(name.clone());
        } else if let Declaration::ClassDef(c) = decl {
            for decl in &mut c.declarations {
                if let Declaration::VarDef(v) = decl {
                    check_var_def(v, &mut errors, &classes);
                }
            }
        }
    }

    let mut global_env: HashMap<String, LocalSlot<FuncType, ValueType>> = HashMap::new();
    global_env.insert(
        "print".to_owned(),
        LocalSlot::Func(FuncType {
            parameters: vec![TYPE_OBJECT.clone()],
            return_type: TYPE_NONE.clone(),
        }),
    );
    global_env.insert(
        "input".to_owned(),
        LocalSlot::Func(FuncType {
            parameters: vec![],
            return_type: TYPE_STR.clone(),
        }),
    );
    global_env.insert(
        "len".to_owned(),
        LocalSlot::Func(FuncType {
            parameters: vec![TYPE_OBJECT.clone()],
            return_type: TYPE_INT.clone(),
        }),
    );

    global_env.insert(
        "int".to_owned(),
        LocalSlot::Func(FuncType {
            parameters: vec![],
            return_type: ValueType::ClassValueType(ClassValueType {
                class_name: "int".to_owned(),
            }),
        }),
    );

    global_env.insert(
        "bool".to_owned(),
        LocalSlot::Func(FuncType {
            parameters: vec![],
            return_type: ValueType::ClassValueType(ClassValueType {
                class_name: "bool".to_owned(),
            }),
        }),
    );

    global_env.insert(
        "str".to_owned(),
        LocalSlot::Func(FuncType {
            parameters: vec![],
            return_type: ValueType::ClassValueType(ClassValueType {
                class_name: "str".to_owned(),
            }),
        }),
    );

    global_env.insert(
        "object".to_owned(),
        LocalSlot::Func(FuncType {
            parameters: vec![],
            return_type: ValueType::ClassValueType(ClassValueType {
                class_name: "object".to_owned(),
            }),
        }),
    );

    // Pass C
    // semantic rules: 1(function), 2, 3, 9, 11(function)
    // collects global environment
    for decl in &mut ast.declarations {
        if let Declaration::FuncDef(f) = decl {
            check_func(f, &mut errors, &classes, &globals, &HashSet::new());
            global_env.insert(
                f.name.name.clone(),
                LocalSlot::Func(FuncType {
                    parameters: f
                        .params
                        .iter()
                        .map(|tv| ValueType::from_annotation(&tv.type_))
                        .collect(),
                    return_type: ValueType::from_annotation(&f.return_type),
                }),
            );
        } else if let Declaration::ClassDef(c) = decl {
            for decl in &mut c.declarations {
                if let Declaration::FuncDef(f) = decl {
                    check_func(f, &mut errors, &classes, &globals, &HashSet::new())
                }
            }
            let name = &c.name.name;
            global_env.insert(
                name.clone(),
                LocalSlot::Func(FuncType {
                    parameters: vec![],
                    return_type: ValueType::ClassValueType(ClassValueType {
                        class_name: name.clone(),
                    }),
                }),
            );
        } else if let Declaration::VarDef(v) = decl {
            let name = &v.var.identifier.name;
            global_env.insert(
                name.clone(),
                LocalSlot::Var(ValueType::from_annotation(&v.var.type_)),
            );
        }
    }

    // Pass D
    // semantic rules: 8, 10
    // and type checking
    if errors.is_empty() {
        let mut env = LocalEnv::new(global_env);
        ast.analyze(&mut errors, &mut env, &classes);
    }

    ast.errors = Errors {
        base: NodeBase::new(0, 0, 0, 0),
        errors,
    };
    ast
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{stdout, Write};
    #[test]
    fn sample() {
        let mut passed = true;
        let test_dirs = ["test/pa2", "test/original/pa2"];
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
                let ast = serde_json::from_str::<Program>(&ast_string).unwrap();
                let mut typed = serde_json::from_str::<Program>(&typed_string).unwrap();
                let mut result = check(ast);
                result.errors.sort();
                typed.errors.sort();
                if result == typed {
                    println!("\x1b[32mOK\x1b[0m");
                } else {
                    println!("\x1b[31mError\x1b[0m");
                    passed = false;
                }
            }
        }
        assert_eq!(passed, true);
    }
}
