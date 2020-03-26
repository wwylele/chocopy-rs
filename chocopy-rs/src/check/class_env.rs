use crate::node::*;
use std::collections::HashMap;

#[derive(Clone, PartialEq, Eq)]
pub enum Type {
    ValueType(ValueType),
    FuncType(FuncType),
}

pub struct ClassInfo {
    pub super_class: String,
    pub items: HashMap<String, Type>,
}

pub struct ClassEnv(pub HashMap<String, ClassInfo>);

impl ClassEnv {
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
