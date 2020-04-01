mod dwarf;
mod gimli_writer;
mod x64;

use crate::local_env::*;
use crate::node::*;
use faerie::*;
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

struct ChunkLink {
    pos: usize,
    to: String,
}

struct Chunk {
    name: String,
    code: Vec<u8>,
    links: Vec<ChunkLink>,
    extra: ChunkExtra,
}

struct DebugChunkLink {
    pos: usize,
    to: String,
    size: u8,
}

struct DebugChunk {
    name: String,
    code: Vec<u8>,
    links: Vec<DebugChunkLink>,
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
        // global
        (GLOBAL_SECTION, Decl::data().writable().into()),
    ];

    let current_dir_buf = std::env::current_dir();
    let current_dir = current_dir_buf
        .as_ref()
        .map(|s| s.to_str())
        .ok()
        .flatten()
        .unwrap_or("");

    let mut dwarf = dwarf::Dwarf::new(source_path, current_dir);

    obj.declarations(declarations.into_iter())?;

    let code_set = x64::gen_code_set(ast);

    dwarf.add_types(code_set.used_types());

    for (class_name, classes_debug) in code_set.classes_debug {
        dwarf.add_class(class_name, classes_debug);
    }

    obj.define(GLOBAL_SECTION, vec![0; code_set.global_size])?;

    for global_debug in code_set.globals_debug {
        dwarf.add_global(global_debug);
    }

    for chunk in &code_set.chunks {
        if let ChunkExtra::Procedure(_) = chunk.extra {
            if chunk.name == BUILTIN_CHOCOPY_MAIN || chunk.name == "object.__init__" {
                obj.declare(&chunk.name, Decl::function().global())?;
            } else {
                obj.declare(&chunk.name, Decl::function().local())?
            }
        } else {
            obj.declare(&chunk.name, Decl::data())?;
        }
    }

    for chunk in code_set.chunks {
        dwarf.add_chunk(&chunk);
        obj.define(&chunk.name, chunk.code)?;
        for link in chunk.links {
            obj.link(Link {
                from: &chunk.name,
                to: &link.to,
                at: link.pos as u64,
            })?;
        }
    }

    dwarf.finalize_code_range();

    for chunk in dwarf.finalize() {
        obj.declare(&chunk.name, Decl::section(SectionKind::Debug))?;
        obj.define(&chunk.name, chunk.code)?;
        for link in chunk.links {
            obj.link_with(
                Link {
                    from: &chunk.name,
                    to: &link.to,
                    at: link.pos as u64,
                },
                Reloc::Debug {
                    size: link.size,
                    addend: 0,
                },
            )?;
        }
    }

    let obj_file = std::fs::File::create(&obj_path)?;
    obj.write(obj_file)?;

    if no_link {
        return Ok(());
    }

    let mut lib_path = std::env::current_exe()?;
    lib_path.set_file_name("libchocopy_rs_std.a");

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

    Ok(())
}
