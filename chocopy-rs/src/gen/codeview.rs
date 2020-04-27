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
    LProc32Id = 0x1146,
    GProc32Id = 0x1147,
    ProcIdEnd = 0x114F,
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
    string_table: Vec<u8>,
}

impl Codeview {
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

        // TODO: emit build info

        let mut symbol_stream = vec![];
        let type_stream = vec![];
        let mut string_table = vec![0]; // not sure what the leading 0 means

        symbol_stream.write_u32(4);
        symbol_stream.write_subsection(SubsectionType::Symbols, unit_info);

        let md5 = compute_md5(source_path)?;

        // Use canonicalize() instead? But it starts with "\\?\". Is it ok?
        let source_path = std::path::PathBuf::from(source_path);
        let full_path = if source_path.is_absolute() {
            source_path
        } else {
            std::path::Path::new(current_dir).join(source_path)
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

        Ok(Codeview {
            symbol_stream,
            symbol_links: vec![],
            type_stream,
            string_table,
        })
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

    fn add_global(&mut self, _: VarDebug) {}

    fn finalize(mut self: Box<Self>) -> Vec<DebugChunk> {
        self.symbol_stream
            .write_subsection(SubsectionType::StringTable, self.string_table);
        vec![DebugChunk {
            name: ".debug$S".to_owned(),
            code: self.symbol_stream,
            links: self.symbol_links,
        }]
    }
}
