use super::debug::*;
use super::*;
use chocopy_rs_common::*;
use md5::*;
use std::collections::HashMap;
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
    Udt = 0x1108,
    LData32 = 0x110C,
    Local = 0x113E,
    DefRangFramePointerRelFullScope = 0x1144,
    LProc32Id = 0x1146,
    GProc32Id = 0x1147,
    BuildInfo = 0x114C,
    ProcIdEnd = 0x114F,
}

enum LeafType {
    Pointer = 0x1002,
    Procedure = 0x1008,
    ArgList = 0x1201,
    FieldList = 0x1203,
    Structure = 0x1505,
    FuncId = 0x1601,
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
    type_map: HashMap<String, u32>,
    pdata: Vec<u8>,
    pdata_links: Vec<DebugChunkLink>,
    xdata: Vec<u8>,
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

        let version_major = env!("CARGO_PKG_VERSION_MAJOR");
        let version_minor = env!("CARGO_PKG_VERSION_MINOR");
        let version_patch = env!("CARGO_PKG_VERSION_PATCH");

        let version_major = version_major.parse().unwrap_or(0);
        let version_minor = version_minor.parse().unwrap_or(0);
        let version_patch = version_patch.parse().unwrap_or(0);

        let mut compile3 = vec![];
        compile3.write_u32(1); // flags, language = C++
        compile3.write_u16(0xD0); // x86-64
        compile3.write_u16(version_major); // front major version
        compile3.write_u16(version_minor); // front minor version
        compile3.write_u16(version_patch); // front build version
        compile3.write_u16(0); // front QFE version
        compile3.write_u16(version_major); // back major version
        compile3.write_u16(version_minor); // back minor version
        compile3.write_u16(version_patch); // back build version
        compile3.write_u16(0); // back QFE version
        compile3.write_str(env!("CARGO_PKG_NAME"));

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
            type_map: HashMap::new(),
            pdata: vec![],
            pdata_links: vec![],
            xdata: vec![],
        };

        let mut leaf_current_dir = vec![];
        leaf_current_dir.write_u32(0);
        leaf_current_dir.write_str(current_dir);
        let id_current_dir = codeview.write_leaf(LeafType::StringId, leaf_current_dir);

        let mut leaf_build_tool = vec![];
        leaf_build_tool.write_u32(0);
        leaf_build_tool.write_str(
            std::env::current_exe()?
                .as_os_str()
                .to_str()
                .ok_or(PathError)?,
        );
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

    fn get_type(&mut self, type_debug: &TypeDebug) -> u32 {
        if type_debug.array_level == 0 {
            match type_debug.core_name.as_str() {
                "int" => return 0x0074,
                "bool" => return 0x0030,
                "str" => (),
                s => return self.type_map.get(s).copied().unwrap_or(0x0603),
            }
        }

        if let Some(&id) = self.type_map.get("[]") {
            return id;
        }

        self.add_type(TypeDebugRepresentive {
            core_name: "[]",
            max_array_level: 0,
        });

        self.add_class(
            "[]".to_owned(),
            ClassDebug {
                size: 8,
                attributes: vec![VarDebug {
                    offset: ARRAY_LEN_OFFSET as i32,
                    line: 0,
                    name: "$len".to_owned(),
                    var_type: TypeDebug::class_type("int"),
                }],
                methods: std::iter::once((
                    PROTOTYPE_INIT_OFFSET,
                    (
                        "__init__".to_owned(),
                        MethodDebug {
                            params: vec![],
                            return_type: TypeDebug::class_type("[]"),
                        },
                    ),
                ))
                .collect(),
            },
        );

        self.type_map.get("[]").copied().unwrap()
    }
}

impl DebugWriter for Codeview {
    fn add_type<'a>(&mut self, representive: TypeDebugRepresentive<'a>) {
        if !matches!(
            representive.core_name,
            "str" | "int" | "bool" | "<None>" | "<Empty>"
        ) {
            let mut storage_type = vec![];
            storage_type.write_u16(0); // element count
            storage_type.write_u16(0x0080); // forward def
            storage_type.write_u32(0); // field
            storage_type.write_u32(0); // derived
            storage_type.write_u32(0); // vshape
            storage_type.write_u16(0); // size
            storage_type.write_str(representive.core_name);
            let storage_type_id = self.write_leaf(LeafType::Structure, storage_type);

            let mut pointer_type = vec![];
            pointer_type.write_u32(storage_type_id);
            pointer_type.write_u32(0xC); // ptr64
            let pointer_type_id = self.write_leaf(LeafType::Pointer, pointer_type);

            self.type_map
                .insert(representive.core_name.to_owned(), pointer_type_id);
        }
    }

    fn add_class(&mut self, name: String, class_debug: ClassDebug) {
        const MEMBER: u16 = 0x150D;

        let mut proto_fields = vec![];

        proto_fields.write_u16(MEMBER);
        proto_fields.write_u16(1); // private
        proto_fields.write_u32(0x0074);
        proto_fields.write_u16(PROTOTYPE_SIZE_OFFSET as u16);
        proto_fields.write_str("$size");

        proto_fields.write_u16(MEMBER);
        proto_fields.write_u16(1); // private
        proto_fields.write_u32(0x0074);
        proto_fields.write_u16(PROTOTYPE_TAG_OFFSET as u16);
        proto_fields.write_str("$tag");

        let mut arg_list = vec![];
        arg_list.write_u32(1);
        arg_list.write_u32(self.get_type(&TypeDebug::class_type(&name)));
        let arg_list_id = self.write_leaf(LeafType::ArgList, arg_list);

        let mut procedure_type = vec![];
        procedure_type.write_u32(0x0003); // void
        procedure_type.write_u8(0); // CV_CALL_NEAR_C,  near right to left push, caller pops stack
        procedure_type.write_u8(0); // funcattr
        procedure_type.write_u16(1);
        procedure_type.write_u32(arg_list_id);
        let procedure_type_id = self.write_leaf(LeafType::Procedure, procedure_type);

        let mut procedure_pointer_type = vec![];
        procedure_pointer_type.write_u32(procedure_type_id);
        procedure_pointer_type.write_u32(0xC); // ptr64
        let procedure_pointer_type_id = self.write_leaf(LeafType::Pointer, procedure_pointer_type);

        proto_fields.write_u16(MEMBER);
        proto_fields.write_u16(1); // private
        proto_fields.write_u32(procedure_pointer_type_id);
        proto_fields.write_u16(PROTOTYPE_MAP_OFFSET as u16);
        proto_fields.write_str("$map");

        for (&offset, (name, method)) in &class_debug.methods {
            let mut arg_list = vec![];
            arg_list.write_u32(method.params.len() as u32);
            for param in &method.params {
                arg_list.write_u32(self.get_type(&param));
            }
            let arg_list_id = self.write_leaf(LeafType::ArgList, arg_list);

            let mut procedure_type = vec![];
            procedure_type.write_u32(self.get_type(&method.return_type));
            procedure_type.write_u8(0); // CV_CALL_NEAR_C,  near right to left push, caller pops stack
            procedure_type.write_u8(0); // funcattr
            procedure_type.write_u16(method.params.len() as u16);
            procedure_type.write_u32(arg_list_id);
            let procedure_type_id = self.write_leaf(LeafType::Procedure, procedure_type);

            let mut procedure_pointer_type = vec![];
            procedure_pointer_type.write_u32(procedure_type_id);
            procedure_pointer_type.write_u32(0xC); // ptr64
            let procedure_pointer_type_id =
                self.write_leaf(LeafType::Pointer, procedure_pointer_type);

            proto_fields.write_u16(MEMBER);
            proto_fields.write_u16(1); // private
            proto_fields.write_u32(procedure_pointer_type_id);
            proto_fields.write_u16(offset as u16);
            proto_fields.write_str(name);
        }

        let proto_fields_id = self.write_leaf(LeafType::FieldList, proto_fields);

        let mut proto_storage_type = vec![];
        proto_storage_type
            .write_u16(class_debug.methods.len() as u16 + PROTOTYPE_HEADER_MEMBER_COUNT as u16); // element count
        proto_storage_type.write_u16(0); // no flag
        proto_storage_type.write_u32(proto_fields_id);
        proto_storage_type.write_u32(0); // derived
        proto_storage_type.write_u32(0); // vshape
        proto_storage_type
            .write_u16(class_debug.methods.len() as u16 * 8 + PROTOTYPE_INIT_OFFSET as u16); // size
        proto_storage_type.write_str(&(name.clone() + ".$prototype"));
        let proto_storage_type_id = self.write_leaf(LeafType::Structure, proto_storage_type);

        let mut proto_pointer_type = vec![];
        proto_pointer_type.write_u32(proto_storage_type_id);
        proto_pointer_type.write_u32(0xC); // ptr64
        let proto_pointer_type_id = self.write_leaf(LeafType::Pointer, proto_pointer_type);

        let mut fields = vec![];

        fields.write_u16(MEMBER);
        fields.write_u16(1); // private
        fields.write_u32(proto_pointer_type_id);
        fields.write_u16(OBJECT_PROTOTYPE_OFFSET as u16);
        fields.write_str("$proto");

        fields.write_u16(MEMBER);
        fields.write_u16(1); // private
        fields.write_u32(0x0077);
        fields.write_u16(OBJECT_GC_COUNT_OFFSET as u16);
        fields.write_str("$gc_count");

        fields.write_u16(MEMBER);
        fields.write_u16(1); // private
        fields.write_u32(0x0077);
        fields.write_u16(OBJECT_GC_NEXT_OFFSET as u16);
        fields.write_str("$gc_next");

        for attribute in &class_debug.attributes {
            fields.write_u16(MEMBER);
            fields.write_u16(3); // public
            fields.write_u32(self.get_type(&attribute.var_type));
            fields.write_u16(attribute.offset as u16);
            fields.write_str(&attribute.name);
        }
        let fields_id = self.write_leaf(LeafType::FieldList, fields);

        let mut storage_type = vec![];
        storage_type
            .write_u16(class_debug.attributes.len() as u16 + OBJECT_HEADER_MEMBER_COUNT as u16); // element count
        storage_type.write_u16(0); // no flag
        storage_type.write_u32(fields_id);
        storage_type.write_u32(0); // derived
        storage_type.write_u32(0); // vshape
        storage_type.write_u16(class_debug.size as u16 + OBJECT_ATTRIBUTE_OFFSET as u16); // size
        storage_type.write_str(&name);
        let storage_type_id = self.write_leaf(LeafType::Structure, storage_type);

        let mut udt = vec![];
        udt.write_u32(storage_type_id);
        udt.write_str(&name);
        let mut udt_subsection = vec![];
        udt_subsection.write_record(RecordType::Udt, udt);
        self.symbol_stream
            .write_subsection(SubsectionType::Symbols, udt_subsection);
    }

    fn add_chunk(&mut self, chunk: &Chunk) {
        if let ChunkExtra::Procedure(procedure) = &chunk.extra {
            let mut arg_list = vec![];
            arg_list.write_u32(procedure.params.len() as u32);
            for param in &procedure.params {
                arg_list.write_u32(self.get_type(&param.var_type));
            }
            let arg_list_id = self.write_leaf(LeafType::ArgList, arg_list);

            let mut procedure_type = vec![];
            procedure_type.write_u32(self.get_type(&procedure.return_type));
            procedure_type.write_u8(0); // CV_CALL_NEAR_C,  near right to left push, caller pops stack
            procedure_type.write_u8(0); // funcattr
            procedure_type.write_u16(procedure.params.len() as u16);
            procedure_type.write_u32(arg_list_id);
            let procedure_type_id = self.write_leaf(LeafType::Procedure, procedure_type);

            let mut func_id = vec![];
            func_id.write_u32(0); // parent
            func_id.write_u32(procedure_type_id);
            func_id.write_str(&chunk.name);
            let func_id_id = self.write_leaf(LeafType::FuncId, func_id);

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
            proc.write_u32(11); // debug start
            proc.write_u32(chunk.code.len() as u32); // debug end
            proc.write_u32(func_id_id);
            proc.write_u32(0); // offset
            proc.write_u16(0); // segment
            proc.write_u8(1 | (1 << 5)); // CV_PFLAG_CUST_CALL | CV_PFLAG_NOFPO
            proc.write_str(&chunk.name);

            let mut frame_proc = vec![];
            frame_proc.write_u32(procedure.frame_size);
            frame_proc.write_u32(0); // pad
            frame_proc.write_u32(0); // pad offset
            frame_proc.write_u32(0); // save regs
            frame_proc.write_u32(0); // exception handler
            frame_proc.write_u16(0); // exception handler id
            frame_proc.write_u32((2 << 16) | (2 << 14)); // flags: RBP as frame pointer

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

            let xdata_offset = self.xdata.len();
            self.xdata.write_u8(1); // version
            self.xdata.write_u8(11); // prolog
            self.xdata.write_u8(3); // code count
            self.xdata.write_u8(0); // frame register
            self.xdata.write_u16(0x010B); // UWOP_ALLOC_LARGE
            self.xdata.write_u16((procedure.frame_size / 8) as u16);
            self.xdata.write_u16(0x5001); // UWOP_PUSH_NONVOL RBP
            self.xdata.write_u16(0); // padding

            self.pdata_links.push(DebugChunkLink {
                link_type: DebugChunkLinkType::ImageRelative,
                pos: self.pdata.len(),
                to: chunk.name.clone(),
                size: 4,
            });
            self.pdata.write_u32(0);

            self.pdata_links.push(DebugChunkLink {
                link_type: DebugChunkLinkType::ImageRelative,
                pos: self.pdata.len(),
                to: chunk.name.clone(),
                size: 4,
            });
            self.pdata.write_u32(chunk.code.len() as u32);

            self.pdata_links.push(DebugChunkLink {
                link_type: DebugChunkLinkType::ImageRelative,
                pos: self.pdata.len(),
                to: ".xdata".to_owned(),
                size: 4,
            });
            self.pdata.write_u32(xdata_offset as u32);
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
                discardable: true,
            },
            DebugChunk {
                name: ".debug$T".to_owned(),
                code: self.type_stream,
                links: vec![],
                discardable: true,
            },
            DebugChunk {
                name: ".pdata".to_owned(),
                code: self.pdata,
                links: self.pdata_links,
                discardable: false,
            },
            DebugChunk {
                name: ".xdata".to_owned(),
                code: self.xdata,
                links: vec![],
                discardable: false,
            },
        ]
    }
}
