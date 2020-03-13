use crate::node::*;
use faerie::*;
use std::convert::*;
use std::io::Write;
use std::str::FromStr;
use target_lexicon::*;

struct ProcedureLink {
    pos: usize,
    to: String,
}

struct Procedure {
    name: String,
    code: Vec<u8>,
    links: Vec<ProcedureLink>,
}

struct CodeSet {
    procedures: Vec<Procedure>,
}

struct Emitter {
    name: String,
    code: Vec<u8>,
    links: Vec<ProcedureLink>,
    rsp_aligned: bool,
    rsp_call_restore: Vec<(usize, bool)>,
}

impl Emitter {
    pub fn new(name: &str) -> Emitter {
        Emitter {
            name: name.to_owned(),
            // push rbp; mov rbp,rsp
            code: vec![0x55, 0x48, 0x89, 0xe5],
            links: vec![],
            rsp_aligned: true,
            rsp_call_restore: vec![],
        }
    }

    pub fn emit(&mut self, instruction: &[u8]) {
        self.code.extend_from_slice(&instruction);
    }

    pub fn pos(&self) -> usize {
        self.code.len()
    }

    pub fn end_proc(&mut self) {
        // mov rsp,rbp; pop rbp; ret
        self.emit(&[0x48, 0x89, 0xec, 0x5d, 0xc3])
    }

    pub fn prepare_call(&mut self, param_count: usize) {
        let mut spill = param_count.saturating_sub(6);
        if self.rsp_aligned != (spill % 2 == 0) {
            spill += 1;
        }
        spill *= 8;
        assert!(spill < 128);
        // sub rsp,{spill}
        self.emit(&[0x48, 0x83, 0xEC, spill as u8]);
        self.rsp_call_restore.push((spill, self.rsp_aligned));
        self.rsp_aligned = true;
    }

    pub fn call(&mut self, name: &str) {
        self.emit(&[0xe8]);
        self.links.push(ProcedureLink {
            pos: self.pos(),
            to: name.to_owned(),
        });
        self.emit(&[0x00, 0x00, 0x00, 0x00]);
        let (spill, rsp_aligned) = self.rsp_call_restore.pop().unwrap();
        assert!(spill < 128);
        // add rsp,{spill}
        self.emit(&[0x48, 0x83, 0xC4, spill as u8]);
        self.rsp_aligned = rsp_aligned;
    }

    pub fn finalize(self) -> Procedure {
        Procedure {
            name: self.name,
            code: self.code,
            links: self.links,
        }
    }

    pub fn emit_push_r10(&mut self) {
        self.emit(&[0x41, 0x52]);
        self.rsp_aligned = !self.rsp_aligned;
    }

    pub fn emit_pop_r11(&mut self) {
        self.emit(&[0x41, 0x5B]);
        self.rsp_aligned = !self.rsp_aligned;
    }

    pub fn emit_box_int(&mut self) {
        self.emit_push_r10();
        self.prepare_call(2);
        // mov rdi,[rip+{p^INT_PROTOTYPE}]
        self.emit(&[0x48, 0x8B, 0x3D]);
        self.links.push(ProcedureLink {
            pos: self.pos(),
            to: "INT_PROTOTYPE".to_owned(),
        });
        self.emit(&[0; 4]);

        // xor rsi,rsi
        self.emit(&[0x48, 0x31, 0xF6]);
        self.call("alloc_obj");
        self.emit_pop_r11();
        // mov QWORD PTR [rax+0x10],r11
        self.emit(&[0x4C, 0x89, 0x58, 0x10]);
        // mov r10,rax
        self.emit(&[0x49, 0x89, 0xC2]);
    }

    pub fn emit_expression(&mut self, expression: &Expr) {
        match &expression.content {
            ExprContent::IntegerLiteral(i) => {
                // mov r10,{i}
                self.emit(&[0x49, 0xc7, 0xc2]);
                self.emit(&i.value.to_le_bytes());
            }
            ExprContent::CallExpr(c) => {
                self.prepare_call(c.args.len());

                for (i, arg) in c.args.iter().enumerate() {
                    self.emit_expression(arg);

                    let param_type = if let Some(Type::FuncType(f)) = &c.function.id().inferred_type
                    {
                        &f.parameters[0]
                    } else {
                        panic!();
                    };
                    if param_type == &TYPE_OBJECT.clone().into() {
                        if arg.inferred_type.as_ref().unwrap() == &TYPE_INT.clone().into() {
                            self.emit_box_int();
                        }
                    }

                    match i {
                        // mov rdi,r10
                        0 => self.emit(&[0x4c, 0x89, 0xd7]),
                        // mov rsi,r10
                        1 => self.emit(&[0x4c, 0x89, 0xd6]),
                        // mov rdx,r10
                        2 => self.emit(&[0x4c, 0x89, 0xd2]),
                        // mov rcx,r10
                        3 => self.emit(&[0x4c, 0x89, 0xd1]),
                        // mov r8,r10
                        4 => self.emit(&[0x4d, 0x89, 0xd0]),
                        // mov r9,r10
                        5 => self.emit(&[0x4d, 0x89, 0xd1]),
                        _ => {
                            let offset = (i - 6) * 8;
                            assert!(offset < 128);
                            // mov QWORD PTR [rsp+{offset}],r10
                            self.emit(&[0x4c, 0x89, 0x54, 0x24, offset as u8]);
                        }
                    }
                }

                self.call(&c.function.id().name);
                // mov r10,rax
                self.emit(&[0x49, 0x89, 0xC2]);
            }
            _ => (),
        }
    }

    pub fn emit_statement(&mut self, statement: &Stmt) {
        match statement {
            Stmt::ExprStmt(e) => {
                self.emit_expression(&e.expr);
            }
            _ => (),
        }
    }
}

fn gen_code_set(ast: Ast) -> CodeSet {
    let mut main_code = Emitter::new("chocopy_main");

    for statement in &ast.program().statements {
        main_code.emit_statement(statement);
    }

    //main_code.prepare_call(1);
    //main_code.emit(&[0xbf, 0x2a, 0x00, 0x00, 0x00]);
    //main_code.call("debug_print");
    main_code.end_proc();

    let main_procedure = main_code.finalize();

    CodeSet {
        procedures: vec![main_procedure],
    }
}

pub fn gen(ast: Ast, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let obj_name = format!("chocopy-{}.o", rand::random::<u32>());
    let mut obj_path = std::env::temp_dir();
    obj_path.push(obj_name.clone());

    let mut obj = ArtifactBuilder::new(triple!("x86_64-pc-linux-gnu-elf"))
        .name(obj_name)
        .finish();

    obj.declarations(
        [
            // prototypes
            ("BOOL_PROTOTYPE", Decl::data_import().into()),
            ("INT_PROTOTYPE", Decl::data_import().into()),
            ("STR_PROTOTYPE", Decl::data_import().into()),
            ("OBJECT_PROTOTYPE", Decl::data_import().into()),
            ("BOOL_LIST_PROTOTYPE", Decl::data_import().into()),
            ("INT_LIST_PROTOTYPE", Decl::data_import().into()),
            ("OBJECT_LIST_PROTOTYPE", Decl::data_import().into()),
            // allocation
            ("alloc_obj", Decl::function_import().into()),
            ("free_obj", Decl::function_import().into()),
            // built-in functions
            ("debug_print", Decl::function_import().into()),
            ("len", Decl::function_import().into()),
            ("print", Decl::function_import().into()),
            ("input", Decl::function_import().into()),
            // main
            ("chocopy_main", Decl::function().global().into()),
        ]
        .iter()
        .cloned(),
    )?;

    let code_set = gen_code_set(ast);
    for procedure in code_set.procedures {
        obj.define(&procedure.name, procedure.code)?;
        for link in procedure.links {
            obj.link(Link {
                from: &procedure.name,
                to: &link.to,
                at: link.pos as u64,
            })?;
        }
    }

    let obj_file = std::fs::File::create(&obj_path)?;
    obj.write(obj_file)?;

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
