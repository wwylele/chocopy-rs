use super::gimli_writer::*;
use super::*;
use gimli::constants::*;
use gimli::write::*;
use std::collections::HashMap;

const GLOBAL_RELOC_HACK_MAGIC: u32 = 0xDEAD_B00F_u32;

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
    tag.set(DW_AT_type, AttributeValue::ThisUnitEntryRef(member_type_id));
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
    tag.set(DW_AT_type, AttributeValue::ThisUnitEntryRef(pointee));
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
    tag.set(DW_AT_type, AttributeValue::ThisUnitEntryRef(element_type));

    let index_id = dwarf.unit.add(id, DW_TAG_subrange_type);
    let index_tag = dwarf.unit.get_mut(index_id);
    index_tag.set(DW_AT_type, AttributeValue::ThisUnitEntryRef(index_type));
    index_tag.set(DW_AT_count, AttributeValue::ThisUnitEntryRef(len_member));

    id
}

pub(super) struct Dwarf {
    dwarf: DwarfUnit,
    size_t_id: UnitEntryId,
    int_t_id: UnitEntryId,
    char_id: UnitEntryId,
    default_prototype_id: UnitEntryId,
    default_prototype_ptr_id: UnitEntryId,
    debug_types: HashMap<TypeDebug, UnitEntryId>,
    debug_method_types: HashMap<MethodDebug, UnitEntryId>,
    range_list: Vec<Range>,
    procedure_debug_map: HashMap<String, UnitEntryId>,
    symbol_pool: Vec<String>,
}

impl Dwarf {
    pub fn new(source_path: &str, current_dir: &str) -> Dwarf {
        let encoding = gimli::Encoding {
            format: gimli::Format::Dwarf32,
            version: 4,
            address_size: 8,
        };
        let mut dwarf = DwarfUnit::new(encoding);

        dwarf.unit.line_program = LineProgram::new(
            encoding,
            gimli::LineEncoding {
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
        let producer = dwarf.strings.add("chocopy-rs");

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
        let default_prototype_id = dwarf_add_struct_type(&mut dwarf, "object.$prototype", 24);
        dwarf_add_member(&mut dwarf, default_prototype_id, "$size", int_t_id, 0);
        dwarf_add_member(&mut dwarf, default_prototype_id, "$tag", int_t_id, 4);

        let default_prototype_ptr_id =
            dwarf_add_pointer_type(&mut dwarf, None, default_prototype_id);

        Dwarf {
            dwarf,
            size_t_id,
            int_t_id,
            char_id,
            default_prototype_id,
            default_prototype_ptr_id,
            debug_types: HashMap::new(),
            debug_method_types: HashMap::new(),
            range_list: vec![],
            procedure_debug_map: HashMap::new(),
            symbol_pool: vec![],
        }
    }

    pub fn add_types<'a>(&mut self, types: impl IntoIterator<Item = &'a TypeDebug>) {
        let mut array_level_map = HashMap::<&str, u32>::new();
        for type_used in types {
            if let Some(array_level) = array_level_map.get_mut(type_used.core_name.as_str()) {
                *array_level = std::cmp::max(*array_level, type_used.array_level)
            } else {
                array_level_map.insert(&type_used.core_name, type_used.array_level);
            }
        }
        array_level_map.entry("int").or_insert(0);
        array_level_map.entry("str").or_insert(0);
        array_level_map.entry("bool").or_insert(0);
        array_level_map.entry("object").or_insert(0);
        array_level_map.entry("<None>").or_insert(0);

        for (type_name, max_array_level) in array_level_map {
            for array_level in 0..=max_array_level {
                let type_debug = TypeDebug {
                    core_name: type_name.to_owned(),
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
                        if is_array { 24 } else { 16 },
                    );

                    if is_array {
                        dwarf_add_member(
                            &mut self.dwarf,
                            storage_type_id,
                            "$proto",
                            self.default_prototype_ptr_id,
                            0,
                        );

                        dwarf_add_member(
                            &mut self.dwarf,
                            storage_type_id,
                            "$ref",
                            self.size_t_id,
                            8,
                        );

                        let len_id = dwarf_add_member(
                            &mut self.dwarf,
                            storage_type_id,
                            "$len",
                            self.size_t_id,
                            16,
                        );

                        let element_type = if type_string == "str" {
                            self.char_id
                        } else {
                            let mut element_type = type_debug.clone();
                            element_type.array_level -= 1;
                            self.debug_types[&element_type]
                        };
                        let array_type_id = dwarf_add_array_type(
                            &mut self.dwarf,
                            element_type,
                            self.size_t_id,
                            len_id,
                        );
                        dwarf_add_member(
                            &mut self.dwarf,
                            storage_type_id,
                            "$array",
                            array_type_id,
                            24,
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

        let object_init_type = self.add_method_type(MethodDebug {
            params: vec![TypeDebug::class_type("object")],
            return_type: TypeDebug::class_type("<None>"),
        });

        dwarf_add_member(
            &mut self.dwarf,
            self.default_prototype_id,
            "__init__",
            object_init_type,
            16,
        );

        let dtor_type = self.add_method_type(MethodDebug {
            return_type: TypeDebug::class_type("<None>"),
            params: vec![TypeDebug::class_type("object")],
        });

        dwarf_add_member(
            &mut self.dwarf,
            self.default_prototype_id,
            "$dtor",
            dtor_type,
            8,
        );
    }

    pub fn add_method_type(&mut self, method_type: MethodDebug) -> UnitEntryId {
        if let Some(&id) = self.debug_method_types.get(&method_type) {
            return id;
        }

        let tag = dwarf_add_subroutine_type(&mut self.dwarf);
        self.dwarf.unit.get_mut(tag).set(
            DW_AT_type,
            AttributeValue::ThisUnitEntryRef(self.debug_types[&method_type.return_type]),
        );

        for param in &method_type.params {
            let param_tag = self.dwarf.unit.add(tag, DW_TAG_formal_parameter);
            self.dwarf.unit.get_mut(param_tag).set(
                DW_AT_type,
                AttributeValue::ThisUnitEntryRef(self.debug_types[param]),
            )
        }

        let tag = dwarf_add_pointer_type(&mut self.dwarf, None, tag);
        self.debug_method_types.insert(method_type, tag);
        tag
    }

    pub fn add_class(&mut self, class_name: String, class_debug: ClassDebug) {
        let prototype_name = class_name.clone() + ".$prototype";
        let tag_id = self.debug_types[&TypeDebug::class_type(&class_name)];

        let tag_id = if let AttributeValue::ThisUnitEntryRef(id) =
            self.dwarf.unit.get(tag_id).get(DW_AT_type).unwrap()
        {
            *id
        } else {
            panic!()
        };

        self.dwarf.unit.get_mut(tag_id).set(
            DW_AT_byte_size,
            AttributeValue::Udata((class_debug.size + 16) as u64),
        );

        let prototype_id = dwarf_add_struct_type(
            &mut self.dwarf,
            &prototype_name,
            ((class_debug.methods.len() + 2) * 8) as u64,
        );

        dwarf_add_member(&mut self.dwarf, prototype_id, "$size", self.int_t_id, 0);
        dwarf_add_member(&mut self.dwarf, prototype_id, "$tag", self.int_t_id, 4);

        let dtor_type = self.add_method_type(MethodDebug {
            return_type: TypeDebug::class_type("<None>"),
            params: vec![TypeDebug::class_type(&class_name)],
        });

        dwarf_add_member(&mut self.dwarf, prototype_id, "$dtor", dtor_type, 8);

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

        let prototype_ptr_id = dwarf_add_pointer_type(&mut self.dwarf, None, prototype_id);

        dwarf_add_member(&mut self.dwarf, tag_id, "$proto", prototype_ptr_id, 0);
        dwarf_add_member(&mut self.dwarf, tag_id, "$ref", self.size_t_id, 8);

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

    pub fn add_chunk(&mut self, chunk: &Chunk) {
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
                AttributeValue::Udata(chunk.code.len() as u64),
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
            sub_program.set(
                DW_AT_frame_base,
                AttributeValue::Exprloc(Expression(vec![DW_OP_reg6.0])),
            );
            sub_program.set(
                DW_AT_type,
                AttributeValue::ThisUnitEntryRef(self.debug_types[&procedure_debug.return_type]),
            );
            if procedure_debug.parent.is_some() {
                sub_program.set(
                    DW_AT_static_link,
                    AttributeValue::Exprloc(Expression(vec![
                        DW_OP_fbreg.0,
                        0x78, // -8 in SLEB128
                        DW_OP_deref.0,
                    ])),
                )
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
                let mut offset_expr = vec![DW_OP_fbreg.0];
                gimli::leb128::write::signed(&mut offset_expr, var.offset as i64).unwrap();

                node.set(
                    DW_AT_location,
                    AttributeValue::Exprloc(Expression(offset_expr)),
                );

                node.set(DW_AT_name, AttributeValue::String(var.name.as_str().into()));

                node.set(DW_AT_decl_file, AttributeValue::Data1(1));

                node.set(DW_AT_decl_line, AttributeValue::Udata(var.line as u64));

                node.set(
                    DW_AT_type,
                    AttributeValue::ThisUnitEntryRef(self.debug_types[&var.var_type]),
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

    pub fn finalize_code_range(&mut self) {
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
    }

    pub fn add_global(&mut self, global_debug: VarDebug) {
        let root_id = self.dwarf.unit.root();
        let node_id = self.dwarf.unit.add(root_id, DW_TAG_variable);
        let node = self.dwarf.unit.get_mut(node_id);

        let mut location = vec![DW_OP_addr.0];
        location.extend_from_slice(&global_debug.offset.to_le_bytes());
        location.extend_from_slice(&GLOBAL_RELOC_HACK_MAGIC.to_le_bytes());

        node.set(
            DW_AT_location,
            AttributeValue::Exprloc(Expression(location)),
        );

        node.set(DW_AT_name, AttributeValue::String(global_debug.name.into()));

        node.set(DW_AT_decl_file, AttributeValue::Data1(1));

        node.set(
            DW_AT_decl_line,
            AttributeValue::Udata(global_debug.line as u64),
        );

        node.set(
            DW_AT_type,
            AttributeValue::ThisUnitEntryRef(self.debug_types[&global_debug.var_type]),
        );
    }

    pub fn finalize(mut self) -> Vec<DebugChunk> {
        let mut chunks = vec![];

        let mut dwarf_sections = Sections::new(DwarfWriter::new());
        self.dwarf.write(&mut dwarf_sections).unwrap();

        dwarf_sections
            .for_each_mut(|id, data| -> std::result::Result<(), () /*should be !*/> {
                let (mut data, relocs, self_relocs) = data.take();
                let mut links = vec![];

                if data.len() >= 4 {
                    for i in 0..data.len() - 3 {
                        if data[i..i + 4] == GLOBAL_RELOC_HACK_MAGIC.to_le_bytes() {
                            data[i..i + 4].copy_from_slice(&[0; 4]);
                            links.push(DebugChunkLink {
                                pos: i - 4,
                                to: GLOBAL_SECTION.to_owned(),
                                size: 8,
                            });
                        }
                    }
                }

                for reloc in relocs {
                    links.push(DebugChunkLink {
                        pos: reloc.offset,
                        to: self.symbol_pool[reloc.symbol].to_owned(),
                        size: reloc.size,
                    });
                }

                for self_reloc in self_relocs {
                    links.push(DebugChunkLink {
                        pos: self_reloc.offset,
                        to: self_reloc.section.to_owned(),
                        size: self_reloc.size,
                    });
                }

                chunks.push(DebugChunk {
                    name: id.name().to_owned(),
                    code: data,
                    links,
                });

                Ok(())
            })
            .unwrap();

        chunks
    }
}
