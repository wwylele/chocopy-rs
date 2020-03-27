use super::error::*;
use crate::node::*;
use std::collections::HashMap;
use std::collections::HashSet;

#[derive(Clone, PartialEq, Eq)]
enum Type {
    ValueType(ValueType),
    FuncType(FuncType),
}

struct ClassInfo {
    super_class: String,
    items: HashMap<String, Type>,
}

pub struct ClassEnv(HashMap<String, ClassInfo>);

impl ClassEnv {
    fn add_basic_type(&mut self, name: &str) {
        self.0.insert(
            name.to_owned(),
            ClassInfo {
                super_class: "object".to_owned(),
                items: std::iter::once((
                    "__init__".to_owned(),
                    Type::FuncType(FuncType {
                        parameters: vec![ValueType::ClassValueType(ClassValueType {
                            class_name: "object".to_owned(),
                        })],
                        return_type: TYPE_NONE.clone(),
                    }),
                ))
                .collect(),
            },
        );
    }

    pub fn new() -> ClassEnv {
        let mut class_env = ClassEnv(HashMap::new());
        class_env.add_basic_type("object");
        class_env
    }

    pub fn add_class(
        &mut self,
        class_def: &mut ClassDef,
        errors: &mut Vec<Error>,
        id_set: &HashSet<String>,
    ) {
        let class_name = &class_def.name.id().name;
        let super_name = &class_def.super_class.id().name;
        let super_class = if let Some(super_class) = self.0.get(super_name) {
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
            self.0.get("object").unwrap()
        };

        // Inherit items
        let mut items = super_class.items.clone();

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
                        Some(Type::FuncType(mut old)) => {
                            old.parameters[0] = ValueType::ClassValueType(ClassValueType {
                                class_name: class_name.clone(),
                            });
                            if Type::FuncType(old) != item_type {
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
                            Type::ValueType(ValueType::from_annotation(&var.var.tv().type_)),
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
        self.0.insert(
            class_name.clone(),
            ClassInfo {
                super_class: class_def.super_class.id().name.clone(),
                items,
            },
        );
    }

    pub fn complete_basic_types(&mut self) {
        self.add_basic_type("str");
        self.add_basic_type("int");
        self.add_basic_type("bool");
        self.add_basic_type("<None>");
        self.add_basic_type("<Empty>");
    }

    pub fn is_compatible(&self, sub_class: &ValueType, super_class: &ValueType) -> bool {
        if sub_class == super_class {
            return true;
        }
        if *super_class == *TYPE_OBJECT {
            return true;
        }
        if *sub_class == *TYPE_NONE {
            if let ValueType::ClassValueType(ClassValueType { class_name }) = super_class {
                return class_name != "int" && class_name != "str" && class_name != "bool";
            } else {
                return true;
            }
        }
        if *sub_class == *TYPE_EMPTY {
            if let ValueType::ListValueType(_) = super_class {
                return true;
            } else {
                return false;
            }
        }
        if *sub_class == *TYPE_NONE_LIST {
            if let ValueType::ListValueType(ListValueType { element_type }) = super_class {
                return self.is_compatible(&*TYPE_NONE, element_type);
            } else {
                return false;
            }
        }

        if *super_class == *TYPE_NONE || *super_class == *TYPE_EMPTY {
            return false;
        }

        let mut sub_name =
            if let ValueType::ClassValueType(ClassValueType { class_name }) = sub_class {
                class_name
            } else {
                return false;
            };

        let super_name =
            if let ValueType::ClassValueType(ClassValueType { class_name }) = super_class {
                class_name
            } else {
                return false;
            };

        loop {
            if sub_name == super_name {
                return true;
            }
            if sub_name == "object" {
                return false;
            }
            sub_name = &self.0.get(sub_name).unwrap().super_class;
        }
    }

    pub fn join(&self, a: &ValueType, b: &ValueType) -> ValueType {
        if self.is_compatible(a, b) {
            return b.clone();
        }
        if self.is_compatible(b, a) {
            return a.clone();
        }
        if let (
            ValueType::ClassValueType(ClassValueType {
                class_name: a_class,
            }),
            ValueType::ClassValueType(ClassValueType {
                class_name: b_class,
            }),
        ) = (a, b)
        {
            if a_class == "<None>"
                || a_class == "<Empty>"
                || b_class == "<None>"
                || b_class == "<Empty>"
            {
                return TYPE_OBJECT.clone();
            }

            let gen_chain = |mut t| {
                let mut v = vec![t];
                while t != "object" {
                    t = &self.0.get(t).unwrap().super_class;
                    v.push(t);
                }
                v
            };

            let mut a_chain = gen_chain(a_class);
            let mut b_chain = gen_chain(b_class);

            loop {
                let common = a_chain.pop().unwrap();
                b_chain.pop();
                if a_chain.last() != b_chain.last() {
                    return ValueType::ClassValueType(ClassValueType {
                        class_name: common.to_owned(),
                    });
                }
            }
        } else {
            TYPE_OBJECT.clone()
        }
    }

    pub fn get_attribute(&self, class_name: &str, name: &str) -> Option<&ValueType> {
        match self.0.get(class_name)?.items.get(name)? {
            Type::ValueType(t) => Some(t),
            _ => None,
        }
    }

    pub fn get_method(&self, class_name: &str, name: &str) -> Option<&FuncType> {
        match self.0.get(class_name)?.items.get(name)? {
            Type::FuncType(t) => Some(t),
            _ => None,
        }
    }

    pub fn contains(&self, class_name: &str) -> bool {
        self.0.contains_key(class_name)
    }
}
