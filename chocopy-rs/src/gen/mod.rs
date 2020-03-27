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
    core_name: String,
    array_level: u32,
}

impl TypeDebug {
    fn from_annotation(type_annotation: &TypeAnnotation) -> TypeDebug {
        match type_annotation {
            TypeAnnotation::ClassType(c) => TypeDebug {
                core_name: c.class_name.clone(),
                array_level: 0,
            },
            TypeAnnotation::ListType(l) => {
                let mut type_debug = TypeDebug::from_annotation(&l.element_type);
                type_debug.array_level += 1;
                type_debug
            }
        }
    }
}

impl std::fmt::Display for TypeDebug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for _ in 0..self.array_level {
            write!(f, "[")?;
        }
        write!(f, "{}", &self.core_name)?;
        for _ in 0..self.array_level {
            write!(f, "]")?;
        }
        Ok(())
    }
}

#[derive(Clone)]
struct VarDebug {
    offset: i32,
    line: u32,
    name: String,
    var_type: TypeDebug,
}

struct ProcedureDebug {
    decl_line: u32,
    artificial: bool,
    parent: Option<String>,
    lines: Vec<(usize, u32)>,
    return_type: TypeDebug,
    params: Vec<VarDebug>,
    locals: Vec<VarDebug>,
}

impl ProcedureDebug {
    fn used_types(&self) -> impl Iterator<Item = &TypeDebug> {
        std::iter::once(&self.return_type)
            .chain(self.params.iter().map(|param| &param.var_type))
            .chain(self.locals.iter().map(|local| &local.var_type))
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

#[derive(Clone)]
struct ClassDebug {
    size: u32,
    attributes: Vec<VarDebug>,
    methods: Vec<(String, i32)>,
}

impl ClassDebug {
    fn used_types(&self) -> impl Iterator<Item = &TypeDebug> {
        self.attributes.iter().map(|attribute| &attribute.var_type)
    }
}

struct CodeSet {
    chunks: Vec<Chunk>,
    global_size: usize,
    globals_debug: Vec<VarDebug>,
    classes_debug: HashMap<String, ClassDebug>,
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
            .chain(self.globals_debug.iter().map(|global| &global.var_type))
            .chain(
                self.classes_debug
                    .iter()
                    .map(|(_, class)| class.used_types())
                    .flatten(),
            )
    }
}

fn dwarf_add_base_type(
    dwarf: &mut gimli::write::DwarfUnit,
    name: &str,
    encoding: gimli::DwAte,
    byte_size: u8,
) -> gimli::write::UnitEntryId {
    let root_id = dwarf.unit.root();
    let id = dwarf.unit.add(root_id, gimli::DW_TAG_base_type);
    let tag = dwarf.unit.get_mut(id);
    tag.set(
        gimli::DW_AT_name,
        gimli::write::AttributeValue::String(name.into()),
    );
    tag.set(
        gimli::DW_AT_encoding,
        gimli::write::AttributeValue::Encoding(encoding),
    );
    tag.set(
        gimli::DW_AT_byte_size,
        gimli::write::AttributeValue::Data1(byte_size),
    );
    id
}

fn dwarf_add_struct_type(
    dwarf: &mut gimli::write::DwarfUnit,
    name: &str,
    byte_size: u64,
) -> gimli::write::UnitEntryId {
    let root_id = dwarf.unit.root();
    let id = dwarf.unit.add(root_id, gimli::DW_TAG_structure_type);
    let tag = dwarf.unit.get_mut(id);
    tag.set(
        gimli::DW_AT_name,
        gimli::write::AttributeValue::String(name.into()),
    );
    tag.set(
        gimli::DW_AT_byte_size,
        gimli::write::AttributeValue::Udata(byte_size),
    );
    id
}

fn dwarf_add_member(
    dwarf: &mut gimli::write::DwarfUnit,
    parent_id: gimli::write::UnitEntryId,
    name: &str,
    member_type_id: gimli::write::UnitEntryId,
    offset: u64,
) -> gimli::write::UnitEntryId {
    let id = dwarf.unit.add(parent_id, gimli::DW_TAG_member);
    let tag = dwarf.unit.get_mut(id);
    tag.set(
        gimli::DW_AT_name,
        gimli::write::AttributeValue::String(name.into()),
    );
    tag.set(
        gimli::DW_AT_data_member_location,
        gimli::write::AttributeValue::Udata(offset),
    );
    tag.set(
        gimli::DW_AT_type,
        gimli::write::AttributeValue::ThisUnitEntryRef(member_type_id),
    );
    id
}

fn dwarf_add_pointer_type(
    dwarf: &mut gimli::write::DwarfUnit,
    name: &str,
    pointee: gimli::write::UnitEntryId,
) -> gimli::write::UnitEntryId {
    let root_id = dwarf.unit.root();
    let id = dwarf.unit.add(root_id, gimli::DW_TAG_pointer_type);
    let tag = dwarf.unit.get_mut(id);
    tag.set(
        gimli::DW_AT_name,
        gimli::write::AttributeValue::String(name.into()),
    );
    tag.set(
        gimli::DW_AT_type,
        gimli::write::AttributeValue::ThisUnitEntryRef(pointee),
    );
    tag.set(
        gimli::DW_AT_byte_size,
        gimli::write::AttributeValue::Data1(8),
    );
    id
}

fn dwarf_add_subroutine_type(dwarf: &mut gimli::write::DwarfUnit) -> gimli::write::UnitEntryId {
    let root_id = dwarf.unit.root();
    let id = dwarf.unit.add(root_id, gimli::DW_TAG_subroutine_type);
    id
}

fn dwarf_add_array_type(
    dwarf: &mut gimli::write::DwarfUnit,
    element_type: gimli::write::UnitEntryId,
    index_type: gimli::write::UnitEntryId,
    len_member: gimli::write::UnitEntryId,
) -> gimli::write::UnitEntryId {
    let root_id = dwarf.unit.root();
    let id = dwarf.unit.add(root_id, gimli::DW_TAG_array_type);
    let tag = dwarf.unit.get_mut(id);
    tag.set(
        gimli::DW_AT_type,
        gimli::write::AttributeValue::ThisUnitEntryRef(element_type),
    );

    let index_id = dwarf.unit.add(id, gimli::DW_TAG_subrange_type);
    let index_tag = dwarf.unit.get_mut(index_id);
    index_tag.set(
        gimli::DW_AT_type,
        gimli::write::AttributeValue::ThisUnitEntryRef(index_type),
    );
    index_tag.set(
        gimli::DW_AT_count,
        gimli::write::AttributeValue::ThisUnitEntryRef(len_member),
    );

    id
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
    // I wanted to set this to python but LLDB didn't like it
    compile_unit.set(
        gimli::DW_AT_language,
        gimli::write::AttributeValue::Language(gimli::DW_LANG_C11),
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

    let ftype_id = dwarf_add_subroutine_type(&mut dwarf);
    let fptr_id = dwarf_add_pointer_type(&mut dwarf, "$fptr", ftype_id);

    let size_t_id = dwarf_add_base_type(&mut dwarf, "$size_t", gimli::DW_ATE_signed, 8);
    let char_id = dwarf_add_base_type(&mut dwarf, "$char", gimli::DW_ATE_signed, 1);
    let default_prototype_id = dwarf_add_struct_type(&mut dwarf, "object.$prototype", 24);
    dwarf_add_member(&mut dwarf, default_prototype_id, "$size", size_t_id, 0);
    dwarf_add_member(&mut dwarf, default_prototype_id, "$dtor", fptr_id, 8);
    dwarf_add_member(&mut dwarf, default_prototype_id, "__init__", fptr_id, 16);
    let default_prototype_ptr_id =
        dwarf_add_pointer_type(&mut dwarf, "$prototype*", default_prototype_id);

    let mut array_level_map = HashMap::new();
    for type_used in code_set.used_types() {
        if let Some(array_level) = array_level_map.get_mut(&type_used.core_name) {
            *array_level = std::cmp::max(*array_level, type_used.array_level)
        } else {
            array_level_map.insert(&type_used.core_name, type_used.array_level);
        }
    }

    let mut debug_types = HashMap::new();
    for (type_name, max_array_level) in array_level_map {
        for array_level in 0..=max_array_level {
            let type_debug = TypeDebug {
                core_name: type_name.clone(),
                array_level,
            };
            if debug_types.contains_key(&type_debug) {
                continue;
            }
            let node_id = if type_debug.array_level == 0 && type_debug.core_name == "bool" {
                dwarf_add_base_type(&mut dwarf, "bool", gimli::DW_ATE_boolean, 1)
            } else if type_debug.array_level == 0 && type_debug.core_name == "int" {
                dwarf_add_base_type(&mut dwarf, "int", gimli::DW_ATE_signed, 4)
            } else {
                let type_string = type_debug.to_string();
                let is_array = array_level != 0 || type_string == "str";

                let storage_type_id = dwarf_add_struct_type(
                    &mut dwarf,
                    &(type_debug.to_string() + ".$storage"),
                    if is_array { 24 } else { 16 },
                );

                if is_array {
                    dwarf_add_member(
                        &mut dwarf,
                        storage_type_id,
                        "$proto",
                        default_prototype_ptr_id,
                        0,
                    );

                    dwarf_add_member(&mut dwarf, storage_type_id, "$ref", size_t_id, 8);

                    let len_id =
                        dwarf_add_member(&mut dwarf, storage_type_id, "$len", size_t_id, 16);

                    let element_type = if type_string == "str" {
                        char_id
                    } else {
                        let mut element_type = type_debug.clone();
                        element_type.array_level -= 1;
                        debug_types[&element_type]
                    };
                    let array_type_id =
                        dwarf_add_array_type(&mut dwarf, element_type, size_t_id, len_id);
                    dwarf_add_member(&mut dwarf, storage_type_id, "$array", array_type_id, 24);
                }

                dwarf_add_pointer_type(&mut dwarf, &type_debug.to_string(), storage_type_id)
            };
            debug_types.insert(type_debug, node_id);
        }
    }

    for (class_name, class_debug) in code_set.classes_debug {
        let prototype_name = class_name.clone() + ".$prototype";
        let prototype_ptr_name = prototype_name.clone() + "*";
        let tag_id = debug_types[&TypeDebug {
            core_name: class_name,
            array_level: 0,
        }];

        let tag_id = if let gimli::write::AttributeValue::ThisUnitEntryRef(id) =
            dwarf.unit.get(tag_id).get(gimli::DW_AT_type).unwrap()
        {
            *id
        } else {
            panic!()
        };

        dwarf.unit.get_mut(tag_id).set(
            gimli::DW_AT_byte_size,
            gimli::write::AttributeValue::Udata((class_debug.size + 16) as u64),
        );

        let prototype_id = dwarf_add_struct_type(
            &mut dwarf,
            &prototype_name,
            ((class_debug.methods.len() + 2) * 8) as u64,
        );

        dwarf_add_member(&mut dwarf, prototype_id, "$size", size_t_id, 0);
        dwarf_add_member(&mut dwarf, prototype_id, "$dtor", fptr_id, 8);

        for (method, offset) in class_debug.methods {
            dwarf_add_member(&mut dwarf, prototype_id, &method, fptr_id, offset as u64);
        }

        let prototype_ptr_id =
            dwarf_add_pointer_type(&mut dwarf, &prototype_ptr_name, prototype_id);

        dwarf_add_member(&mut dwarf, tag_id, "$proto", prototype_ptr_id, 0);
        dwarf_add_member(&mut dwarf, tag_id, "$ref", size_t_id, 8);

        for attribute in class_debug.attributes {
            dwarf_add_member(
                &mut dwarf,
                tag_id,
                &attribute.name,
                debug_types[&attribute.var_type],
                attribute.offset as u64,
            );
        }
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
                sub_program.set(
                    gimli::DW_AT_static_link,
                    gimli::write::AttributeValue::Exprloc(gimli::write::Expression(vec![
                        gimli::DW_OP_fbreg.0,
                        0x78, // -8 in SLEB128
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

            for (var, is_param) in procedure_debug
                .params
                .into_iter()
                .zip(std::iter::repeat(true))
                .chain(
                    procedure_debug
                        .locals
                        .into_iter()
                        .zip(std::iter::repeat(false)),
                )
            {
                let node_id = dwarf.unit.add(
                    sub_program_id,
                    if is_param {
                        gimli::DW_TAG_formal_parameter
                    } else {
                        gimli::DW_TAG_variable
                    },
                );
                let node = dwarf.unit.get_mut(node_id);
                let mut offset_expr = vec![gimli::DW_OP_fbreg.0];
                gimli::leb128::write::signed(&mut offset_expr, var.offset as i64)?;

                node.set(
                    gimli::DW_AT_location,
                    gimli::write::AttributeValue::Exprloc(gimli::write::Expression(offset_expr)),
                );

                node.set(
                    gimli::DW_AT_name,
                    gimli::write::AttributeValue::String(var.name.into()),
                );

                node.set(
                    gimli::DW_AT_decl_file,
                    gimli::write::AttributeValue::Data1(1),
                );

                node.set(
                    gimli::DW_AT_decl_line,
                    gimli::write::AttributeValue::Udata(var.line as u64),
                );

                node.set(
                    gimli::DW_AT_type,
                    gimli::write::AttributeValue::ThisUnitEntryRef(debug_types[&var.var_type]),
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

    let global_reloc_hack_magic = 0xDEADB00Fu32;

    for global_debug in code_set.globals_debug {
        let node_id = dwarf.unit.add(root_id, gimli::DW_TAG_variable);
        let node = dwarf.unit.get_mut(node_id);

        let mut location = vec![gimli::DW_OP_addr.0];
        location.extend_from_slice(&global_debug.offset.to_le_bytes());
        location.extend_from_slice(&global_reloc_hack_magic.to_le_bytes());

        node.set(
            gimli::DW_AT_location,
            gimli::write::AttributeValue::Exprloc(gimli::write::Expression(location)),
        );

        node.set(
            gimli::DW_AT_name,
            gimli::write::AttributeValue::String(global_debug.name.into()),
        );

        node.set(
            gimli::DW_AT_decl_file,
            gimli::write::AttributeValue::Data1(1),
        );

        node.set(
            gimli::DW_AT_decl_line,
            gimli::write::AttributeValue::Udata(global_debug.line as u64),
        );

        node.set(
            gimli::DW_AT_type,
            gimli::write::AttributeValue::ThisUnitEntryRef(debug_types[&global_debug.var_type]),
        );
    }

    let mut dwarf_sections = gimli::write::Sections::new(DwarfWriter::new());
    dwarf.write(&mut dwarf_sections)?;
    dwarf_sections.for_each(|id, _| -> Result<(), ArtifactError> {
        obj.declare(&id.name(), Decl::section(SectionKind::Debug))?;
        Ok(())
    })?;
    dwarf_sections.for_each_mut(|id, data| -> Result<(), Box<dyn std::error::Error>> {
        let (mut data, relocs, self_relocs) = data.take();

        if data.len() >= 4 {
            for i in 0..data.len() - 3 {
                if &data[i..i + 4] == &global_reloc_hack_magic.to_le_bytes() {
                    data[i..i + 4].copy_from_slice(&[0; 4]);
                    obj.link_with(
                        Link {
                            from: &id.name(),
                            to: GLOBAL_SECTION,
                            at: (i - 4) as u64,
                        },
                        Reloc::Debug { size: 8, addend: 0 },
                    )?;
                }
            }
        }

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

    let ld_output = std::process::Command::new("gcc")
        .args(&[
            "-o",
            path,
            obj_path.to_str().unwrap(),
            lib_path.to_str().unwrap(),
            "-pthread",
            "-ldl",
        ])
        .output()?;

    std::io::stdout().write_all(&ld_output.stdout).unwrap();
    std::io::stderr().write_all(&ld_output.stderr).unwrap();

    std::fs::remove_file(&obj_path)?;
    std::fs::remove_file(&lib_path)?;

    Ok(())
}
