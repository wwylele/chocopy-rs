mod gimli_writer;
mod x64;

use crate::local_env::*;
use crate::node::*;
use faerie::*;
use gimli_writer::*;
use std::collections::HashMap;
use std::convert::*;
use std::io::Write;
use std::str::FromStr;
use target_lexicon::*;

const BOOL_PROTOTYPE: &'static str = "bool.$proto";
const INT_PROTOTYPE: &'static str = "int.$proto";
const STR_PROTOTYPE: &'static str = "str.$proto";
const BOOL_LIST_PROTOTYPE: &'static str = "[bool].$proto";
const INT_LIST_PROTOTYPE: &'static str = "[int].$proto";
const OBJECT_LIST_PROTOTYPE: &'static str = "[object].$proto";
const BUILTIN_ALLOC_OBJ: &'static str = "$alloc_obj";
const BUILTIN_FREE_OBJ: &'static str = "$free_obj";
const BUILTIN_BROKEN_STACK: &'static str = "$broken_stack";
const BUILTIN_DIV_ZERO: &'static str = "$div_zero";
const BUILTIN_OUT_OF_BOUND: &'static str = "$out_of_bound";
const BUILTIN_NONE_OP: &'static str = "$none_op";
const BUILTIN_CHOCOPY_MAIN: &'static str = "$chocopy_main";
const GLOBAL_SECTION: &'static str = "$global";

struct ChunkLink {
    pos: usize,
    to: String,
}

#[derive(PartialEq, Eq, Hash, Clone)]
struct TypeDebug {
    name: String,
    array_level: u32,
}

impl std::fmt::Display for TypeDebug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for _ in 0..self.array_level {
            write!(f, "[")?;
        }
        write!(f, "{}", &self.name)?;
        for _ in 0..self.array_level {
            write!(f, "]")?;
        }
        Ok(())
    }
}

struct ParamDebug {
    offset: i32,
    line: u32,
    name: String,
    param_type: TypeDebug,
}

struct ProcedureDebug {
    decl_line: u32,
    artificial: bool,
    parent: Option<String>,
    lines: Vec<(usize, u32)>,
    return_type: TypeDebug,
    params: Vec<ParamDebug>,
}

impl ProcedureDebug {
    fn used_types(&self) -> impl Iterator<Item = &TypeDebug> {
        std::iter::once(&self.return_type).chain(self.params.iter().map(|param| &param.param_type))
    }
}

enum ChunkExtra {
    Procedure(ProcedureDebug),
    Data,
}

struct Chunk {
    name: String,
    code: Vec<u8>,
    links: Vec<ChunkLink>,
    extra: ChunkExtra,
}

struct CodeSet {
    chunks: Vec<Chunk>,
    global_size: usize,
}

impl CodeSet {
    fn used_types(&self) -> impl Iterator<Item = &TypeDebug> {
        self.chunks
            .iter()
            .filter_map(|chunk| {
                if let ChunkExtra::Procedure(procedure) = &chunk.extra {
                    Some(procedure.used_types())
                } else {
                    None
                }
            })
            .flatten()
    }
}

pub fn gen(
    source_path: &str,
    ast: Ast,
    path: &str,
    no_link: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let (obj_name, obj_path) = if no_link {
        let obj_path = std::path::Path::new(path);
        (
            obj_path
                .file_name()
                .map(std::ffi::OsStr::to_str)
                .flatten()
                .unwrap_or("a.o")
                .to_owned(),
            obj_path.to_owned(),
        )
    } else {
        let mut obj_path = std::env::temp_dir();
        let obj_name = format!("chocopy-{}.o", rand::random::<u32>());
        obj_path.push(obj_name.clone());
        (obj_name, obj_path)
    };

    let mut symbol_pool = vec![];

    let mut obj = ArtifactBuilder::new(triple!("x86_64-pc-linux-gnu-elf"))
        .name(obj_name.clone())
        .finish();

    let declarations = vec![
        (BOOL_PROTOTYPE, Decl::data_import().into()),
        (INT_PROTOTYPE, Decl::data_import().into()),
        (STR_PROTOTYPE, Decl::data_import().into()),
        (BOOL_LIST_PROTOTYPE, Decl::data_import().into()),
        (INT_LIST_PROTOTYPE, Decl::data_import().into()),
        (OBJECT_LIST_PROTOTYPE, Decl::data_import().into()),
        // hidden built-in functions
        (BUILTIN_ALLOC_OBJ, Decl::function_import().into()),
        (BUILTIN_FREE_OBJ, Decl::function_import().into()),
        (BUILTIN_BROKEN_STACK, Decl::function_import().into()),
        (BUILTIN_DIV_ZERO, Decl::function_import().into()),
        (BUILTIN_OUT_OF_BOUND, Decl::function_import().into()),
        (BUILTIN_NONE_OP, Decl::function_import().into()),
        // built-in functions
        ("len", Decl::function_import().into()),
        ("print", Decl::function_import().into()),
        ("input", Decl::function_import().into()),
        ("object.__init__", Decl::function_import().into()),
        ("str", Decl::function_import().into()),
        ("int", Decl::function_import().into()),
        ("bool", Decl::function_import().into()),
        // global
        (GLOBAL_SECTION, Decl::data().writable().into()),
    ];

    let encoding = gimli::Encoding {
        format: gimli::Format::Dwarf32,
        version: 4,
        address_size: 8,
    };
    let mut dwarf = gimli::write::DwarfUnit::new(encoding);

    let current_dir_buf = std::env::current_dir();
    let current_dir = current_dir_buf
        .as_ref()
        .map(|s| s.to_str())
        .ok()
        .flatten()
        .unwrap_or("");

    dwarf.unit.line_program = gimli::write::LineProgram::new(
        encoding,
        gimli::LineEncoding {
            minimum_instruction_length: 1,
            maximum_operations_per_instruction: 1,
            default_is_stmt: true,
            line_base: -5,
            line_range: 14,
        },
        gimli::write::LineString::String(current_dir.into()),
        gimli::write::LineString::String(source_path.into()),
        None,
    );

    dwarf.unit.line_program.add_file(
        gimli::write::LineString::String(source_path.into()),
        dwarf.unit.line_program.default_directory(),
        None,
    );

    let comp_dir = dwarf.strings.add(current_dir);
    let comp_name = dwarf.strings.add(source_path);
    let producer = dwarf.strings.add("chocopy-rs");

    let root_id = dwarf.unit.root();
    let compile_unit = dwarf.unit.get_mut(root_id);
    compile_unit.set(
        gimli::DW_AT_language,
        gimli::write::AttributeValue::Language(gimli::DW_LANG_Python),
    );
    compile_unit.set(
        gimli::DW_AT_comp_dir,
        gimli::write::AttributeValue::StringRef(comp_dir),
    );
    compile_unit.set(
        gimli::DW_AT_name,
        gimli::write::AttributeValue::StringRef(comp_name),
    );
    compile_unit.set(
        gimli::DW_AT_producer,
        gimli::write::AttributeValue::StringRef(producer),
    );

    obj.declarations(declarations.into_iter())?;

    let code_set = x64::gen_code_set(ast);

    for chunk in &code_set.chunks {
        if let ChunkExtra::Procedure(_) = chunk.extra {
            if chunk.name == BUILTIN_CHOCOPY_MAIN {
                obj.declare(&chunk.name, Decl::function().global())?;
            } else {
                obj.declare(&chunk.name, Decl::function().local())?
            }
        } else {
            obj.declare(&chunk.name, Decl::data())?;
        }
    }

    let mut debug_types = HashMap::new();
    for type_debug in code_set.used_types() {
        if debug_types.contains_key(type_debug) {
            continue;
        }
        let node_id = if type_debug.array_level == 0 && type_debug.name == "bool" {
            let node_type_bool_id = dwarf.unit.add(root_id, gimli::DW_TAG_base_type);
            let node_type_bool = dwarf.unit.get_mut(node_type_bool_id);
            node_type_bool.set(
                gimli::DW_AT_name,
                gimli::write::AttributeValue::String("bool".into()),
            );
            node_type_bool.set(
                gimli::DW_AT_encoding,
                gimli::write::AttributeValue::Encoding(gimli::DW_ATE_boolean),
            );
            node_type_bool.set(
                gimli::DW_AT_byte_size,
                gimli::write::AttributeValue::Data1(1),
            );
            node_type_bool_id
        } else if type_debug.array_level == 0 && type_debug.name == "int" {
            let node_type_int_id = dwarf.unit.add(root_id, gimli::DW_TAG_base_type);
            let node_type_int = dwarf.unit.get_mut(node_type_int_id);
            node_type_int.set(
                gimli::DW_AT_name,
                gimli::write::AttributeValue::String("int".into()),
            );
            node_type_int.set(
                gimli::DW_AT_encoding,
                gimli::write::AttributeValue::Encoding(gimli::DW_ATE_signed),
            );
            node_type_int.set(
                gimli::DW_AT_byte_size,
                gimli::write::AttributeValue::Data1(4),
            );
            node_type_int_id
        } else {
            let node_type_object_id = dwarf.unit.add(root_id, gimli::DW_TAG_base_type);
            let node_type_object = dwarf.unit.get_mut(node_type_object_id);
            node_type_object.set(
                gimli::DW_AT_name,
                gimli::write::AttributeValue::String(type_debug.to_string().into()),
            );
            node_type_object.set(
                gimli::DW_AT_encoding,
                gimli::write::AttributeValue::Encoding(gimli::DW_ATE_address),
            );
            node_type_object.set(
                gimli::DW_AT_byte_size,
                gimli::write::AttributeValue::Data1(8),
            );
            node_type_object_id
        };
        debug_types.insert(type_debug.clone(), node_id);
    }

    let mut range_list = vec![];
    let mut procedure_debug_map = HashMap::new();
    for chunk in code_set.chunks {
        if let ChunkExtra::Procedure(procedure_debug) = chunk.extra {
            let parent_id = if let Some(parent) = &procedure_debug.parent {
                procedure_debug_map[parent]
            } else {
                root_id
            };

            let sub_program_id = dwarf.unit.add(parent_id, gimli::DW_TAG_subprogram);
            procedure_debug_map.insert(chunk.name.clone(), sub_program_id);

            let sub_program = dwarf.unit.get_mut(sub_program_id);
            sub_program.set(
                gimli::DW_AT_low_pc,
                gimli::write::AttributeValue::Address(gimli::write::Address::Symbol {
                    symbol: symbol_pool.len(),
                    addend: 0,
                }),
            );
            sub_program.set(
                gimli::DW_AT_high_pc,
                gimli::write::AttributeValue::Udata(chunk.code.len() as u64),
            );
            sub_program.set(
                gimli::DW_AT_decl_file,
                gimli::write::AttributeValue::Data1(1),
            );
            sub_program.set(
                gimli::DW_AT_decl_line,
                gimli::write::AttributeValue::Udata(procedure_debug.decl_line as u64),
            );
            sub_program.set(
                gimli::DW_AT_name,
                gimli::write::AttributeValue::String(chunk.name.as_bytes().into()),
            );
            sub_program.set(
                gimli::DW_AT_artificial,
                gimli::write::AttributeValue::Flag(procedure_debug.artificial),
            );
            sub_program.set(
                gimli::DW_AT_frame_base,
                gimli::write::AttributeValue::Exprloc(gimli::write::Expression(vec![
                    gimli::DW_OP_reg6.0,
                ])),
            );
            sub_program.set(
                gimli::DW_AT_type,
                gimli::write::AttributeValue::ThisUnitEntryRef(
                    debug_types[&procedure_debug.return_type],
                ),
            );
            if procedure_debug.parent.is_some() {
                // Is this correct?
                sub_program.set(
                    gimli::DW_AT_static_link,
                    gimli::write::AttributeValue::Exprloc(gimli::write::Expression(vec![
                        gimli::DW_OP_fbreg.0,
                        -8i8 as u8,
                        gimli::DW_OP_deref.0,
                    ])),
                )
            }

            if !procedure_debug.lines.is_empty() {
                let line_program = &mut dwarf.unit.line_program;
                line_program.begin_sequence(Some(gimli::write::Address::Symbol {
                    symbol: symbol_pool.len(),
                    addend: 0,
                }));

                for (code_pos, line) in procedure_debug.lines {
                    line_program.row().address_offset = code_pos as u64;
                    line_program.row().line = line as u64;
                    line_program.generate_row();
                }

                line_program.end_sequence(chunk.code.len() as u64);
            }

            for param in procedure_debug.params {
                let param_node = dwarf
                    .unit
                    .add(sub_program_id, gimli::DW_TAG_formal_parameter);
                let param_node = dwarf.unit.get_mut(param_node);
                let mut offset_expr = vec![gimli::DW_OP_fbreg.0];
                gimli::leb128::write::signed(&mut offset_expr, param.offset as i64)?;

                param_node.set(
                    gimli::DW_AT_location,
                    gimli::write::AttributeValue::Exprloc(gimli::write::Expression(offset_expr)),
                );

                param_node.set(
                    gimli::DW_AT_name,
                    gimli::write::AttributeValue::String(param.name.into()),
                );

                param_node.set(
                    gimli::DW_AT_decl_file,
                    gimli::write::AttributeValue::Data1(1),
                );

                param_node.set(
                    gimli::DW_AT_decl_line,
                    gimli::write::AttributeValue::Udata(param.line as u64),
                );

                param_node.set(
                    gimli::DW_AT_type,
                    gimli::write::AttributeValue::ThisUnitEntryRef(debug_types[&param.param_type]),
                );
            }

            range_list.push(gimli::write::Range::StartLength {
                begin: gimli::write::Address::Symbol {
                    symbol: symbol_pool.len(),
                    addend: 0,
                },
                length: chunk.code.len() as u64,
            });
            symbol_pool.push(chunk.name.clone());
        }
        obj.define(&chunk.name, chunk.code)?;
        for link in chunk.links {
            obj.link(Link {
                from: &chunk.name,
                to: &link.to,
                at: link.pos as u64,
            })?;
        }
    }

    let range_list = dwarf.unit.ranges.add(gimli::write::RangeList(range_list));

    dwarf.unit.get_mut(root_id).set(
        gimli::DW_AT_ranges,
        gimli::write::AttributeValue::RangeListRef(range_list),
    );

    obj.define(GLOBAL_SECTION, vec![0; code_set.global_size])?;

    let mut dwarf_sections = gimli::write::Sections::new(DwarfWriter::new());
    dwarf.write(&mut dwarf_sections)?;
    dwarf_sections.for_each(|id, _| -> Result<(), ArtifactError> {
        obj.declare(&id.name(), Decl::section(SectionKind::Debug))?;
        Ok(())
    })?;
    dwarf_sections.for_each_mut(|id, data| -> Result<(), Box<dyn std::error::Error>> {
        let (data, relocs, self_relocs) = data.take();
        obj.define(&id.name(), data)?;

        for reloc in relocs {
            obj.link_with(
                Link {
                    from: &id.name(),
                    to: &symbol_pool[reloc.symbol],
                    at: reloc.offset as u64,
                },
                Reloc::Debug {
                    size: reloc.size,
                    addend: 0,
                },
            )?;
        }

        for self_reloc in self_relocs {
            obj.link_with(
                Link {
                    from: &id.name(),
                    to: self_reloc.section,
                    at: self_reloc.offset as u64,
                },
                Reloc::Debug {
                    size: self_reloc.size,
                    addend: 0,
                },
            )?;
        }
        Ok(())
    })?;

    let obj_file = std::fs::File::create(&obj_path)?;
    obj.write(obj_file)?;

    if no_link {
        return Ok(());
    }

    let mut lib_path = std::env::temp_dir();
    lib_path.push("libchocopy_rs_std.a");
    std::fs::write(
        &lib_path,
        &include_bytes!("../../../target/debug/libchocopy_rs_std.a")[..],
    )?;

    let ld_output = std::process::Command::new("ld")
        .args(&[
            "-o",
            path,
            "-l:crt1.o",
            "-l:crti.o",
            "-l:crtn.o",
            obj_path.to_str().unwrap(),
            lib_path.to_str().unwrap(),
            "-lc",
            "-lpthread",
            "-ldl",
            "-lunwind",
            "--dynamic-linker=/lib64/ld-linux-x86-64.so.2",
        ])
        .output()?;

    // println!("ld status: {}", ld_output.status);
    std::io::stdout().write_all(&ld_output.stdout).unwrap();
    std::io::stderr().write_all(&ld_output.stderr).unwrap();

    std::fs::remove_file(&obj_path)?;
    std::fs::remove_file(&lib_path)?;

    Ok(())
}
