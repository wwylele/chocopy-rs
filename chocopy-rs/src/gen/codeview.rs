use super::debug::*;
use super::*;
use md5::*;
use std::fs::*;
use std::io::Read;

enum SubsectionType {
    Symbols = 0xF1,
    Lines = 0xF2,
    StringTable = 0xF3,
    FileChksms = 0xF4,
}

enum RecordType {
    ObjName = 0x1101,
    Compile3 = 0x113C,
    FrameProc = 0x1012,
    LData32 = 0x110C,
    Local = 0x113E,
    DefRangFramePointerRelFullScope = 0x1144,
    LProc32Id = 0x1146,
    GProc32Id = 0x1147,
    BuildInfo = 0x114C,
    ProcIdEnd = 0x114F,
}

enum LeafType {
    BuildInfo = 0x1603,
    StringId = 0x1605,
}

trait VecWriter {
    fn write_slice(&mut self, value: &[u8]);
    fn write_u8(&mut self, value: u8);
    fn align4(&mut self);

    fn write_u16(&mut self, value: u16) {
        self.write_slice(&value.to_le_bytes())
    }

    fn write_u32(&mut self, value: u32) {
        self.write_slice(&value.to_le_bytes())
    }

    fn write_str(&mut self, s: &str) {
        self.write_slice(s.as_bytes());
        self.write_u8(0);
    }

    fn write_subsection(&mut self, subsection_type: SubsectionType, subsection: Vec<u8>) {
        self.write_u32(subsection_type as u32);
        self.write_u32(subsection.len() as u32);
        self.write_slice(&subsection);
        self.align4();
    }

    fn write_record(&mut self, record_type: RecordType, record: Vec<u8>) {
        self.write_u16((record.len() + 2) as u16);
        self.write_u16(record_type as u16);
        self.write_slice(&record);
    }
}

impl VecWriter for Vec<u8> {
    fn write_slice(&mut self, value: &[u8]) {
        self.extend_from_slice(value)
    }
    fn write_u8(&mut self, value: u8) {
        self.push(value)
    }
    fn align4(&mut self) {
        while self.len() % 4 != 0 {
            self.push(0)
        }
    }
}

fn compute_md5(source_path: &str) -> std::result::Result<[u8; 16], Box<dyn std::error::Error>> {
    let mut file = File::open(source_path)?;
    let mut buffer = [0; 256];
    let mut md5 = Md5::new();
    loop {
        let len = file.read(&mut buffer)?;
        if len == 0 {
            break;
        }
        md5.input(&buffer[0..len]);
    }
    Ok(md5.result().into())
}

pub struct Codeview {
    symbol_stream: Vec<u8>,
    symbol_links: Vec<DebugChunkLink>,
    type_stream: Vec<u8>,
    type_index: u32,
    string_table: Vec<u8>,
}

impl Codeview {
    fn write_leaf(&mut self, record_type: LeafType, record: Vec<u8>) -> u32 {
        self.type_stream.write_u16((record.len() + 2) as u16);
        self.type_stream.write_u16(record_type as u16);
        self.type_stream.write_slice(&record);
        let current_index = self.type_index;
        self.type_index += 1;
        current_index
    }

    pub fn new(
        source_path: &str,
        current_dir: &str,
        obj_path: &str,
    ) -> std::result::Result<Codeview, Box<dyn std::error::Error>> {
        let mut obj_name = vec![];
        obj_name.write_u32(0); // signature
        obj_name.write_str(obj_path);

        let mut compile3 = vec![];
        compile3.write_u32(0); // flags, language = C
        compile3.write_u16(0xD0); // x86-64
        compile3.write_u16(1); // front major version
        compile3.write_u16(0); // front minor version
        compile3.write_u16(1); // front build version
        compile3.write_u16(0); // front QFE version
        compile3.write_u16(1); // back major version
        compile3.write_u16(0); // back minor version
        compile3.write_u16(1); // back build version
        compile3.write_u16(0); // back QFE version
        compile3.write_str("chocopy-rs");

        let mut unit_info = vec![];
        unit_info.write_record(RecordType::ObjName, obj_name);
        unit_info.write_record(RecordType::Compile3, compile3);

        let mut symbol_stream = vec![];
        let mut type_stream = vec![];
        type_stream.write_u32(4);
        let mut string_table = vec![0]; // not sure what the leading 0 means

        symbol_stream.write_u32(4);
        symbol_stream.write_subsection(SubsectionType::Symbols, unit_info);

        let md5 = compute_md5(source_path)?;

        // Use canonicalize() instead? But it starts with "\\?\". Is it ok?
        let source_path_buf = std::path::PathBuf::from(source_path);
        let full_path = if source_path_buf.is_absolute() {
            source_path_buf
        } else {
            std::path::Path::new(current_dir).join(source_path_buf)
        };
        let source_path_offset = string_table.len();
        string_table.write_str(full_path.to_str().ok_or(PathError)?);

        let mut chksms = vec![];
        chksms.write_u32(source_path_offset as u32);
        chksms.write_u8(0x10); // len
        chksms.write_u8(1); // type
        chksms.write_slice(&md5);
        chksms.align4();
        symbol_stream.write_subsection(SubsectionType::FileChksms, chksms);

        let mut codeview = Codeview {
            symbol_stream,
            symbol_links: vec![],
            type_stream,
            type_index: 0x1000,
            string_table,
        };

        let mut leaf_current_dir = vec![];
        leaf_current_dir.write_u32(0);
        leaf_current_dir.write_str(current_dir);
        let id_current_dir = codeview.write_leaf(LeafType::StringId, leaf_current_dir);

        let mut leaf_build_tool = vec![];
        leaf_build_tool.write_u32(0);
        leaf_build_tool.write_str("chocopy-rs.exe");
        let id_build_tool = codeview.write_leaf(LeafType::StringId, leaf_build_tool);

        let mut leaf_source_path = vec![];
        leaf_source_path.write_u32(0);
        leaf_source_path.write_str(source_path);
        let id_source_path = codeview.write_leaf(LeafType::StringId, leaf_source_path);

        let mut leaf_database = vec![];
        leaf_database.write_u32(0);
        leaf_database.write_str("");
        let id_database = codeview.write_leaf(LeafType::StringId, leaf_database);

        let mut leaf_build_arg = vec![];
        leaf_build_arg.write_u32(0);
        leaf_build_arg.write_str("");
        let id_build_arg = codeview.write_leaf(LeafType::StringId, leaf_build_arg);

        let mut leaf_build_info = vec![];
        leaf_build_info.write_u16(5);
        leaf_build_info.write_u32(id_current_dir);
        leaf_build_info.write_u32(id_build_tool);
        leaf_build_info.write_u32(id_source_path);
        leaf_build_info.write_u32(id_database);
        leaf_build_info.write_u32(id_build_arg);
        let id_build_info = codeview.write_leaf(LeafType::BuildInfo, leaf_build_info);

        let mut build_info = vec![];
        build_info.write_u32(id_build_info);
        let mut subsection_build_info = vec![];
        subsection_build_info.write_record(RecordType::BuildInfo, build_info);
        codeview
            .symbol_stream
            .write_subsection(SubsectionType::Symbols, subsection_build_info);

        Ok(codeview)
    }

    fn get_type(&self, type_debug: &TypeDebug) -> u32 {
        if type_debug.array_level == 0 {
            match type_debug.core_name.as_str() {
                "int" => 0x0074,
                "bool" => 0x0030,
                _ => 0x0603,
            }
        } else {
            0x0603
        }
    }
}

impl DebugWriter for Codeview {
    fn add_type<'a>(&mut self, _: TypeDebugRepresentive<'a>) {}

    fn add_class(&mut self, _: String, _: ClassDebug) {}

    fn add_chunk(&mut self, chunk: &Chunk) {
        if let ChunkExtra::Procedure(procedure) = &chunk.extra {
            let proc_id_type = if chunk.name == BUILTIN_CHOCOPY_MAIN {
                RecordType::GProc32Id
            } else {
                RecordType::LProc32Id
            };
            let mut proc = vec![];
            proc.write_u32(0); // parent
            proc.write_u32(0); // end
            proc.write_u32(0); // next
            proc.write_u32(chunk.code.len() as u32);
            proc.write_u32(0); // debug start
            proc.write_u32(chunk.code.len() as u32); // debug end
            proc.write_u32(0); // type
            proc.write_u32(0); // offset
            proc.write_u16(0); // segment
            proc.write_u8(1 | (1 << 5)); // flags: CV_PFLAG_NOFPO | CV_PFLAG_CUST_CALL
            proc.write_str(&chunk.name);

            let mut frame_proc = vec![];
            frame_proc.write_u32(0); // frame length
            frame_proc.write_u32(0); // pad
            frame_proc.write_u32(0); // pad offset
            frame_proc.write_u32(0); // save regs
            frame_proc.write_u32(0); // exception handler
            frame_proc.write_u16(0); // exception handler id
            frame_proc.write_u32((2 << 14) | (2 << 16)); // flags: RBP as frame pointer

            let mut symbols = vec![];
            symbols.write_record(proc_id_type, proc);
            symbols.write_record(RecordType::FrameProc, frame_proc);

            for (var, is_param) in procedure
                .params
                .iter()
                .zip(std::iter::repeat(true))
                .chain(procedure.locals.iter().zip(std::iter::repeat(false)))
            {
                let type_id = self.get_type(&var.var_type);
                let mut symbol = vec![];
                symbol.write_u32(type_id);
                symbol.write_u16(if is_param { 1 } else { 0 });
                symbol.write_str(&var.name);
                symbols.write_record(RecordType::Local, symbol);

                let mut location = vec![];
                location.write_u32(var.offset as u32);
                symbols.write_record(RecordType::DefRangFramePointerRelFullScope, location);
            }

            symbols.write_record(RecordType::ProcIdEnd, vec![]);

            self.symbol_links.push(DebugChunkLink {
                link_type: DebugChunkLinkType::SectionRelative,
                pos: self.symbol_stream.len() + 8 + 32,
                to: chunk.name.clone(),
                size: 4,
            });

            self.symbol_links.push(DebugChunkLink {
                link_type: DebugChunkLinkType::SectionId,
                pos: self.symbol_stream.len() + 8 + 32 + 4,
                to: chunk.name.clone(),
                size: 2,
            });

            self.symbol_stream
                .write_subsection(SubsectionType::Symbols, symbols);

            if !procedure.artificial {
                let mut lines = vec![];

                lines.write_u32(0); // offset
                lines.write_u16(0); // segment
                lines.write_u16(0); // flags
                lines.write_u32(chunk.code.len() as u32);
                lines.write_u32(0); // file ID
                lines.write_u32(procedure.lines.len() as u32);
                lines.write_u32(12 + procedure.lines.len() as u32 * 8);

                for line in &procedure.lines {
                    lines.write_u32(line.code_pos as u32);
                    lines.write_u32(line.line_number | 0x8000_0000);
                }

                self.symbol_links.push(DebugChunkLink {
                    link_type: DebugChunkLinkType::SectionRelative,
                    pos: self.symbol_stream.len() + 8,
                    to: chunk.name.clone(),
                    size: 4,
                });

                self.symbol_links.push(DebugChunkLink {
                    link_type: DebugChunkLinkType::SectionId,
                    pos: self.symbol_stream.len() + 12,
                    to: chunk.name.clone(),
                    size: 2,
                });

                self.symbol_stream
                    .write_subsection(SubsectionType::Lines, lines);
            }
        }
    }

    fn add_global(&mut self, global: VarDebug) {
        let mut symbol = vec![];

        let type_id = self.get_type(&global.var_type);

        symbol.write_u32(type_id);
        symbol.write_u32(global.offset as u32);
        symbol.write_u16(0); // segment
        symbol.write_str(&global.name);

        let mut subsection = vec![];
        subsection.write_record(RecordType::LData32, symbol);

        self.symbol_links.push(DebugChunkLink {
            link_type: DebugChunkLinkType::SectionRelative,
            pos: self.symbol_stream.len() + 16,
            to: GLOBAL_SECTION.to_owned(),
            size: 4,
        });

        self.symbol_links.push(DebugChunkLink {
            link_type: DebugChunkLinkType::SectionId,
            pos: self.symbol_stream.len() + 20,
            to: GLOBAL_SECTION.to_owned(),
            size: 2,
        });

        self.symbol_stream
            .write_subsection(SubsectionType::Symbols, subsection);
    }

    fn finalize(mut self: Box<Self>) -> Vec<DebugChunk> {
        self.symbol_stream
            .write_subsection(SubsectionType::StringTable, self.string_table);
        vec![
            DebugChunk {
                name: ".debug$S".to_owned(),
                code: self.symbol_stream,
                links: self.symbol_links,
            },
            DebugChunk {
                name: ".debug$T".to_owned(),
                code: self.type_stream,
                links: vec![],
            },
        ]
    }
}
