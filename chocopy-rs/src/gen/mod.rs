use crate::node::*;
use faerie::*;
use std::convert::*;
use std::io::Write;
use std::str::FromStr;
use target_lexicon::*;

const BOOL_PROTOTYPE: &'static str = "$BOOL_PROTOTYPE";
const INT_PROTOTYPE: &'static str = "$INT_PROTOTYPE";
const STR_PROTOTYPE: &'static str = "$STR_PROTOTYPE";
const OBJECT_PROTOTYPE: &'static str = "$OBJECT_PROTOTYPE";
const BOOL_LIST_PROTOTYPE: &'static str = "$BOOL_LIST_PROTOTYPE";
const INT_LIST_PROTOTYPE: &'static str = "$INT_LIST_PROTOTYPE";
const OBJECT_LIST_PROTOTYPE: &'static str = "$OBJECT_LIST_PROTOTYPE";
const BUILTIN_ALLOC_OBJ: &'static str = "$alloc_obj";
const BUILTIN_FREE_OBJ: &'static str = "$free_obj";
const BUILTIN_REPORT_BROKEN_STACK: &'static str = "$report_broken_stack";
const BUILTIN_CHOCOPY_MAIN: &'static str = "$chocopy_main";

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
    strings: Vec<(usize, String)>,
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
            strings: vec![],
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

    pub fn finalize(mut self) -> Procedure {
        for (pos, s) in &self.strings {
            let dest = self.pos();
            self.code.extend_from_slice(s.as_bytes());
            let delta = (dest - *pos - 4) as u32;
            self.code[*pos..*pos + 4].copy_from_slice(&delta.to_le_bytes());
        }

        Procedure {
            name: self.name,
            code: self.code,
            links: self.links,
        }
    }

    pub fn emit_push_rax(&mut self) {
        self.emit(&[0x50]);
        self.rsp_aligned = !self.rsp_aligned;
    }

    pub fn emit_push_r10(&mut self) {
        self.emit(&[0x41, 0x52]);
        self.rsp_aligned = !self.rsp_aligned;
    }

    pub fn emit_push_r11(&mut self) {
        self.emit(&[0x41, 0x53]);
        self.rsp_aligned = !self.rsp_aligned;
    }

    pub fn emit_pop_rax(&mut self) {
        self.emit(&[0x58]);
        self.rsp_aligned = !self.rsp_aligned;
    }

    pub fn emit_pop_r11(&mut self) {
        self.emit(&[0x41, 0x5B]);
        self.rsp_aligned = !self.rsp_aligned;
    }

    pub fn emit_box_int(&mut self) {
        self.emit_push_rax();
        self.prepare_call(2);
        // mov rdi,[rip+{INT_PROTOTYPE}]
        self.emit(&[0x48, 0x8B, 0x3D]);
        self.links.push(ProcedureLink {
            pos: self.pos(),
            to: INT_PROTOTYPE.to_owned(),
        });
        self.emit(&[0; 4]);
        // xor rsi,rsi
        self.emit(&[0x48, 0x31, 0xF6]);
        self.call(BUILTIN_ALLOC_OBJ);
        self.emit_pop_r11();
        // mov DWORD PTR [rax+0x10],r11d
        self.emit(&[0x44, 0x89, 0x58, 0x10]);
    }

    pub fn emit_box_bool(&mut self) {
        self.emit_push_rax();
        self.prepare_call(2);
        // mov rdi,[rip+{BOOL_PROTOTYPE}]
        self.emit(&[0x48, 0x8B, 0x3D]);
        self.links.push(ProcedureLink {
            pos: self.pos(),
            to: BOOL_PROTOTYPE.to_owned(),
        });
        self.emit(&[0; 4]);
        // xor rsi,rsi
        self.emit(&[0x48, 0x31, 0xF6]);
        self.call(BUILTIN_ALLOC_OBJ);
        self.emit_pop_r11();
        // mov BYTE PTR [rax+0x10],r11b
        self.emit(&[0x44, 0x88, 0x58, 0x10]);
    }

    pub fn emit_drop(&mut self) {
        // test rax,rax
        self.emit(&[0x48, 0x85, 0xC0]);
        // je
        self.emit(&[0x0f, 0x84]);
        let pos = self.pos();
        self.emit(&[0; 4]);
        // sub QWORD PTR [rax+8],1
        self.emit(&[0x48, 0x83, 0x68, 0x08, 0x01]);
        {
            // jne
            self.emit(&[0x0f, 0x85]);
            let pos = self.pos();
            self.emit(&[0; 4]);

            self.prepare_call(1);
            // mov rdi,rax
            self.emit(&[0x48, 0x89, 0xc7]);
            self.call(BUILTIN_FREE_OBJ);

            let delta = (self.pos() - pos - 4) as u32;
            self.code[pos..pos + 4].copy_from_slice(&delta.to_le_bytes());
        }

        let delta = (self.pos() - pos - 4) as u32;
        self.code[pos..pos + 4].copy_from_slice(&delta.to_le_bytes());
    }

    pub fn emit_expression(&mut self, expression: &Expr) {
        match &expression.content {
            ExprContent::NoneLiteral(_) => {
                // xor rax,rax
                self.emit(&[0x48, 0x31, 0xC0]);
            }
            ExprContent::IntegerLiteral(i) => {
                // mov rax,{i}
                self.emit(&[0x48, 0xc7, 0xc0]);
                self.emit(&i.value.to_le_bytes());
            }
            ExprContent::BooleanLiteral(b) => {
                if b.value {
                    // mov rax,1
                    self.emit(&[0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00]);
                } else {
                    // xor rax,rax
                    self.emit(&[0x48, 0x31, 0xC0]);
                }
            }
            ExprContent::StringLiteral(s) => {
                self.prepare_call(2);
                // mov rdi,[rip+{STR_PROTOTYPE}]
                self.emit(&[0x48, 0x8B, 0x3D]);
                self.links.push(ProcedureLink {
                    pos: self.pos(),
                    to: STR_PROTOTYPE.to_owned(),
                });
                self.emit(&[0; 4]);
                // mov rsi,{len}
                self.emit(&[0x48, 0xc7, 0xc6]);
                self.emit(&(s.value.len() as u32).to_le_bytes());
                self.call(BUILTIN_ALLOC_OBJ);
                if s.value.len() != 0 {
                    // lea rdi,[rax+24]
                    self.emit(&[0x48, 0x8D, 0x78, 0x18]);
                    // lea rsi,[rip+{STR}]
                    self.emit(&[0x48, 0x8d, 0x35]);
                    self.strings.push((self.pos(), s.value.clone()));
                    self.emit(&[0; 4]);
                    // mov rcx,{len}
                    self.emit(&[0x48, 0xc7, 0xc1]);
                    self.emit(&(s.value.len() as u32).to_le_bytes());
                    // mov dl,[rsi]
                    self.emit(&[0x8A, 0x16]);
                    // mov [rdi],dl
                    self.emit(&[0x88, 0x17]);
                    // inc rsi
                    self.emit(&[0x48, 0xFF, 0xC6]);
                    // inc rdi
                    self.emit(&[0x48, 0xFF, 0xC7]);
                    // loop
                    self.emit(&[0xE2, 0xF4]);
                }
            }
            ExprContent::UnaryExpr(u) => {
                self.emit_expression(&u.operand);
                match u.operator {
                    UnaryOp::Negative => {
                        // neg rax
                        self.emit(&[0x48, 0xF7, 0xD8]);
                    }
                    UnaryOp::Not => {
                        // test rax,rax
                        self.emit(&[0x48, 0x85, 0xC0]);
                        // sete al
                        self.emit(&[0x0F, 0x94, 0xC0]);
                    }
                }
            }
            ExprContent::BinaryExpr(b) => {
                self.emit_expression(&b.left);
                if b.operator == BinaryOp::Or || b.operator == BinaryOp::And {
                    // test rax,rax
                    self.emit(&[0x48, 0x85, 0xC0]);
                    if b.operator == BinaryOp::Or {
                        // jne
                        self.emit(&[0x0f, 0x85]);
                    } else {
                        // je
                        self.emit(&[0x0f, 0x84]);
                    }
                    let pos = self.pos();
                    self.emit(&[0; 4]);
                    self.emit_expression(&b.right);
                    let delta = (self.pos() - pos - 4) as u32;
                    self.code[pos..pos + 4].copy_from_slice(&delta.to_le_bytes());
                } else {
                    self.emit_push_rax();
                    self.emit_expression(&b.right);
                    self.emit_pop_r11();

                    match b.operator {
                        BinaryOp::Add => {
                            if b.left.inferred_type.as_ref().unwrap() == &*TYPE_INT {
                                // Note: swapped
                                // add rax,r11
                                self.emit(&[0x4C, 0x01, 0xD8]);
                            } else if b.left.inferred_type.as_ref().unwrap() == &*TYPE_STR {
                            } else {
                            }
                        }
                        BinaryOp::Sub => {
                            // sub r11,rax
                            // mov rax,r11
                            self.emit(&[0x49, 0x29, 0xC3, 0x4C, 0x89, 0xD8]);
                        }
                        BinaryOp::Mul => {
                            // imul rax,r11
                            self.emit(&[0x49, 0x0F, 0xAF, 0xC3]);
                        }
                        BinaryOp::Div => {
                            // xchg rax,r11
                            self.emit(&[0x49, 0x93]);
                            // cqo
                            self.emit(&[0x48, 0x99]);
                            // idiv,r11
                            self.emit(&[0x49, 0xf7, 0xfb]);
                        }
                        BinaryOp::Mod => {
                            // xchg rax,r11
                            self.emit(&[0x49, 0x93]);
                            // cqo
                            self.emit(&[0x48, 0x99]);
                            // idiv,r11
                            self.emit(&[0x49, 0xf7, 0xfb]);
                            // mov rax,rdx
                            self.emit(&[0x48, 0x89, 0xD0]);
                        }
                        BinaryOp::Is => {
                            // cmp r11,rax
                            self.emit(&[0x49, 0x39, 0xC3]);
                            // sete r10b
                            self.emit(&[0x41, 0x0F, 0x94, 0xC2]);
                            // movzx r10,r10b
                            self.emit(&[0x4D, 0x0F, 0xB6, 0xD2]);
                            self.emit_push_r10();
                            self.emit_push_r11();
                            self.emit_drop();
                            self.emit_pop_rax();
                            self.emit_drop();
                            self.emit_pop_rax();
                        }
                        BinaryOp::Ne | BinaryOp::Eq => {
                            if b.left.inferred_type.as_ref().unwrap() == &*TYPE_STR {
                            } else {
                                let code = match b.operator {
                                    BinaryOp::Ne => 5,
                                    BinaryOp::Eq => 4,
                                    _ => panic!(),
                                };
                                // cmp r11,rax
                                self.emit(&[0x49, 0x39, 0xC3]);
                                // set* al
                                self.emit(&[0x0f, 0x90 + code, 0xc0]);
                                // movzx rax,al
                                self.emit(&[0x48, 0x0f, 0xb6, 0xc0]);
                            }
                        }
                        BinaryOp::Lt | BinaryOp::Ge | BinaryOp::Le | BinaryOp::Gt => {
                            let code = match b.operator {
                                BinaryOp::Lt => 0xc,
                                BinaryOp::Ge => 0xd,
                                BinaryOp::Le => 0xe,
                                BinaryOp::Gt => 0xf,
                                _ => panic!(),
                            };
                            // cmp r11,rax
                            self.emit(&[0x49, 0x39, 0xC3]);
                            // set* al
                            self.emit(&[0x0f, 0x90 + code, 0xc0]);
                            // movzx rax,al
                            self.emit(&[0x48, 0x0f, 0xb6, 0xc0]);
                        }
                        _ => (),
                    }
                }
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
                    if param_type == &*TYPE_OBJECT {
                        if arg.inferred_type.as_ref().unwrap() == &*TYPE_INT {
                            self.emit_box_int();
                        } else if arg.inferred_type.as_ref().unwrap() == &*TYPE_BOOL {
                            self.emit_box_bool();
                        }
                    }

                    match i {
                        // mov rdi,rax
                        0 => self.emit(&[0x48, 0x89, 0xc7]),
                        // mov rsi,rax
                        1 => self.emit(&[0x48, 0x89, 0xc6]),
                        // mov rdx,rax
                        2 => self.emit(&[0x48, 0x89, 0xc2]),
                        // mov rcx,rax
                        3 => self.emit(&[0x48, 0x89, 0xc1]),
                        // mov r8,rax
                        4 => self.emit(&[0x49, 0x89, 0xc0]),
                        // mov r9,rax
                        5 => self.emit(&[0x49, 0x89, 0xc1]),
                        _ => {
                            let offset = (i - 6) * 8;
                            assert!(offset < 128);
                            // mov QWORD PTR [rsp+{offset}],rax
                            self.emit(&[0x48, 0x89, 0x44, 0x24, offset as u8]);
                        }
                    }
                }

                self.call(&c.function.id().name);
            }
            _ => (),
        }
    }

    pub fn emit_statement(&mut self, statement: &Stmt) {
        match statement {
            Stmt::ExprStmt(e) => {
                self.emit_expression(&e.expr);
                if e.expr.inferred_type.as_ref().unwrap() != &*TYPE_INT
                    && e.expr.inferred_type.as_ref().unwrap() != &*TYPE_BOOL
                {
                    self.emit_drop();
                }
            }
            _ => (),
        }
    }
}

fn gen_code_set(ast: Ast) -> CodeSet {
    let mut main_code = Emitter::new(BUILTIN_CHOCOPY_MAIN);

    // mov rax,0x12345678
    main_code.emit(&[0x48, 0xC7, 0xC0, 0x78, 0x56, 0x34, 0x12]);
    main_code.emit_push_rax();

    for statement in &ast.program().statements {
        main_code.emit_statement(statement);
    }

    main_code.emit_pop_rax();
    // cmp rax,0x12345678
    main_code.emit(&[0x48, 0x3D, 0x78, 0x56, 0x34, 0x12]);

    // je
    main_code.emit(&[0x0f, 0x84]);
    let pos = main_code.pos();
    main_code.emit(&[0; 4]);

    main_code.prepare_call(0);
    main_code.call(BUILTIN_REPORT_BROKEN_STACK);

    let delta = (main_code.pos() - pos - 4) as u32;
    main_code.code[pos..pos + 4].copy_from_slice(&delta.to_le_bytes());

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
            (BOOL_PROTOTYPE, Decl::data_import().into()),
            (INT_PROTOTYPE, Decl::data_import().into()),
            (STR_PROTOTYPE, Decl::data_import().into()),
            (OBJECT_PROTOTYPE, Decl::data_import().into()),
            (BOOL_LIST_PROTOTYPE, Decl::data_import().into()),
            (INT_LIST_PROTOTYPE, Decl::data_import().into()),
            (OBJECT_LIST_PROTOTYPE, Decl::data_import().into()),
            // hidden built-in functions
            (BUILTIN_ALLOC_OBJ, Decl::function_import().into()),
            (BUILTIN_FREE_OBJ, Decl::function_import().into()),
            (BUILTIN_REPORT_BROKEN_STACK, Decl::function_import().into()),
            // built-in functions
            ("len", Decl::function_import().into()),
            ("print", Decl::function_import().into()),
            ("input", Decl::function_import().into()),
            // main
            (BUILTIN_CHOCOPY_MAIN, Decl::function().global().into()),
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
