use std::collections::HashMap;

pub struct LocalEnv<F, V>(Vec<HashMap<String, LocalSlot<F, V>>>);
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Assignable(pub bool);
pub struct FrameHandle<'a, F, V>(&'a mut LocalEnv<F, V>);

impl<'a, F, V> FrameHandle<'a, F, V> {
    pub fn inner(&mut self) -> &mut LocalEnv<F, V> {
        self.0
    }
}

impl<'a, F, V> Drop for FrameHandle<'a, F, V> {
    fn drop(&mut self) {
        (self.0).0.pop();
    }
}

pub enum EnvSlot<'a, F, V> {
    Func(&'a F),
    Var(&'a V, Assignable),
}

pub enum LocalSlot<F, V> {
    Func(F),
    Var(V),
    NonLocal,
    Global,
}

impl<F: Clone, V: Clone> LocalEnv<F, V> {
    pub fn new(base: HashMap<String, LocalSlot<F, V>>) -> LocalEnv<F, V> {
        LocalEnv(vec![base])
    }

    pub fn get(&self, name: &str) -> Option<EnvSlot<F, V>> {
        match self.0.last().unwrap().get(name) {
            Some(LocalSlot::Var(t)) => Some(EnvSlot::Var(t, Assignable(true))),
            Some(LocalSlot::Func(t)) => Some(EnvSlot::Func(t)),
            Some(LocalSlot::Global) => {
                let t = if let Some(LocalSlot::Var(t)) = &self.0[0].get(name) {
                    t
                } else {
                    panic!()
                };
                Some(EnvSlot::Var(t, Assignable(true)))
            }
            s @ Some(LocalSlot::NonLocal) | s @ None => {
                for frame in self.0[0..self.0.len() - 1].iter().rev() {
                    match frame.get(name) {
                        Some(LocalSlot::NonLocal) | None => (),
                        Some(LocalSlot::Global) => {
                            assert!(s.is_none());
                            let t = if let Some(LocalSlot::Var(t)) = &self.0[0].get(name) {
                                t
                            } else {
                                panic!()
                            };
                            return Some(EnvSlot::Var(t, Assignable(false)));
                        }
                        Some(LocalSlot::Var(t)) => {
                            return Some(EnvSlot::Var(t, Assignable(s.is_some())))
                        }
                        Some(LocalSlot::Func(t)) => {
                            assert!(s.is_none());
                            return Some(EnvSlot::Func(t));
                        }
                    }
                }
                None
            }
        }
    }

    pub fn push(&mut self, frame: HashMap<String, LocalSlot<F, V>>) -> FrameHandle<F, V> {
        self.0.push(frame);
        FrameHandle(self)
    }
}
