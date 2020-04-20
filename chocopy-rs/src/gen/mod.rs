mod dwarf;
mod gimli_writer;
mod x64;

use crate::local_env::*;
use crate::node::*;
use std::collections::HashMap;
use std::convert::*;
use std::ffi::OsStr;
use std::io::Write;
use std::path::*;
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
const BUILTIN_LEN: &'static str = "$len";
const BUILTIN_INPUT: &'static str = "$input";
const BUILTIN_PRINT: &'static str = "$print";

const BUILTIN_CHOCOPY_MAIN: &'static str = "$chocopy_main";

const GLOBAL_SECTION: &'static str = "$global";

#[allow(unused)]
#[derive(PartialEq, Eq)]
enum Platform {
    Windows,
    Linux,
}

#[cfg(target_os = "windows")]
const PLATFORM: Platform = Platform::Windows;

#[cfg(target_os = "linux")]
const PLATFORM: Platform = Platform::Linux;

#[derive(PartialEq, Eq, Hash, Clone)]
struct TypeDebug {
    core_name: String,
    array_level: u32,
}

impl TypeDebug {
    fn class_type(name: &str) -> TypeDebug {
        TypeDebug {
            core_name: name.to_owned(),
            array_level: 0,
        }
    }
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

#[derive(Clone, PartialEq, Eq, Hash)]
struct MethodDebug {
    params: Vec<TypeDebug>,
    return_type: TypeDebug,
}

#[derive(Clone)]
struct ClassDebug {
    size: u32,
    attributes: Vec<VarDebug>,
    methods: HashMap<u32, (String, MethodDebug)>,
}

impl ClassDebug {
    fn used_types(&self) -> impl Iterator<Item = &TypeDebug> {
        self.attributes.iter().map(|attribute| &attribute.var_type)
    }
}

struct CodeSet {
    chunks: Vec<Chunk>,
    global_size: u64,
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

#[derive(Debug)]
struct ToolChainError;

impl std::fmt::Display for ToolChainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Failed to find MSVC tools. Please install Visual Studio or Visual C++ Build Tools"
        )
    }
}

impl std::error::Error for ToolChainError {}

#[derive(Debug)]
struct PathError;

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Illegal path")
    }
}

impl std::error::Error for PathError {}

fn windows_path_escape(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let path = path.to_str().ok_or(PathError)?;

    // TODO: actually escape the path
    // For now we just forbid suspicious strings.
    if path
        .find(|c| matches!(c, '\"' | '\'' | '^') || c.is_control())
        .is_some()
        || path.ends_with('\\')
    {
        return Err(PathError.into());
    }

    Ok(path.to_owned())
}

pub fn gen(
    source_path: &str,
    ast: Program,
    path: &str,
    no_link: bool,
    static_lib: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let current_dir_buf = std::env::current_dir();
    let current_dir = current_dir_buf
        .as_ref()
        .map(|s| s.to_str())
        .ok()
        .flatten()
        .unwrap_or("");

    let mut dwarf = dwarf::Dwarf::new(source_path, current_dir);

    let binary_format = match PLATFORM {
        Platform::Windows => BinaryFormat::Coff,
        Platform::Linux => BinaryFormat::Elf,
    };
    let mut obj = object::write::Object::new(binary_format, Architecture::X86_64);

    let import_function = |obj: &mut object::write::Object, name: &str| {
        obj.add_symbol(object::write::Symbol {
            name: name.into(),
            value: 0,
            size: 0,
            kind: object::SymbolKind::Text,
            scope: object::SymbolScope::Linkage,
            weak: false,
            section: object::write::SymbolSection::Common,
            flags: object::SymbolFlags::None,
        })
    };

    import_function(&mut obj, BUILTIN_ALLOC_OBJ);
    import_function(&mut obj, BUILTIN_FREE_OBJ);
    import_function(&mut obj, BUILTIN_BROKEN_STACK);
    import_function(&mut obj, BUILTIN_DIV_ZERO);
    import_function(&mut obj, BUILTIN_OUT_OF_BOUND);
    import_function(&mut obj, BUILTIN_NONE_OP);
    import_function(&mut obj, BUILTIN_LEN);
    import_function(&mut obj, BUILTIN_PRINT);
    import_function(&mut obj, BUILTIN_INPUT);
    import_function(&mut obj, "[object].$dtor");

    let code_set = x64::gen_code_set(ast);

    dwarf.add_types(code_set.used_types());

    for (class_name, classes_debug) in code_set.classes_debug {
        dwarf.add_class(class_name, classes_debug);
    }

    let bss_section = obj.section_id(object::write::StandardSection::UninitializedData);

    let global_symbol = obj.add_symbol(object::write::Symbol {
        name: GLOBAL_SECTION.into(),
        value: 0,
        size: code_set.global_size,
        kind: object::SymbolKind::Data,
        scope: object::SymbolScope::Compilation,
        weak: false,
        section: object::write::SymbolSection::Undefined,
        flags: object::SymbolFlags::None,
    });

    obj.add_symbol_bss(global_symbol, bss_section, code_set.global_size, 8);

    for global_debug in code_set.globals_debug {
        dwarf.add_global(global_debug);
    }

    let mut section_map = HashMap::new();

    for chunk in &code_set.chunks {
        dwarf.add_chunk(&chunk);
        if let ChunkExtra::Procedure(_) = chunk.extra {
            let scope = if chunk.name == BUILTIN_CHOCOPY_MAIN {
                object::SymbolScope::Linkage
            } else {
                object::SymbolScope::Compilation
            };

            let (section, offset) = obj.add_subsection(
                object::write::StandardSection::Text,
                chunk.name.as_bytes(),
                &chunk.code,
                1,
            );

            obj.add_symbol(object::write::Symbol {
                name: chunk.name.as_bytes().into(),
                value: offset,
                size: chunk.code.len() as u64,
                kind: object::SymbolKind::Text,
                scope,
                weak: false,
                section: object::write::SymbolSection::Section(section),
                flags: object::SymbolFlags::None,
            });
            section_map.insert(&chunk.name, (section, offset));
        } else {
            let (section, offset) = obj.add_subsection(
                object::write::StandardSection::ReadOnlyData,
                chunk.name.as_bytes(),
                &chunk.code,
                8,
            );

            obj.add_symbol(object::write::Symbol {
                name: chunk.name.as_bytes().into(),
                value: offset,
                size: chunk.code.len() as u64,
                kind: object::SymbolKind::Data,
                scope: object::SymbolScope::Compilation,
                weak: false,
                section: object::write::SymbolSection::Section(section),
                flags: object::SymbolFlags::None,
            });

            section_map.insert(&chunk.name, (section, offset));
        }
    }

    for chunk in &code_set.chunks {
        let (from, from_offset) = section_map[&chunk.name];
        let size;
        let kind;
        let encoding;
        let addend;
        if let ChunkExtra::Procedure(_) = chunk.extra {
            size = 32;
            kind = object::RelocationKind::Relative;
            encoding = object::RelocationEncoding::X86RipRelative;
            addend = -4;
        } else {
            size = 64;
            kind = object::RelocationKind::Absolute;
            encoding = object::RelocationEncoding::Generic;
            addend = 0;
        };
        for link in &chunk.links {
            obj.add_relocation(
                from,
                object::write::Relocation {
                    offset: from_offset + link.pos as u64,
                    size,
                    kind,
                    encoding,
                    symbol: obj.symbol_id(link.to.as_bytes()).unwrap(),
                    addend,
                },
            )?;
        }
    }

    dwarf.finalize_code_range();

    if PLATFORM == Platform::Linux {
        let debug_chunks = dwarf.finalize();
        let mut debug_section_map = HashMap::new();
        for chunk in &debug_chunks {
            let section = obj.add_section(
                "".into(),
                chunk.name.as_bytes().into(),
                object::SectionKind::Debug,
            );
            obj.append_section_data(section, &chunk.code, 8);
            debug_section_map.insert(chunk.name.clone(), section);
        }

        for chunk in debug_chunks {
            for link in chunk.links {
                let to = obj
                    .symbol_id(link.to.as_bytes())
                    .unwrap_or_else(|| obj.section_symbol(debug_section_map[&link.to]));
                obj.add_relocation(
                    debug_section_map[&chunk.name],
                    object::write::Relocation {
                        offset: link.pos as u64,
                        size: link.size * 8,
                        kind: object::RelocationKind::Absolute,
                        encoding: object::RelocationEncoding::Generic,
                        symbol: to,
                        addend: 0,
                    },
                )?;
            }
        }
    }

    let obj_path = if no_link {
        let obj_path = Path::new(path);
        obj_path.to_owned()
    } else {
        let mut obj_path = std::env::temp_dir();
        let obj_name = format!("chocopy-{}.o", rand::random::<u32>());
        obj_path.push(obj_name.clone());
        obj_path
    };

    let mut obj_file = std::fs::File::create(&obj_path)?;
    obj_file.write_all(&obj.write()?)?;
    drop(obj_file);

    if no_link {
        return Ok(());
    }

    let lib_file = match PLATFORM {
        Platform::Windows => "chocopy_rs_std.lib",
        Platform::Linux => "libchocopy_rs_std.a",
    };

    let mut lib_path = std::env::current_exe()?;
    lib_path.set_file_name(lib_file);

    let ld_output = match PLATFORM {
        Platform::Windows => {
            let vcvarsall = (|| -> Option<PathBuf> {
                let linker = cc::windows_registry::find_tool("x86_64-pc-windows-msvc", "link.exe")?;
                let mut vcvarsall = linker.path();
                vcvarsall = vcvarsall.parent()?;
                vcvarsall = vcvarsall.parent()?;
                vcvarsall = vcvarsall.parent()?;
                vcvarsall = vcvarsall.parent()?;
                vcvarsall = vcvarsall.parent()?;
                vcvarsall = vcvarsall.parent()?;
                vcvarsall = vcvarsall.parent()?;
                Some(
                    vcvarsall
                        .join("Auxiliary")
                        .join("Build")
                        .join("vcvarsall.bat"),
                )
            })()
            .ok_or(ToolChainError)?;

            let libs = if static_lib {
                "libvcruntime.lib libucrt.lib libcmt.lib"
            } else {
                "vcruntime.lib ucrt.lib msvcrt.lib"
            };

            // We need to execute vcvarsall.bat, then link.exe with the
            // inherited environment variables.
            // However, the syntax for chained execution in `cmd` is not in the
            // standard escaping format, and rust std::process::Command doesn't
            // support it. To work around this, we make a temporary batch file
            // with the commands we want, and execute that batch file.
            let batch_content = format!(
                "@echo off
call \"{}\" amd64
link /NOLOGO /NXCOMPAT /OPT:REF,NOICF \
\"{}\" \"{}\" /OUT:\"{}\" \
kernel32.lib advapi32.lib ws2_32.lib userenv.lib {} \
/SUBSYSTEM:CONSOLE",
                windows_path_escape(&vcvarsall)?,
                windows_path_escape(&obj_path)?,
                windows_path_escape(&lib_path)?,
                windows_path_escape(Path::new(path))?,
                libs
            );

            let mut bat_path = std::env::temp_dir();
            let bat_name = format!("chocopy-{}.bat", rand::random::<u32>());
            bat_path.push(bat_name);

            std::fs::write(&bat_path, batch_content)?;

            let ld_output = std::process::Command::new("cmd")
                .args(&[OsStr::new("/c"), bat_path.as_os_str()])
                .output()?;
            std::fs::remove_file(&bat_path)?;
            ld_output
        }
        Platform::Linux => {
            let mut command = std::process::Command::new("gcc");
            command.args(&[
                OsStr::new("-o"),
                OsStr::new(path),
                obj_path.as_os_str(),
                lib_path.as_os_str(),
                OsStr::new("-pthread"),
                OsStr::new("-ldl"),
            ]);
            if static_lib {
                command.arg("-static");
            }
            command.output()?
        }
    };

    if !ld_output.status.success() {
        println!("Error from linker:");
        std::io::stdout().write_all(&ld_output.stdout).unwrap();
        std::io::stderr().write_all(&ld_output.stderr).unwrap();
    }

    std::fs::remove_file(&obj_path)?;

    Ok(())
}
