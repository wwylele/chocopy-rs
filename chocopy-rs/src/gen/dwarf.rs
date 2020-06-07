// Debug info generator for DWARF (Linux/MacOS)

use super::debug::*;
use super::gimli_writer::*;
use super::*;
use chocopy_rs_common::*;
use gimli::{constants::*, write::*, *};
use std::collections::HashMap;

fn dwarf_add_base_type(
    dwarf: &mut DwarfUnit,
    name: &str,
    encoding: DwAte,
    byte_size: u8,
) -> UnitEntryId {
    let root_id = dwarf.unit.root();
    let id = dwarf.unit.add(root_id, DW_TAG_base_type);
    let tag = dwarf.unit.get_mut(id);
    tag.set(DW_AT_name, AttributeValue::String(name.into()));
    tag.set(DW_AT_encoding, AttributeValue::Encoding(encoding));
    tag.set(DW_AT_byte_size, AttributeValue::Data1(byte_size));
    id
}

fn dwarf_add_struct_type(dwarf: &mut DwarfUnit, name: &str, byte_size: u64) -> UnitEntryId {
    let root_id = dwarf.unit.root();
    let id = dwarf.unit.add(root_id, DW_TAG_structure_type);
    let tag = dwarf.unit.get_mut(id);
    tag.set(DW_AT_name, AttributeValue::String(name.into()));
    tag.set(DW_AT_byte_size, AttributeValue::Udata(byte_size));
    id
}

fn dwarf_add_member(
    dwarf: &mut DwarfUnit,
    parent_id: UnitEntryId,
    name: &str,
    member_type_id: UnitEntryId,
    offset: u64,
) -> UnitEntryId {
    let id = dwarf.unit.add(parent_id, DW_TAG_member);
    let tag = dwarf.unit.get_mut(id);
    tag.set(DW_AT_name, AttributeValue::String(name.into()));
    tag.set(DW_AT_data_member_location, AttributeValue::Udata(offset));
    tag.set(DW_AT_type, AttributeValue::UnitRef(member_type_id));
    id
}

fn dwarf_add_pointer_type(
    dwarf: &mut DwarfUnit,
    name: Option<&str>,
    pointee: UnitEntryId,
) -> UnitEntryId {
    let root_id = dwarf.unit.root();
    let id = dwarf.unit.add(root_id, DW_TAG_pointer_type);
    let tag = dwarf.unit.get_mut(id);
    if let Some(name) = name {
        tag.set(DW_AT_name, AttributeValue::String(name.into()));
    }
    tag.set(DW_AT_type, AttributeValue::UnitRef(pointee));
    tag.set(DW_AT_byte_size, AttributeValue::Data1(8));
    id
}

fn dwarf_add_subroutine_type(dwarf: &mut DwarfUnit) -> UnitEntryId {
    let root_id = dwarf.unit.root();
    dwarf.unit.add(root_id, DW_TAG_subroutine_type)
}

fn dwarf_add_array_type(
    dwarf: &mut DwarfUnit,
    element_type: UnitEntryId,
    index_type: UnitEntryId,
    len_member: UnitEntryId,
) -> UnitEntryId {
    let root_id = dwarf.unit.root();
    let id = dwarf.unit.add(root_id, DW_TAG_array_type);
    let tag = dwarf.unit.get_mut(id);
    tag.set(DW_AT_type, AttributeValue::UnitRef(element_type));

    let index_id = dwarf.unit.add(id, DW_TAG_subrange_type);
    let index_tag = dwarf.unit.get_mut(index_id);
    index_tag.set(DW_AT_type, AttributeValue::UnitRef(index_type));
    index_tag.set(DW_AT_count, AttributeValue::UnitRef(len_member));

    id
}

#[derive(PartialEq, Eq)]
pub(super) enum DwarfFlavor {
    Linux,
    Macos,
}

pub(super) struct Dwarf {
    flavor: DwarfFlavor,
    dwarf: DwarfUnit,
    size_t_id: UnitEntryId,
    int_t_id: UnitEntryId,
    char_id: UnitEntryId,
    object_prototype_id: UnitEntryId,
    object_prototype_ptr_id: UnitEntryId,
    debug_types: HashMap<TypeDebug, UnitEntryId>,
    debug_method_types: HashMap<MethodDebug, UnitEntryId>,
    range_list: Vec<Range>,
    procedure_debug_map: HashMap<String, UnitEntryId>,
    symbol_pool: Vec<String>,
}

impl Dwarf {
    pub fn new(flavor: DwarfFlavor, source_path: &str, current_dir: &str) -> Dwarf {
        let version = match flavor {
            DwarfFlavor::Linux => 4,
            DwarfFlavor::Macos => 2,
        };
        let encoding = Encoding {
            format: Format::Dwarf32,
            version,
            address_size: 8,
        };
        let mut dwarf = DwarfUnit::new(encoding);

        dwarf.unit.line_program = LineProgram::new(
            encoding,
            LineEncoding {
                minimum_instruction_length: 1,
                maximum_operations_per_instruction: 1,
                default_is_stmt: true,
                line_base: -5,
                line_range: 14,
            },
            LineString::String(current_dir.into()),
            LineString::String(source_path.into()),
            None,
        );

        dwarf.unit.line_program.add_file(
            LineString::String(source_path.into()),
            dwarf.unit.line_program.default_directory(),
            None,
        );

        let comp_dir = dwarf.strings.add(current_dir);
        let comp_name = dwarf.strings.add(source_path);
        let producer = dwarf.strings.add(format!(
            "{} {}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION")
        ));

        let root_id = dwarf.unit.root();
        let compile_unit = dwarf.unit.get_mut(root_id);
        // I wanted to set this to python but LLDB didn't like it
        compile_unit.set(DW_AT_language, AttributeValue::Language(DW_LANG_C11));
        compile_unit.set(DW_AT_comp_dir, AttributeValue::StringRef(comp_dir));
        compile_unit.set(DW_AT_name, AttributeValue::StringRef(comp_name));
        compile_unit.set(DW_AT_producer, AttributeValue::StringRef(producer));

        let size_t_id = dwarf_add_base_type(&mut dwarf, "$size_t", DW_ATE_signed, 8);
        let int_t_id = dwarf_add_base_type(&mut dwarf, "$int_t", DW_ATE_signed, 4);
        let char_id = dwarf_add_base_type(&mut dwarf, "$char", DW_ATE_signed, 1);

        let object_prototype_id =
            dwarf_add_struct_type(&mut dwarf, "object", OBJECT_PROTOTYPE_SIZE as u64);
        let object_prototype_ptr_id = dwarf_add_pointer_type(&mut dwarf, None, object_prototype_id);

        Dwarf {
            flavor,
            dwarf,
            size_t_id,
            int_t_id,
            char_id,
            object_prototype_id,
            object_prototype_ptr_id,
            debug_types: HashMap::new(),
            debug_method_types: HashMap::new(),
            range_list: vec![],
            procedure_debug_map: HashMap::new(),
            symbol_pool: vec![],
        }
    }

    fn add_method_type(&mut self, method_type: MethodDebug) -> UnitEntryId {
        if let Some(&id) = self.debug_method_types.get(&method_type) {
            return id;
        }

        let tag = dwarf_add_subroutine_type(&mut self.dwarf);
        self.dwarf.unit.get_mut(tag).set(
            DW_AT_type,
            AttributeValue::UnitRef(self.debug_types[&method_type.return_type]),
        );

        for param in &method_type.params {
            let param_tag = self.dwarf.unit.add(tag, DW_TAG_formal_parameter);
            self.dwarf
                .unit
                .get_mut(param_tag)
                .set(DW_AT_type, AttributeValue::UnitRef(self.debug_types[param]))
        }

        let tag = dwarf_add_pointer_type(&mut self.dwarf, None, tag);
        self.debug_method_types.insert(method_type, tag);
        tag
    }
}

impl DebugWriter for Dwarf {
    fn add_type<'a>(&mut self, type_repr: TypeDebugRepresentive<'a>) {
        for array_level in 0..=type_repr.max_array_level {
            let type_debug = TypeDebug {
                core_name: type_repr.core_name.to_owned(),
                array_level,
            };
            if self.debug_types.contains_key(&type_debug) {
                continue;
            }
            let node_id = if type_debug.array_level == 0 && type_debug.core_name == "bool" {
                dwarf_add_base_type(&mut self.dwarf, "bool", DW_ATE_boolean, 1)
            } else if type_debug.array_level == 0 && type_debug.core_name == "int" {
                dwarf_add_base_type(&mut self.dwarf, "int", DW_ATE_signed, 4)
            } else if type_debug.array_level == 0 && type_debug.core_name == "<None>" {
                dwarf_add_base_type(&mut self.dwarf, "<None>", DW_ATE_address, 8)
            } else {
                let type_string = type_debug.to_string();
                let is_array = array_level != 0 || type_string == "str";

                let storage_type_id = dwarf_add_struct_type(
                    &mut self.dwarf,
                    &(type_debug.to_string() + ".$storage"),
                    if is_array {
                        ARRAY_ELEMENT_OFFSET as u64
                    } else {
                        OBJECT_ATTRIBUTE_OFFSET as u64
                    },
                );

                if is_array {
                    dwarf_add_member(
                        &mut self.dwarf,
                        storage_type_id,
                        "$proto",
                        self.object_prototype_ptr_id,
                        OBJECT_PROTOTYPE_OFFSET as u64,
                    );

                    dwarf_add_member(
                        &mut self.dwarf,
                        storage_type_id,
                        "$gc_count",
                        self.size_t_id,
                        OBJECT_GC_COUNT_OFFSET as u64,
                    );

                    dwarf_add_member(
                        &mut self.dwarf,
                        storage_type_id,
                        "$gc_next",
                        self.size_t_id,
                        OBJECT_GC_NEXT_OFFSET as u64,
                    );

                    let len_id = dwarf_add_member(
                        &mut self.dwarf,
                        storage_type_id,
                        "$len",
                        self.size_t_id,
                        ARRAY_LEN_OFFSET as u64,
                    );

                    let element_type = if type_string == "str" {
                        self.char_id
                    } else {
                        let mut element_type = type_debug.clone();
                        element_type.array_level -= 1;
                        self.debug_types[&element_type]
                    };
                    let array_type_id =
                        dwarf_add_array_type(&mut self.dwarf, element_type, self.size_t_id, len_id);
                    dwarf_add_member(
                        &mut self.dwarf,
                        storage_type_id,
                        "$array",
                        array_type_id,
                        ARRAY_ELEMENT_OFFSET as u64,
                    );
                }

                dwarf_add_pointer_type(
                    &mut self.dwarf,
                    Some(&type_debug.to_string()),
                    storage_type_id,
                )
            };
            self.debug_types.insert(type_debug, node_id);
        }
    }

    fn add_class(&mut self, class_name: String, class_debug: ClassDebug) {
        let prototype_name = class_name.clone() + ".$prototype";
        let tag_id = self.debug_types[&TypeDebug::class_type(&class_name)];

        let tag_id = if let AttributeValue::UnitRef(id) =
            self.dwarf.unit.get(tag_id).get(DW_AT_type).unwrap()
        {
            *id
        } else {
            panic!()
        };

        self.dwarf.unit.get_mut(tag_id).set(
            DW_AT_byte_size,
            AttributeValue::Udata((class_debug.size + OBJECT_ATTRIBUTE_OFFSET) as u64),
        );

        let prototype_id = if class_name == "object" {
            self.object_prototype_id
        } else {
            dwarf_add_struct_type(
                &mut self.dwarf,
                &prototype_name,
                (class_debug.methods.len() * 8) as u64 + PROTOTYPE_INIT_OFFSET as u64,
            )
        };

        dwarf_add_member(
            &mut self.dwarf,
            prototype_id,
            "$size",
            self.int_t_id,
            PROTOTYPE_SIZE_OFFSET as u64,
        );
        dwarf_add_member(
            &mut self.dwarf,
            prototype_id,
            "$tag",
            self.int_t_id,
            PROTOTYPE_TAG_OFFSET as u64,
        );

        dwarf_add_member(
            &mut self.dwarf,
            prototype_id,
            "$map",
            self.int_t_id,
            PROTOTYPE_MAP_OFFSET as u64,
        );

        for (offset, (method, method_type)) in class_debug.methods {
            let method_type = self.add_method_type(method_type);
            dwarf_add_member(
                &mut self.dwarf,
                prototype_id,
                &method,
                method_type,
                offset as u64,
            );
        }

        let prototype_ptr_id = if class_name == "object" {
            self.object_prototype_ptr_id
        } else {
            dwarf_add_pointer_type(&mut self.dwarf, None, prototype_id)
        };

        dwarf_add_member(
            &mut self.dwarf,
            tag_id,
            "$proto",
            prototype_ptr_id,
            OBJECT_PROTOTYPE_OFFSET as u64,
        );
        dwarf_add_member(
            &mut self.dwarf,
            tag_id,
            "$gc_count",
            self.size_t_id,
            OBJECT_GC_COUNT_OFFSET as u64,
        );
        dwarf_add_member(
            &mut self.dwarf,
            tag_id,
            "$gc_next",
            self.size_t_id,
            OBJECT_GC_NEXT_OFFSET as u64,
        );

        for attribute in class_debug.attributes {
            dwarf_add_member(
                &mut self.dwarf,
                tag_id,
                &attribute.name,
                self.debug_types[&attribute.var_type],
                attribute.offset as u64,
            );
        }
    }

    fn add_chunk(&mut self, chunk: &Chunk) {
        if let ChunkExtra::Procedure(procedure_debug) = &chunk.extra {
            let parent_id = if let Some(parent) = &procedure_debug.parent {
                self.procedure_debug_map[parent]
            } else {
                self.dwarf.unit.root()
            };

            let sub_program_id = self.dwarf.unit.add(parent_id, DW_TAG_subprogram);
            self.procedure_debug_map
                .insert(chunk.name.clone(), sub_program_id);

            let sub_program = self.dwarf.unit.get_mut(sub_program_id);
            sub_program.set(
                DW_AT_low_pc,
                AttributeValue::Address(Address::Symbol {
                    symbol: self.symbol_pool.len(),
                    addend: 0,
                }),
            );
            sub_program.set(
                DW_AT_high_pc,
                AttributeValue::Address(Address::Symbol {
                    symbol: self.symbol_pool.len(),
                    addend: chunk.code.len() as i64,
                }),
            );
            sub_program.set(DW_AT_decl_file, AttributeValue::Data1(1));
            sub_program.set(
                DW_AT_decl_line,
                AttributeValue::Udata(procedure_debug.decl_line as u64),
            );
            sub_program.set(
                DW_AT_name,
                AttributeValue::String(chunk.name.as_bytes().into()),
            );
            sub_program.set(
                DW_AT_artificial,
                AttributeValue::Flag(procedure_debug.artificial),
            );
            let mut frame_base = Expression::new();
            frame_base.op_reg(Register(6));
            sub_program.set(DW_AT_frame_base, AttributeValue::Exprloc(frame_base));
            sub_program.set(
                DW_AT_type,
                AttributeValue::UnitRef(self.debug_types[&procedure_debug.return_type]),
            );
            if procedure_debug.parent.is_some() {
                let mut static_link = Expression::new();
                static_link.op_fbreg(-8);
                static_link.op_deref();
                sub_program.set(DW_AT_static_link, AttributeValue::Exprloc(static_link))
            }

            if !procedure_debug.lines.is_empty() {
                let line_program = &mut self.dwarf.unit.line_program;
                line_program.begin_sequence(Some(Address::Symbol {
                    symbol: self.symbol_pool.len(),
                    addend: 0,
                }));

                for line_map in &procedure_debug.lines {
                    line_program.row().address_offset = line_map.code_pos as u64;
                    line_program.row().line = line_map.line_number as u64;
                    line_program.generate_row();
                }

                line_program.end_sequence(chunk.code.len() as u64);
            }

            for (var, is_param) in procedure_debug
                .params
                .iter()
                .zip(std::iter::repeat(true))
                .chain(procedure_debug.locals.iter().zip(std::iter::repeat(false)))
            {
                let node_id = self.dwarf.unit.add(
                    sub_program_id,
                    if is_param {
                        DW_TAG_formal_parameter
                    } else {
                        DW_TAG_variable
                    },
                );
                let node = self.dwarf.unit.get_mut(node_id);
                let mut offset_expr = Expression::new();
                offset_expr.op_fbreg(var.offset as i64);
                node.set(DW_AT_location, AttributeValue::Exprloc(offset_expr));

                node.set(DW_AT_name, AttributeValue::String(var.name.as_str().into()));

                node.set(DW_AT_decl_file, AttributeValue::Data1(1));

                node.set(DW_AT_decl_line, AttributeValue::Udata(var.line as u64));

                node.set(
                    DW_AT_type,
                    AttributeValue::UnitRef(self.debug_types[&var.var_type]),
                );
            }

            self.range_list.push(Range::StartLength {
                begin: Address::Symbol {
                    symbol: self.symbol_pool.len(),
                    addend: 0,
                },
                length: chunk.code.len() as u64,
            });
            self.symbol_pool.push(chunk.name.clone());
        }
    }

    fn add_global(&mut self, global_debug: VarDebug) {
        let root_id = self.dwarf.unit.root();
        let node_id = self.dwarf.unit.add(root_id, DW_TAG_variable);
        let node = self.dwarf.unit.get_mut(node_id);

        let mut location = Expression::new();
        location.op_addr(Address::Symbol {
            symbol: self.symbol_pool.len(),
            addend: global_debug.offset as i64,
        });
        self.symbol_pool.push(GLOBAL_SECTION.to_owned());

        node.set(DW_AT_location, AttributeValue::Exprloc(location));

        node.set(DW_AT_name, AttributeValue::String(global_debug.name.into()));

        node.set(DW_AT_decl_file, AttributeValue::Data1(1));

        node.set(
            DW_AT_decl_line,
            AttributeValue::Udata(global_debug.line as u64),
        );

        node.set(
            DW_AT_type,
            AttributeValue::UnitRef(self.debug_types[&global_debug.var_type]),
        );
    }

    fn finalize(mut self: Box<Self>) -> Vec<DebugChunk> {
        let range_list = self
            .dwarf
            .unit
            .ranges
            .add(RangeList(std::mem::replace(&mut self.range_list, vec![])));
        let root_id = self.dwarf.unit.root();
        self.dwarf
            .unit
            .get_mut(root_id)
            .set(DW_AT_ranges, AttributeValue::RangeListRef(range_list));

        let mut chunks = vec![];

        let mut dwarf_sections = Sections::new(DwarfWriter::new());
        self.dwarf.write(&mut dwarf_sections).unwrap();

        dwarf_sections
            .for_each_mut(|id, data| -> std::result::Result<(), () /*should be !*/> {
                let (data, relocs, self_relocs) = data.take();
                let mut links = vec![];

                for reloc in relocs {
                    links.push(DebugChunkLink {
                        link_type: DebugChunkLinkType::Absolute,
                        pos: reloc.offset,
                        to: self.symbol_pool[reloc.symbol].to_owned(),
                        size: reloc.size,
                    });
                }

                if self.flavor == DwarfFlavor::Linux {
                    for self_reloc in self_relocs {
                        links.push(DebugChunkLink {
                            link_type: DebugChunkLinkType::Absolute,
                            pos: self_reloc.offset,
                            to: self_reloc.section.to_owned(),
                            size: self_reloc.size,
                        });
                    }
                }

                chunks.push(DebugChunk {
                    name: id.name().to_owned(),
                    code: data,
                    links,
                    discardable: true,
                });

                Ok(())
            })
            .unwrap();

        chunks
    }
}
