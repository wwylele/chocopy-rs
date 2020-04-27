use super::*;

pub(super) trait DebugWriter {
    fn add_type<'a>(&mut self, type_repr: TypeDebugRepresentive<'a>);
    fn add_class(&mut self, class_name: String, class_debug: ClassDebug);
    fn add_chunk(&mut self, chunk: &Chunk);
    fn add_global(&mut self, global_debug: VarDebug);
    fn finalize(&mut self) -> Vec<DebugChunk>;
}

pub struct DummyDebug;

impl DebugWriter for DummyDebug {
    fn add_type<'a>(&mut self, _: TypeDebugRepresentive<'a>) {}
    fn add_class(&mut self, _: String, _: ClassDebug) {}
    fn add_chunk(&mut self, _: &Chunk) {}
    fn add_global(&mut self, _: VarDebug) {}
    fn finalize(&mut self) -> Vec<DebugChunk> {
        vec![]
    }
}
