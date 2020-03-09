use crate::node::*;
use std::collections::HashMap;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Assignable(pub bool);
pub struct FrameHandle<'a>(&'a mut LocalEnv);

impl<'a> FrameHandle<'a> {
    pub fn inner(&mut self) -> &mut LocalEnv {
        self.0
    }
}

impl<'a> Drop for FrameHandle<'a> {
    fn drop(&mut self) {
        (self.0).0.pop();
    }
}

impl LocalEnv {
    pub fn get(&self, name: &str) -> Option<(Type, Assignable)> {
        match self.0.last().unwrap().get(name) {
            Some(EnvSlot::Local(t)) => Some((t.clone().into(), Assignable(true))),
            Some(EnvSlot::Func(t)) => Some((Type::FuncType(t.clone()), Assignable(false))),
            Some(EnvSlot::Global) => {
                let t = if let Some(EnvSlot::Local(t)) = self.0[0].get(name) {
                    t.clone()
                } else {
                    panic!()
                };
                Some((t.into(), Assignable(true)))
            }
            s @ Some(EnvSlot::NonLocal) | s @ None => {
                for frame in self.0[0..self.0.len() - 1].iter().rev() {
                    match frame.get(name) {
                        Some(EnvSlot::NonLocal) | None => (),
                        Some(EnvSlot::Global) => {
                            assert!(s.is_none());
                            let t = if let Some(EnvSlot::Local(t)) = self.0[0].get(name) {
                                t.clone()
                            } else {
                                panic!()
                            };
                            return Some((t.into(), Assignable(false)));
                        }
                        Some(EnvSlot::Local(t)) => {
                            return Some((t.clone().into(), Assignable(s.is_some())))
                        }
                        Some(EnvSlot::Func(t)) => {
                            assert!(s.is_none());
                            return Some((Type::FuncType(t.clone()), Assignable(false)));
                        }
                    }
                }
                None
            }
        }
    }

    pub fn push(&mut self, frame: HashMap<String, EnvSlot>) -> FrameHandle {
        self.0.push(frame);
        FrameHandle(self)
    }
}
