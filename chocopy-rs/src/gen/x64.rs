use super::*;

struct FuncSlot {
    link_name: String,
    level: u32, // 0 = global function
}

struct VarSlot {
    offset: i32, // relative to global seciton or rbp
    level: u32,  // 0 = global variable
}

type StorageEnv = LocalEnv<FuncSlot, VarSlot>;

#[derive(Clone)]
struct AttributeSlot {
    offset: u32,
    source_type: ValueType,
    target_type: ValueType,
    init: LiteralContent,
}

#[derive(Clone)]
struct MethodSlot {
    offset: u32,
    link_name: String,
}

#[derive(Clone)]
struct ClassSlot {
    attributes: HashMap<String, AttributeSlot>,
    object_size: u32,
    methods: HashMap<String, MethodSlot>,
    prototype_size: u32,
}

struct Emitter<'a> {
    name: String,
    storage_env: &'a mut StorageEnv,
    classes: &'a HashMap<String, ClassSlot>,
    clean_up_list: &'a [i32], // offsets relative to rbp
    level: u32,
    code: Vec<u8>,
    links: Vec<ChunkLink>,
    rsp_aligned: bool,
    rsp_call_restore: Vec<(usize, bool)>,
    strings: Vec<(usize, String)>,
}

impl<'a> Emitter<'a> {
    pub fn new(
        name: &str,
        storage_env: &'a mut StorageEnv,
        classes: &'a HashMap<String, ClassSlot>,
        clean_up_list: &'a [i32],
        level: u32,
    ) -> Emitter<'a> {
        Emitter {
            name: name.to_owned(),
            storage_env,
            classes,
            clean_up_list,
            level,
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
        if !self.clean_up_list.is_empty() {
            self.emit_push_rax();
            for &offset in self.clean_up_list {
                // mov rax,[rbp+{}]
                self.emit(&[0x48, 0x8B, 0x85]);
                self.emit(&offset.to_le_bytes());
                self.emit_drop();
            }
            self.emit_pop_rax();
        }
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

    pub fn after_call(&mut self) {
        let (spill, rsp_aligned) = self.rsp_call_restore.pop().unwrap();
        assert!(spill < 128);
        // add rsp,{spill}
        self.emit(&[0x48, 0x83, 0xC4, spill as u8]);
        self.rsp_aligned = rsp_aligned;
    }

    pub fn call(&mut self, name: &str) {
        self.emit(&[0xe8]);
        self.links.push(ChunkLink {
            pos: self.pos(),
            to: name.to_owned(),
        });
        self.emit(&[0x00, 0x00, 0x00, 0x00]);
        self.after_call();
    }

    pub fn call_virtual(&mut self, offset: u32) {
        // mov rax,[rdi]
        self.emit(&[0x48, 0x8B, 0x07]);
        // call [rax+{}]
        self.emit(&[0xFF, 0x90]);
        self.emit(&offset.to_le_bytes());
        self.after_call();
    }

    pub fn finalize(mut self, procedure_debug: ProcedureDebug) -> Chunk {
        for (pos, s) in &self.strings {
            let dest = self.pos();
            self.code.extend_from_slice(s.as_bytes());
            let delta = (dest - *pos - 4) as u32;
            self.code[*pos..*pos + 4].copy_from_slice(&delta.to_le_bytes());
        }

        Chunk {
            name: self.name,
            code: self.code,
            links: self.links,
            extra: ChunkExtra::Procedure(procedure_debug),
        }
    }

    pub fn emit_push_rax(&mut self) {
        self.emit(&[0x50]);
        self.rsp_aligned = !self.rsp_aligned;
    }

    pub fn emit_push_rcx(&mut self) {
        self.emit(&[0x51]);
        self.rsp_aligned = !self.rsp_aligned;
    }

    pub fn emit_push_rdx(&mut self) {
        self.emit(&[0x52]);
        self.rsp_aligned = !self.rsp_aligned;
    }

    pub fn emit_push_rsi(&mut self) {
        self.emit(&[0x56]);
        self.rsp_aligned = !self.rsp_aligned;
    }

    pub fn emit_push_rdi(&mut self) {
        self.emit(&[0x57]);
        self.rsp_aligned = !self.rsp_aligned;
    }

    pub fn emit_push_r8(&mut self) {
        self.emit(&[0x41, 0x50]);
        self.rsp_aligned = !self.rsp_aligned;
    }

    pub fn emit_push_r9(&mut self) {
        self.emit(&[0x41, 0x51]);
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

    pub fn emit_pop_rcx(&mut self) {
        self.emit(&[0x59]);
        self.rsp_aligned = !self.rsp_aligned;
    }

    pub fn emit_pop_rdx(&mut self) {
        self.emit(&[0x5a]);
        self.rsp_aligned = !self.rsp_aligned;
    }

    pub fn emit_pop_rsi(&mut self) {
        self.emit(&[0x5e]);
        self.rsp_aligned = !self.rsp_aligned;
    }

    pub fn emit_pop_rdi(&mut self) {
        self.emit(&[0x5f]);
        self.rsp_aligned = !self.rsp_aligned;
    }

    pub fn emit_pop_r8(&mut self) {
        self.emit(&[0x41, 0x58]);
        self.rsp_aligned = !self.rsp_aligned;
    }

    pub fn emit_pop_r9(&mut self) {
        self.emit(&[0x41, 0x59]);
        self.rsp_aligned = !self.rsp_aligned;
    }

    pub fn emit_pop_r10(&mut self) {
        self.emit(&[0x41, 0x5A]);
        self.rsp_aligned = !self.rsp_aligned;
    }

    pub fn emit_pop_r11(&mut self) {
        self.emit(&[0x41, 0x5B]);
        self.rsp_aligned = !self.rsp_aligned;
    }

    pub fn emit_check_none(&mut self) {
        // test rax,rax
        self.emit(&[0x48, 0x85, 0xC0]);
        // jne
        self.emit(&[0x0F, 0x85]);
        let pos = self.pos();
        self.emit(&[0; 4]);
        self.prepare_call(0);
        self.call(BUILTIN_NONE_OP);
        let delta = (self.pos() - pos - 4) as u32;
        self.code[pos..pos + 4].copy_from_slice(&delta.to_le_bytes());
    }

    pub fn emit_box_int(&mut self) {
        self.emit_push_rax();
        self.prepare_call(2);
        // mov rdi,[rip+{INT_PROTOTYPE}]
        self.emit(&[0x48, 0x8B, 0x3D]);
        self.links.push(ChunkLink {
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
        self.links.push(ChunkLink {
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

    pub fn emit_clone(&mut self) {
        // test rax,rax
        self.emit(&[0x48, 0x85, 0xC0]);
        // je
        self.emit(&[0x74, 0x04]);
        // incq [rax+8]
        self.emit(&[0x48, 0xFF, 0x40, 0x08]);
    }

    pub fn emit_none_literal(&mut self) {
        // xor rax,rax
        self.emit(&[0x48, 0x31, 0xC0]);
    }

    pub fn emit_int_literal(&mut self, i: &IntegerLiteral) {
        // mov eax,{i}
        self.emit(&[0xB8]);
        self.emit(&i.value.to_le_bytes());
    }

    pub fn emit_bool_literal(&mut self, b: &BooleanLiteral) {
        // mov al,{}
        self.emit(&[0xB0, b.value as u8]);
    }

    pub fn emit_string_literal(&mut self, s: &StringLiteral) {
        self.prepare_call(2);
        // mov rdi,[rip+{STR_PROTOTYPE}]
        self.emit(&[0x48, 0x8B, 0x3D]);
        self.links.push(ChunkLink {
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

    pub fn emit_string_add(&mut self, expr: &BinaryExpr) {
        self.emit_expression(&expr.left);
        // mov rsi,QWORD PTR [rax+0x10]
        self.emit(&[0x48, 0x8B, 0x70, 0x10]);
        self.emit_push_rax();
        self.emit_push_rsi();
        self.emit_expression(&expr.right);
        self.emit_pop_rsi();
        // add rsi,QWORD PTR [rax+0x10]
        self.emit(&[0x48, 0x03, 0x70, 0x10]);
        self.emit_push_rax();

        self.prepare_call(2);
        // mov rdi,[rip+{STR_PROTOTYPE}]
        self.emit(&[0x48, 0x8B, 0x3D]);
        self.links.push(ChunkLink {
            pos: self.pos(),
            to: STR_PROTOTYPE.to_owned(),
        });
        self.emit(&[0; 4]);
        self.call(BUILTIN_ALLOC_OBJ);

        self.emit_pop_r11(); // right
        self.emit_pop_r10(); // left

        /*
        lea rdi,[rax+24]
        mov rcx,[r10+16]
        test rcx,rcx
        je skip1
        lea rsi,[r10+24]
        loop1:
        mov dl,[rsi]
        mov [rdi],dl
        inc rsi
        inc rdi
        loop loop1
        skip1:
        mov rcx,[r11+16]
        test rcx,rcx
        je skip2
        lea rsi,[r11+24]
        loop2:
        mov dl,[rsi]
        mov [rdi],dl
        inc rsi
        inc rdi
        loop loop2
        skip2:
        */
        self.emit(&[
            0x48, 0x8D, 0x78, 0x18, 0x49, 0x8B, 0x4A, 0x10, 0x48, 0x85, 0xC9, 0x74, 0x10, 0x49,
            0x8D, 0x72, 0x18, 0x8A, 0x16, 0x88, 0x17, 0x48, 0xFF, 0xC6, 0x48, 0xFF, 0xC7, 0xE2,
            0xF4, 0x49, 0x8B, 0x4B, 0x10, 0x48, 0x85, 0xC9, 0x74, 0x10, 0x49, 0x8D, 0x73, 0x18,
            0x8A, 0x16, 0x88, 0x17, 0x48, 0xFF, 0xC6, 0x48, 0xFF, 0xC7, 0xE2, 0xF4,
        ]);
        self.emit_push_rax();
        self.emit_push_r11();
        // mov rax,r10
        self.emit(&[0x4C, 0x89, 0xD0]);
        self.emit_drop();
        self.emit_pop_rax();
        self.emit_drop();
        self.emit_pop_rax();
    }

    pub fn emit_list_add_half(&mut self, source_element: &ValueType, target_element: &ValueType) {
        // rax: destintion buffer
        // rsi: source list object

        // mov rcx,[rsi+16]
        self.emit(&[0x48, 0x8B, 0x4E, 0x10]);
        // test rcx,rcx
        self.emit(&[0x48, 0x85, 0xC9]);
        // je skip
        self.emit(&[0x0F, 0x84]);
        let pos_skip = self.pos();
        self.emit(&[0; 4]);
        // add rsi,24
        self.emit(&[0x48, 0x83, 0xC6, 0x18]);
        let pos_loop = self.pos();

        self.emit_push_rax();

        if source_element == &*TYPE_INT {
            // mov eax,[rsi]
            self.emit(&[0x8B, 0x06]);
            // add rsi,4
            self.emit(&[0x48, 0x83, 0xC6, 0x04]);
        } else if source_element == &*TYPE_BOOL {
            // mov al,[rsi]
            self.emit(&[0x8A, 0x06]);
            // add rsi,1
            self.emit(&[0x48, 0x83, 0xC6, 0x01]);
        } else {
            // mov rax,[rsi]
            self.emit(&[0x48, 0x8B, 0x06]);
            self.emit_clone();
            // add rsi,8
            self.emit(&[0x48, 0x83, 0xC6, 0x08]);
        }

        self.emit_push_rsi();
        self.emit_push_rcx();
        self.emit_coerce(source_element, target_element);
        // mov r11,rax
        self.emit(&[0x49, 0x89, 0xC3]);
        self.emit_pop_rcx();
        self.emit_pop_rsi();
        self.emit_pop_rax();

        if target_element == &*TYPE_INT {
            // mov [rax],r11d
            self.emit(&[0x44, 0x89, 0x18]);
            // add rax,4
            self.emit(&[0x48, 0x83, 0xC0, 0x04]);
        } else if target_element == &*TYPE_BOOL {
            // mov [rax],r11b
            self.emit(&[0x44, 0x88, 0x18]);
            // add rax,1
            self.emit(&[0x48, 0x83, 0xC0, 0x01]);
        } else {
            // mov [rax],r11
            self.emit(&[0x4C, 0x89, 0x18]);
            // add rax,8
            self.emit(&[0x48, 0x83, 0xC0, 0x08]);
        }

        // dec rcx
        self.emit(&[0x48, 0xFF, 0xC9]);
        // jne
        self.emit(&[0x0F, 0x85]);
        let loop_delta = -((self.pos() - pos_loop + 4) as i32);
        self.emit(&loop_delta.to_le_bytes());

        let skip_delta = (self.pos() - pos_skip - 4) as u32;
        self.code[pos_skip..pos_skip + 4].copy_from_slice(&skip_delta.to_le_bytes());
    }

    pub fn emit_list_add(&mut self, expr: &BinaryExpr, target_element: &ValueType) {
        let prototype = if target_element == &*TYPE_INT {
            INT_LIST_PROTOTYPE
        } else if target_element == &*TYPE_BOOL {
            BOOL_LIST_PROTOTYPE
        } else {
            OBJECT_LIST_PROTOTYPE
        };

        self.emit_expression(&expr.left);
        // mov rsi,QWORD PTR [rax+0x10]
        self.emit(&[0x48, 0x8B, 0x70, 0x10]);
        self.emit_push_rax();
        self.emit_push_rsi();
        self.emit_expression(&expr.right);
        self.emit_pop_rsi();
        // add rsi,QWORD PTR [rax+0x10]
        self.emit(&[0x48, 0x03, 0x70, 0x10]);
        self.emit_push_rax();

        self.prepare_call(2);
        // mov rdi,[rip+{_PROTOTYPE}]
        self.emit(&[0x48, 0x8B, 0x3D]);
        self.links.push(ChunkLink {
            pos: self.pos(),
            to: prototype.to_owned(),
        });
        self.emit(&[0; 4]);
        self.call(BUILTIN_ALLOC_OBJ);
        self.emit_push_rax();
        // add rax,24
        self.emit(&[0x48, 0x83, 0xC0, 0x18]);

        // mov rsi,[rsp+16]
        self.emit(&[0x48, 0x8B, 0x74, 0x24, 0x10]);
        let source_element = if let Some(ValueType::ListValueType(l)) = &expr.left.inferred_type {
            &*l.element_type
        } else {
            panic!()
        };
        self.emit_list_add_half(source_element, target_element);

        // mov rsi,[rsp+8]
        self.emit(&[0x48, 0x8B, 0x74, 0x24, 0x08]);
        let source_element = if let Some(ValueType::ListValueType(l)) = &expr.right.inferred_type {
            &*l.element_type
        } else {
            panic!()
        };
        self.emit_list_add_half(source_element, target_element);

        // mov rax,[rsp+8]
        self.emit(&[0x48, 0x8B, 0x44, 0x24, 0x08]);
        self.emit_drop();
        // mov rax,[rsp+16]
        self.emit(&[0x48, 0x8B, 0x44, 0x24, 0x10]);
        self.emit_drop();
        self.emit_pop_rax();
        self.emit_pop_r11();
        self.emit_pop_r11();
    }

    pub fn emit_str_compare(&mut self, expr: &BinaryExpr) {
        self.emit_expression(&expr.left);
        self.emit_push_rax();
        self.emit_expression(&expr.right);
        self.emit_pop_r11();

        /*
        mov rcx,[rax+16]
        mov rdx,[r11+16]
        cmp rcx,rdx
        jne not_equal
        test rcx,rcx
        je equal
        lea rdi,[rax+24]
        lea rsi,[r11+24]
        lo:
        mov dl,[rdi]
        cmp dl,[rsi]
        jne not_equal
        inc rdi
        inc rsi
        loop lo
        equal:
        mov rdx,1
        jmp finish
        not_equal:
        xor rdx,rdx
        finish:
        */
        self.emit(&[
            0x48, 0x8B, 0x48, 0x10, 0x49, 0x8B, 0x53, 0x10, 0x48, 0x39, 0xD1, 0x75, 0x24, 0x48,
            0x85, 0xC9, 0x74, 0x16, 0x48, 0x8D, 0x78, 0x18, 0x49, 0x8D, 0x73, 0x18, 0x8A, 0x17,
            0x3A, 0x16, 0x75, 0x11, 0x48, 0xFF, 0xC7, 0x48, 0xFF, 0xC6, 0xE2, 0xF2, 0x48, 0xC7,
            0xC2, 0x01, 0x00, 0x00, 0x00, 0xEB, 0x03, 0x48, 0x31, 0xD2,
        ]);

        if expr.operator == BinaryOp::Ne {
            // test rdx,rdx
            self.emit(&[0x48, 0x85, 0xD2]);
            // sete dl
            self.emit(&[0x0F, 0x94, 0xC2]);
        }

        self.emit_push_rdx();
        self.emit_push_r11();
        self.emit_drop();
        self.emit_pop_rax();
        self.emit_drop();
        self.emit_pop_rax();
    }

    pub fn emit_binary_expr(&mut self, expr: &BinaryExpr, target_type: &ValueType) {
        let left_type = expr.left.inferred_type.as_ref().unwrap();
        if expr.operator == BinaryOp::Add && left_type == &*TYPE_STR {
            self.emit_string_add(expr);
        } else if expr.operator == BinaryOp::Add && left_type != &*TYPE_INT {
            let target_element = if let ValueType::ListValueType(l) = &target_type {
                &*l.element_type
            } else {
                panic!()
            };
            self.emit_list_add(expr, target_element);
        } else if (expr.operator == BinaryOp::Eq || expr.operator == BinaryOp::Ne)
            && left_type == &*TYPE_STR
        {
            self.emit_str_compare(expr);
        } else if expr.operator == BinaryOp::Or || expr.operator == BinaryOp::And {
            self.emit_expression(&expr.left);
            // test al,al
            self.emit(&[0x84, 0xC0]);
            if expr.operator == BinaryOp::Or {
                // jne
                self.emit(&[0x0f, 0x85]);
            } else {
                // je
                self.emit(&[0x0f, 0x84]);
            }
            let pos = self.pos();
            self.emit(&[0; 4]);
            self.emit_expression(&expr.right);
            let delta = (self.pos() - pos - 4) as u32;
            self.code[pos..pos + 4].copy_from_slice(&delta.to_le_bytes());
        } else {
            self.emit_expression(&expr.left);
            self.emit_push_rax();
            self.emit_expression(&expr.right);
            self.emit_pop_r11();

            match expr.operator {
                BinaryOp::Add => {
                    // Note: swapped
                    // add eax,r11d
                    self.emit(&[0x44, 0x01, 0xD8]);
                }
                BinaryOp::Sub => {
                    // sub r11d,eax
                    // mov eax,r11d
                    self.emit(&[0x41, 0x29, 0xC3, 0x44, 0x89, 0xD8]);
                }
                BinaryOp::Mul => {
                    // imul eax,r11d
                    self.emit(&[0x41, 0x0F, 0xAF, 0xC3]);
                }
                BinaryOp::Div | BinaryOp::Mod => {
                    // test eax,eax
                    self.emit(&[0x85, 0xC0]);
                    // jne
                    self.emit(&[0x0F, 0x85]);
                    let pos = self.pos();
                    self.emit(&[0; 4]);
                    self.prepare_call(0);
                    self.call(BUILTIN_DIV_ZERO);
                    let delta = (self.pos() - pos - 4) as u32;
                    self.code[pos..pos + 4].copy_from_slice(&delta.to_le_bytes());

                    // xchg eax,r11d
                    self.emit(&[0x41, 0x93]);
                    // cdq
                    self.emit(&[0x99]);
                    // idiv,r11d
                    self.emit(&[0x41, 0xF7, 0xFB]);
                    if expr.operator == BinaryOp::Mod {
                        // mov eax,edx
                        self.emit(&[0x89, 0xD0]);
                    }
                }
                BinaryOp::Is => {
                    // cmp r11,rax
                    self.emit(&[0x49, 0x39, 0xC3]);
                    // sete r10b
                    self.emit(&[0x41, 0x0F, 0x94, 0xC2]);
                    self.emit_push_r10();
                    self.emit_push_r11();
                    self.emit_drop();
                    self.emit_pop_rax();
                    self.emit_drop();
                    self.emit_pop_rax();
                }
                BinaryOp::Ne
                | BinaryOp::Eq
                | BinaryOp::Lt
                | BinaryOp::Ge
                | BinaryOp::Le
                | BinaryOp::Gt => {
                    let code = match expr.operator {
                        BinaryOp::Eq => 0x4,
                        BinaryOp::Ne => 0x5,
                        BinaryOp::Lt => 0xc,
                        BinaryOp::Ge => 0xd,
                        BinaryOp::Le => 0xe,
                        BinaryOp::Gt => 0xf,
                        _ => panic!(),
                    };

                    if left_type == &*TYPE_BOOL {
                        // cmp r11b,al
                        self.emit(&[0x41, 0x38, 0xC3]);
                    } else {
                        // cmp r11d,eax
                        self.emit(&[0x41, 0x39, 0xC3]);
                    }
                    // set* al
                    self.emit(&[0x0f, 0x90 + code, 0xc0]);
                }
                _ => panic!(),
            }
        }
    }
    pub fn emit_coerce(&mut self, from: &ValueType, to: &ValueType) {
        if to == &*TYPE_OBJECT {
            if from == &*TYPE_INT {
                self.emit_box_int();
            } else if from == &*TYPE_BOOL {
                self.emit_box_bool();
            }
        }
    }

    pub fn emit_call_expr(
        &mut self,
        args: &[Expr],
        func_type: &Option<FuncTypeWrapper>,
        name: &str,
        virtual_call: bool,
    ) {
        self.prepare_call(args.len());

        for (i, arg) in args.iter().enumerate() {
            self.emit_expression(arg);

            let param_type = &func_type.as_ref().unwrap().func_type().parameters[i];

            self.emit_coerce(arg.inferred_type.as_ref().unwrap(), param_type);

            if i == 0 && virtual_call {
                self.emit_check_none();
            }

            if i < 6 {
                self.emit_push_rax();
            } else {
                // note that at these point, 6 params has been pushed on to the
                // stack in reversed order. We want to fill in more params after these 6 params
                // in the ABI required order.
                let offset = i * 8;
                assert!(offset < 128);
                // mov QWORD PTR [rsp+{offset}],rax
                self.emit(&[0x48, 0x89, 0x44, 0x24, offset as u8]);
            }
        }

        if args.len() >= 6 {
            self.emit_pop_r9();
        }

        if args.len() >= 5 {
            self.emit_pop_r8();
        }

        if args.len() >= 4 {
            self.emit_pop_rcx();
        }

        if args.len() >= 3 {
            self.emit_pop_rdx();
        }

        if args.len() >= 2 {
            self.emit_pop_rsi();
        }

        if args.len() >= 1 {
            self.emit_pop_rdi();
        }

        if virtual_call {
            let offset = if let Some(ValueType::ClassValueType(c)) = &args[0].inferred_type {
                if let "int" | "bool" | "str" | "<None>" | "<Empty>" = c.class_name.as_str() {
                    assert!(name == "__init__");
                    16
                } else {
                    self.classes[&c.class_name].methods[name].offset
                }
            } else {
                panic!()
            };
            self.call_virtual(offset);
        } else {
            let slot = if let Some(EnvSlot::Func(f)) = self.storage_env.get(name) {
                f
            } else {
                panic!()
            };

            let link_name = slot.link_name.clone();
            let call_level = slot.level;

            if call_level != 0 {
                // mov r10,rbp
                self.emit(&[0x49, 0x89, 0xEA]);
                for _ in 0..self.level + 1 - call_level {
                    // mov r10,[r10-8]
                    self.emit(&[0x4D, 0x8B, 0x52, 0xF8]);
                }
            }

            self.call(&link_name);
        }
    }

    pub fn emit_str_index(&mut self, expr: &IndexExpr) {
        self.emit_expression(&expr.list);
        self.emit_push_rax();
        self.emit_expression(&expr.index);
        // cdqe
        self.emit(&[0x48, 0x98]);
        self.emit_push_rax();
        self.prepare_call(2);
        // mov rdi,[rip+{STR_PROTOTYPE}]
        self.emit(&[0x48, 0x8B, 0x3D]);
        self.links.push(ChunkLink {
            pos: self.pos(),
            to: STR_PROTOTYPE.to_owned(),
        });
        self.emit(&[0; 4]);
        // mov rsi,1
        self.emit(&[0x48, 0xc7, 0xc6, 0x01, 0x00, 0x00, 0x00]);
        self.call(BUILTIN_ALLOC_OBJ);
        self.emit_pop_rsi();
        self.emit_pop_r11();
        // cmp rsi,[r11+16]
        self.emit(&[0x49, 0x3B, 0x73, 0x10]);
        // jb
        self.emit(&[0x0F, 0x82]);
        let pos = self.pos();
        self.emit(&[0; 4]);
        self.prepare_call(0);
        self.call(BUILTIN_OUT_OF_BOUND);
        let delta = (self.pos() - pos - 4) as u32;
        self.code[pos..pos + 4].copy_from_slice(&delta.to_le_bytes());
        // mov r10b,[r11+rsi+24]
        self.emit(&[0x45, 0x8A, 0x54, 0x33, 0x18]);
        // mov [rax+24],r10b
        self.emit(&[0x44, 0x88, 0x50, 0x18]);
        self.emit_push_rax();
        // mov rax,r11
        self.emit(&[0x4C, 0x89, 0xD8]);
        self.emit_drop();
        self.emit_pop_rax();
    }

    pub fn emit_list_index(&mut self, expr: &IndexExpr) {
        self.emit_expression(&expr.list);
        self.emit_check_none();
        self.emit_push_rax();
        self.emit_expression(&expr.index);
        // cdqe
        self.emit(&[0x48, 0x98]);
        self.emit_pop_rsi();
        let element_type = if let Some(ValueType::ListValueType(l)) = &expr.list.inferred_type {
            &*l.element_type
        } else {
            panic!()
        };

        // cmp rax,[rsi+16]
        self.emit(&[0x48, 0x3B, 0x46, 0x10]);
        // jb
        self.emit(&[0x0F, 0x82]);
        let pos = self.pos();
        self.emit(&[0; 4]);
        self.prepare_call(0);
        self.call(BUILTIN_OUT_OF_BOUND);
        let delta = (self.pos() - pos - 4) as u32;
        self.code[pos..pos + 4].copy_from_slice(&delta.to_le_bytes());

        if element_type == &*TYPE_INT {
            // mov eax,[rsi+rax*4+24]
            self.emit(&[0x8B, 0x44, 0x86, 0x18]);
        } else if element_type == &*TYPE_BOOL {
            // mov al,[rsi+rax+24]
            self.emit(&[0x8A, 0x44, 0x06, 0x18]);
        } else {
            // mov rax,[rsi+rax*8+24]
            self.emit(&[0x48, 0x8B, 0x44, 0xC6, 0x18]);
            self.emit_clone();
        }

        self.emit_push_rax();
        // mov rax,rsi
        self.emit(&[0x48, 0x89, 0xF0]);
        self.emit_drop();
        self.emit_pop_rax();
    }

    pub fn emit_member_expr(&mut self, expr: &MemberExpr) {
        self.emit_expression(&expr.object);
        self.emit_check_none();
        // mov rsi,rax
        self.emit(&[0x48, 0x89, 0xC6]);

        let slot = if let Some(ValueType::ClassValueType(c)) = &expr.object.inferred_type {
            &self.classes[&c.class_name].attributes[&expr.member.id().name]
        } else {
            panic!()
        };

        if slot.target_type == *TYPE_INT {
            // mov eax,[rsi+{}]
            self.emit(&[0x8B, 0x86]);
            self.emit(&slot.offset.to_le_bytes());
        } else if slot.target_type == *TYPE_BOOL {
            // mov al,[rsi+{}]
            self.emit(&[0x8A, 0x86]);
            self.emit(&slot.offset.to_le_bytes());
        } else {
            // mov rax,[rsi+{}]
            self.emit(&[0x48, 0x8B, 0x86]);
            self.emit(&slot.offset.to_le_bytes());
            self.emit_clone();
        }

        self.emit_push_rax();
        // mov rax,rsi
        self.emit(&[0x48, 0x89, 0xF0]);
        self.emit_drop();
        self.emit_pop_rax();
    }

    pub fn emit_if_expr(&mut self, expr: &IfExpr, target_type: &ValueType) {
        self.emit_expression(&expr.condition);
        // test al,al
        self.emit(&[0x84, 0xC0]);
        // je
        self.emit(&[0x0f, 0x84]);
        let pos_if = self.pos();
        self.emit(&[0; 4]);

        self.emit_expression(&expr.then_expr);
        self.emit_coerce(&expr.then_expr.inferred_type.as_ref().unwrap(), target_type);

        // jmp
        self.emit(&[0xe9]);
        let pos_else = self.pos();
        self.emit(&[0; 4]);
        let if_delta = self.pos() - pos_if - 4;
        self.code[pos_if..pos_if + 4].copy_from_slice(&(if_delta as u32).to_le_bytes());

        self.emit_expression(&expr.else_expr);
        self.emit_coerce(&expr.else_expr.inferred_type.as_ref().unwrap(), target_type);

        let else_delta = self.pos() - pos_else - 4;
        self.code[pos_else..pos_else + 4].copy_from_slice(&(else_delta as u32).to_le_bytes());
    }

    pub fn emit_if_stmt(&mut self, stmt: &IfStmt, lines: &mut Vec<(usize, u32)>) {
        self.emit_expression(&stmt.condition);
        // test al,al
        self.emit(&[0x84, 0xC0]);
        // je
        self.emit(&[0x0f, 0x84]);
        let pos_if = self.pos();
        self.emit(&[0; 4]);

        for stmt in &stmt.then_body {
            self.emit_statement(stmt, lines);
        }

        // jmp
        self.emit(&[0xe9]);
        let pos_else = self.pos();
        self.emit(&[0; 4]);
        let if_delta = self.pos() - pos_if - 4;
        self.code[pos_if..pos_if + 4].copy_from_slice(&(if_delta as u32).to_le_bytes());

        for stmt in &stmt.else_body {
            self.emit_statement(stmt, lines);
        }

        let else_delta = self.pos() - pos_else - 4;
        self.code[pos_else..pos_else + 4].copy_from_slice(&(else_delta as u32).to_le_bytes());
    }

    pub fn emit_list_expr(&mut self, expr: &ListExpr, target_type: &ValueType) {
        if target_type == &*TYPE_EMPTY {
            self.prepare_call(2);
            // mov rdi,[rip+{_PROTOTYPE}]
            self.emit(&[0x48, 0x8B, 0x3D]);
            self.links.push(ChunkLink {
                pos: self.pos(),
                to: OBJECT_LIST_PROTOTYPE.to_owned(),
            });
            self.emit(&[0; 4]);
            // mov rsi,{len}
            self.emit(&[0x48, 0xc7, 0xc6]);
            self.emit(&(expr.elements.len() as u32).to_le_bytes());
            self.call(BUILTIN_ALLOC_OBJ);
            return;
        }

        let element_type = if let ValueType::ListValueType(l) = &target_type {
            &*l.element_type
        } else {
            panic!()
        };

        let prototype = if element_type == &*TYPE_INT {
            INT_LIST_PROTOTYPE
        } else if element_type == &*TYPE_BOOL {
            BOOL_LIST_PROTOTYPE
        } else {
            OBJECT_LIST_PROTOTYPE
        };

        self.prepare_call(2);
        // mov rdi,[rip+{_PROTOTYPE}]
        self.emit(&[0x48, 0x8B, 0x3D]);
        self.links.push(ChunkLink {
            pos: self.pos(),
            to: prototype.to_owned(),
        });
        self.emit(&[0; 4]);
        // mov rsi,{len}
        self.emit(&[0x48, 0xc7, 0xc6]);
        self.emit(&(expr.elements.len() as u32).to_le_bytes());
        self.call(BUILTIN_ALLOC_OBJ);
        self.emit_push_rax();

        for (i, element) in expr.elements.iter().enumerate() {
            self.emit_expression(element);
            self.emit_coerce(element.inferred_type.as_ref().unwrap(), element_type);
            // mov rdi,[rsp]
            self.emit(&[0x48, 0x8B, 0x3C, 0x24]);
            if element_type == &*TYPE_INT {
                // mov [rdi+{}],eax
                self.emit(&[0x89, 0x87]);
                self.emit(&((i * 4 + 24) as u32).to_le_bytes());
            } else if element_type == &*TYPE_BOOL {
                // mov [rdi+{}],al
                self.emit(&[0x88, 0x87]);
                self.emit(&((i + 24) as u32).to_le_bytes());
            } else {
                // mov [rdi+{}],rax
                self.emit(&[0x48, 0x89, 0x87]);
                self.emit(&((i * 8 + 24) as u32).to_le_bytes());
            }
        }

        self.emit_pop_rax();
    }

    pub fn emit_load_var(&mut self, identifier: &Identifier, target_type: &ValueType) {
        let (offset, level) =
            if let Some(EnvSlot::Var(v, _)) = self.storage_env.get(&identifier.name) {
                (v.offset, v.level)
            } else {
                panic!()
            };

        if level == 0 {
            if target_type == &*TYPE_INT {
                // mov eax,[rip+{}]
                self.emit(&[0x8B, 0x05]);
                self.links.push(ChunkLink {
                    pos: self.pos(),
                    to: GLOBAL_SECTION.to_owned(),
                });
                self.emit(&offset.to_le_bytes());
            } else if target_type == &*TYPE_BOOL {
                // mov al,[rip+{}]
                self.emit(&[0x8A, 0x05]);
                self.links.push(ChunkLink {
                    pos: self.pos(),
                    to: GLOBAL_SECTION.to_owned(),
                });
                self.emit(&offset.to_le_bytes());
            } else {
                // mov rax,[rip+{}]
                self.emit(&[0x48, 0x8B, 0x05]);
                self.links.push(ChunkLink {
                    pos: self.pos(),
                    to: GLOBAL_SECTION.to_owned(),
                });
                self.emit(&offset.to_le_bytes());
                self.emit_clone();
            }
        } else {
            if level == self.level + 1 {
                // mov rax,[rbp+{}]
                self.emit(&[0x48, 0x8B, 0x85]);
                self.emit(&offset.to_le_bytes());
            } else {
                // mov rax,[rbp-8]
                self.emit(&[0x48, 0x8B, 0x45, 0xF8]);
                for _ in 0..self.level - level {
                    // mov rax,[rax-8]
                    self.emit(&[0x48, 0x8B, 0x40, 0xF8]);
                }
                // mov rax,[rax+{}]
                self.emit(&[0x48, 0x8B, 0x80]);
                self.emit(&offset.to_le_bytes());
            }
            if target_type != &*TYPE_INT && target_type != &*TYPE_BOOL {
                self.emit_clone();
            }
        }
    }

    pub fn emit_expression(&mut self, expression: &Expr) {
        match &expression.content {
            ExprContent::Identifier(identifier) => {
                self.emit_load_var(identifier, expression.inferred_type.as_ref().unwrap());
            }
            ExprContent::NoneLiteral(_) => {
                self.emit_none_literal();
            }
            ExprContent::IntegerLiteral(i) => {
                self.emit_int_literal(i);
            }
            ExprContent::BooleanLiteral(b) => {
                self.emit_bool_literal(b);
            }
            ExprContent::StringLiteral(s) => {
                self.emit_string_literal(s);
            }
            ExprContent::UnaryExpr(expr) => {
                self.emit_expression(&expr.operand);
                match expr.operator {
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
            ExprContent::BinaryExpr(expr) => {
                self.emit_binary_expr(expr, expression.inferred_type.as_ref().unwrap());
            }
            ExprContent::CallExpr(expr) => {
                self.emit_call_expr(
                    &expr.args,
                    &expr.function.id().inferred_type,
                    &expr.function.id().name,
                    false,
                );
            }
            ExprContent::MethodCallExpr(expr) => {
                let method = expr.method.member();
                let args: Vec<Expr> = std::iter::once(method.object.clone())
                    .chain(expr.args.iter().cloned())
                    .collect();
                self.emit_call_expr(&args, &method.inferred_type, &method.member.id().name, true);
            }
            ExprContent::IndexExpr(expr) => {
                if expr.list.inferred_type.as_ref().unwrap() == &*TYPE_STR {
                    self.emit_str_index(&*expr);
                } else {
                    self.emit_list_index(&*expr);
                }
            }
            ExprContent::IfExpr(expr) => {
                self.emit_if_expr(expr, expression.inferred_type.as_ref().unwrap())
            }
            ExprContent::ListExpr(expr) => {
                self.emit_list_expr(expr, expression.inferred_type.as_ref().unwrap());
            }
            ExprContent::MemberExpr(expr) => {
                self.emit_member_expr(expr);
            }
        }
    }

    pub fn emit_while_stmt(&mut self, stmt: &WhileStmt, lines: &mut Vec<(usize, u32)>) {
        let pos_start = self.pos();
        self.emit_expression(&stmt.condition);
        // test al,al
        self.emit(&[0x84, 0xC0]);
        // je
        self.emit(&[0x0f, 0x84]);
        let pos_condition = self.pos();
        self.emit(&[0; 4]);

        for stmt in &stmt.body {
            self.emit_statement(stmt, lines);
        }

        // jmp
        self.emit(&[0xe9]);
        let back_delta = -((self.pos() + 4 - pos_start) as i32);
        self.emit(&back_delta.to_le_bytes());
        let if_delta = (self.pos() - pos_condition - 4) as u32;
        self.code[pos_condition..pos_condition + 4].copy_from_slice(&if_delta.to_le_bytes());
    }

    pub fn emit_assign_identifier(
        &mut self,
        name: &str,
        source_type: &ValueType,
        target_type: &ValueType,
    ) {
        // rax: value to assign

        let (offset, level) = if let Some(EnvSlot::Var(v, _)) = self.storage_env.get(name) {
            (v.offset, v.level)
        } else {
            panic!()
        };

        self.emit_coerce(source_type, target_type);
        if level == 0 {
            if target_type == &*TYPE_INT {
                // mov [rip+{}],eax
                self.emit(&[0x89, 0x05]);
            } else if target_type == &*TYPE_BOOL {
                // mov [rip+{}],al
                self.emit(&[0x88, 0x05]);
            } else {
                self.emit_push_rax();
                // mov rax,[rip+{}]
                self.emit(&[0x48, 0x8B, 0x05]);
                self.links.push(ChunkLink {
                    pos: self.pos(),
                    to: GLOBAL_SECTION.to_owned(),
                });
                self.emit(&offset.to_le_bytes());
                self.emit_drop();
                self.emit_pop_rax();
                // mov [rip+{}],rax
                self.emit(&[0x48, 0x89, 0x05]);
            }
            self.links.push(ChunkLink {
                pos: self.pos(),
                to: GLOBAL_SECTION.to_owned(),
            });
            self.emit(&offset.to_le_bytes());
        } else {
            if level == self.level + 1 {
                // lea rdi,[rbp+{}]
                self.emit(&[0x48, 0x8D, 0xBD]);
                self.emit(&offset.to_le_bytes());
            } else {
                // mov rdi,[rbp-8]
                self.emit(&[0x48, 0x8B, 0x7D, 0xF8]);
                for _ in 0..self.level - level {
                    // mov rdi,[rdi-8]
                    self.emit(&[0x48, 0x8B, 0x7F, 0xF8]);
                }
                // lea rdi,[rdi+{}]
                self.emit(&[0x48, 0x8D, 0xBF]);
                self.emit(&offset.to_le_bytes());
            }

            if target_type != &*TYPE_INT && target_type != &*TYPE_BOOL {
                self.emit_push_rdi();
                self.emit_push_rax();
                // mov rax,[rdi]
                self.emit(&[0x48, 0x8B, 0x07]);
                self.emit_drop();
                self.emit_pop_rax();
                self.emit_pop_rdi();
            }
            // mov [rdi],rax
            self.emit(&[0x48, 0x89, 0x07]);
        }
    }

    pub fn emit_assign(&mut self, stmt: &AssignStmt) {
        let source_type = stmt.value.inferred_type.as_ref().unwrap();
        self.emit_expression(&stmt.value);
        self.emit_push_rax();

        for target in &stmt.targets {
            let target_type = target.inferred_type.as_ref().unwrap();
            match &target.content {
                ExprContent::Identifier(identifier) => {
                    // mov rax,[rsp]
                    self.emit(&[0x48, 0x8B, 0x04, 0x24]);
                    if source_type != &*TYPE_INT && source_type != &*TYPE_BOOL {
                        self.emit_clone();
                    }

                    self.emit_assign_identifier(&identifier.name, source_type, target_type);
                }
                ExprContent::IndexExpr(expr) => {
                    self.emit_expression(&expr.list);
                    self.emit_check_none();
                    self.emit_push_rax();
                    self.emit_expression(&expr.index);
                    // mov rsi,[rsp]
                    self.emit(&[0x48, 0x8B, 0x34, 0x24]);

                    // cmp rax,[rsi+16]
                    self.emit(&[0x48, 0x3B, 0x46, 0x10]);
                    // jb
                    self.emit(&[0x0F, 0x82]);
                    let pos = self.pos();
                    self.emit(&[0; 4]);
                    self.prepare_call(0);
                    self.call(BUILTIN_OUT_OF_BOUND);
                    let delta = (self.pos() - pos - 4) as u32;
                    self.code[pos..pos + 4].copy_from_slice(&delta.to_le_bytes());

                    if target_type == &*TYPE_INT {
                        // lea rsi,[rsi+rax*4+24]
                        self.emit(&[0x48, 0x8D, 0x74, 0x86, 0x18]);
                        self.emit_push_rsi();
                    } else if target_type == &*TYPE_BOOL {
                        // lea rsi,[rsi+rax+24]
                        self.emit(&[0x48, 0x8D, 0x74, 0x06, 0x18]);
                        self.emit_push_rsi();
                    } else {
                        // lea rsi,[rsi+rax*8+24]
                        self.emit(&[0x48, 0x8D, 0x74, 0xC6, 0x18]);
                        // mov rax,[rsi]
                        self.emit(&[0x48, 0x8B, 0x06]);
                        self.emit_push_rsi();
                        self.emit_drop();
                    }

                    // mov rax,[rsp+16]
                    self.emit(&[0x48, 0x8B, 0x44, 0x24, 0x10]);
                    if source_type != &*TYPE_INT && source_type != &*TYPE_BOOL {
                        self.emit_clone();
                    }
                    self.emit_coerce(source_type, target_type);
                    self.emit_pop_rsi();

                    if target_type == &*TYPE_INT {
                        // mov [rsi],eax
                        self.emit(&[0x89, 0x06]);
                    } else if target_type == &*TYPE_BOOL {
                        // mov [rsi],al
                        self.emit(&[0x88, 0x06]);
                    } else {
                        // mov [rsi],rax
                        self.emit(&[0x48, 0x89, 0x06]);
                    }

                    self.emit_pop_rax();
                    self.emit_drop();
                }
                ExprContent::MemberExpr(expr) => {
                    self.emit_expression(&expr.object);
                    self.emit_check_none();
                    self.emit_push_rax();

                    let slot =
                        if let Some(ValueType::ClassValueType(c)) = &expr.object.inferred_type {
                            &self.classes[&c.class_name].attributes[&expr.member.id().name]
                        } else {
                            panic!()
                        };

                    if slot.target_type != *TYPE_INT && slot.target_type != *TYPE_BOOL {
                        // mov rax,[rax+{}]
                        self.emit(&[0x48, 0x8B, 0x80]);
                        self.emit(&slot.offset.to_le_bytes());
                        self.emit_drop();
                    }

                    // mov rax,[rsp+0x8]
                    self.emit(&[0x48, 0x8B, 0x44, 0x24, 0x08]);
                    if source_type != &*TYPE_INT && source_type != &*TYPE_BOOL {
                        self.emit_clone();
                    }
                    self.emit_coerce(source_type, &slot.target_type);

                    // mov rsi,[rsp]
                    self.emit(&[0x48, 0x8B, 0x34, 0x24]);
                    if slot.target_type == *TYPE_INT {
                        // mov [rsi+{}],eax
                        self.emit(&[0x89, 0x86]);
                    } else if slot.target_type == *TYPE_BOOL {
                        // mov [rsi+{}],al
                        self.emit(&[0x88, 0x86]);
                    } else {
                        // mov [rsi+{}],rax
                        self.emit(&[0x48, 0x89, 0x86]);
                    }
                    self.emit(&slot.offset.to_le_bytes());

                    self.emit_pop_rax();
                    self.emit_drop();
                }
                _ => panic!(),
            }
        }

        self.emit_pop_rax();
        if source_type != &*TYPE_INT && source_type != &*TYPE_BOOL {
            self.emit_drop();
        }
    }

    pub fn emit_for_stmt(&mut self, stmt: &ForStmt, lines: &mut Vec<(usize, u32)>) {
        //// Compute the iterable
        self.emit_expression(&stmt.iterable);
        self.emit_check_none();
        self.emit_push_rax();
        // xor rax,rax
        self.emit(&[0x48, 0x31, 0xC0]);

        let pos_start = self.pos();
        //// Check the index range
        // mov rsi,[rsp]
        self.emit(&[0x48, 0x8B, 0x34, 0x24]);
        // cmp rax,[rsi+16]
        self.emit(&[0x48, 0x3B, 0x46, 0x10]);
        // je
        self.emit(&[0x0f, 0x84]);
        let pos_condition = self.pos();
        self.emit(&[0; 4]);

        self.emit_push_rax();

        //// Compute the element
        let iterable_type = stmt.iterable.inferred_type.as_ref().unwrap();
        let source_type = if iterable_type == &*TYPE_STR {
            self.prepare_call(2);
            // mov rdi,[rip+{STR_PROTOTYPE}]
            self.emit(&[0x48, 0x8B, 0x3D]);
            self.links.push(ChunkLink {
                pos: self.pos(),
                to: STR_PROTOTYPE.to_owned(),
            });
            self.emit(&[0; 4]);
            // mov rsi,1
            self.emit(&[0x48, 0xc7, 0xc6, 0x01, 0x00, 0x00, 0x00]);
            self.call(BUILTIN_ALLOC_OBJ);
            // mov rsi,[rsp]
            self.emit(&[0x48, 0x8B, 0x34, 0x24]);
            // mov r11,[rsp+8]
            self.emit(&[0x4C, 0x8B, 0x5C, 0x24, 0x08]);
            // mov r10b,[r11+rsi+24]
            self.emit(&[0x45, 0x8A, 0x54, 0x33, 0x18]);
            // mov [rax+24],r10b
            self.emit(&[0x44, 0x88, 0x50, 0x18]);
            &*TYPE_STR
        } else {
            let element_type = if let ValueType::ListValueType(l) = iterable_type {
                &*l.element_type
            } else {
                panic!()
            };

            if element_type == &*TYPE_INT {
                // mov eax,[rsi+rax*4+24]
                self.emit(&[0x8B, 0x44, 0x86, 0x18]);
            } else if element_type == &*TYPE_BOOL {
                // mov al,[rsi+rax+24]
                self.emit(&[0x8A, 0x44, 0x06, 0x18]);
            } else {
                // mov rax,[rsi+rax*8+24]
                self.emit(&[0x48, 0x8B, 0x44, 0xC6, 0x18]);
                self.emit_clone();
            }

            element_type
        };

        //// Assign the element
        let target_type = stmt.identifier.id().inferred_type.as_ref().unwrap();
        self.emit_assign_identifier(&stmt.identifier.id().name, source_type, target_type);

        //// Execute the loop body
        for stmt in &stmt.body {
            self.emit_statement(stmt, lines);
        }

        //// Increase the index and loop back
        self.emit_pop_rax();
        // inc rax
        self.emit(&[0x48, 0xFF, 0xC0]);
        // jmp
        self.emit(&[0xe9]);
        let back_delta = -((self.pos() + 4 - pos_start) as i32);
        self.emit(&back_delta.to_le_bytes());
        let if_delta = (self.pos() - pos_condition - 4) as u32;
        self.code[pos_condition..pos_condition + 4].copy_from_slice(&if_delta.to_le_bytes());

        //// Drop the iterable
        self.emit_pop_rax();
        self.emit_drop();
    }

    pub fn emit_statement(&mut self, statement: &Stmt, lines: &mut Vec<(usize, u32)>) {
        match statement {
            Stmt::ExprStmt(e) => {
                lines.push((self.pos(), e.base().location.start.row));
                self.emit_expression(&e.expr);
                if e.expr.inferred_type.as_ref().unwrap() != &*TYPE_INT
                    && e.expr.inferred_type.as_ref().unwrap() != &*TYPE_BOOL
                {
                    self.emit_drop();
                }
            }
            Stmt::AssignStmt(stmt) => {
                lines.push((self.pos(), stmt.base().location.start.row));
                self.emit_assign(stmt);
            }
            Stmt::IfStmt(stmt) => {
                lines.push((self.pos(), stmt.base().location.start.row));
                self.emit_if_stmt(stmt, lines);
            }
            Stmt::WhileStmt(stmt) => {
                lines.push((self.pos(), stmt.base().location.start.row));
                self.emit_while_stmt(stmt, lines);
            }
            Stmt::ForStmt(stmt) => {
                lines.push((self.pos(), stmt.base().location.start.row));
                self.emit_for_stmt(stmt, lines);
            }
            Stmt::ReturnStmt(stmt) => {
                lines.push((self.pos(), stmt.base().location.start.row));
                if let Some(value) = &stmt.value {
                    self.emit_expression(value)
                } else {
                    self.emit_none_literal();
                }
                self.end_proc();
            }
        }
    }

    pub fn emit_local_var_init(&mut self, decl: &VarDef) {
        match &decl.value.content {
            LiteralContent::NoneLiteral(_) => {
                self.emit_none_literal();
            }
            LiteralContent::IntegerLiteral(i) => {
                self.emit_int_literal(i);
            }
            LiteralContent::BooleanLiteral(b) => {
                self.emit_bool_literal(b);
            }
            LiteralContent::StringLiteral(s) => {
                self.emit_string_literal(s);
            }
        }

        let target_type = ValueType::from_annotation(&decl.var.tv().type_);
        self.emit_coerce(decl.value.inferred_type.as_ref().unwrap(), &target_type);
        self.emit_push_rax();
    }

    pub fn emit_global_var_init(&mut self, decl: &VarDef) {
        let offset = if let Some(EnvSlot::Var(v, _)) =
            self.storage_env.get(&decl.var.tv().identifier.id().name)
        {
            assert!(v.level == 0);
            v.offset
        } else {
            panic!()
        };

        match &decl.value.content {
            LiteralContent::NoneLiteral(_) => {
                self.emit_none_literal();
            }
            LiteralContent::IntegerLiteral(i) => {
                self.emit_int_literal(i);
            }
            LiteralContent::BooleanLiteral(b) => {
                self.emit_bool_literal(b);
            }
            LiteralContent::StringLiteral(s) => {
                self.emit_string_literal(s);
            }
        }

        let target_type = ValueType::from_annotation(&decl.var.tv().type_);
        self.emit_coerce(decl.value.inferred_type.as_ref().unwrap(), &target_type);

        if target_type == *TYPE_INT {
            // mov [rip+{}],eax
            self.emit(&[0x89, 0x05]);
        } else if target_type == *TYPE_BOOL {
            // mov [rip+{}],al
            self.emit(&[0x88, 0x05]);
        } else {
            // mov [rip+{}],rax
            self.emit(&[0x48, 0x89, 0x05]);
        }
        self.links.push(ChunkLink {
            pos: self.pos(),
            to: GLOBAL_SECTION.to_owned(),
        });
        self.emit(&offset.to_le_bytes());
    }

    pub fn emit_global_var_drop(&mut self, decl: &VarDef) {
        let offset = if let Some(EnvSlot::Var(v, _)) =
            self.storage_env.get(&decl.var.tv().identifier.id().name)
        {
            assert!(v.level == 0);
            v.offset
        } else {
            panic!()
        };

        let target_type = ValueType::from_annotation(&decl.var.tv().type_);
        if target_type != *TYPE_INT && target_type != *TYPE_BOOL {
            // mov rax,[rip+{}]
            self.emit(&[0x48, 0x8B, 0x05]);
            self.links.push(ChunkLink {
                pos: self.pos(),
                to: GLOBAL_SECTION.to_owned(),
            });
            self.emit(&offset.to_le_bytes());
            self.emit_drop();
        }
    }
}

fn gen_function(
    function: &FuncDef,
    storage_env: &mut StorageEnv,
    classes: &HashMap<String, ClassSlot>,
    level: u32,
    parent: Option<&str>,
) -> Vec<Chunk> {
    let link_name = if let Some(parent) = parent {
        parent.to_owned() + "." + &function.name.id().name
    } else {
        function.name.id().name.clone()
    };

    let mut locals = HashMap::new();
    let mut clean_up_list = vec![];

    let mut local_offset = if level == 0 { -8 } else { -16 };

    let mut params_debug = vec![];

    for (i, param) in function.params.iter().enumerate() {
        let offset;
        if i < 6 {
            offset = local_offset;
            local_offset -= 8;
        } else {
            offset = (i - 6) as i32 * 8 + 16;
        }
        let name = &param.tv().identifier.id().name;
        locals.insert(
            name.clone(),
            LocalSlot::Var(VarSlot {
                offset,
                level: level + 1,
            }),
        );
        let param_type = ValueType::from_annotation(&param.tv().type_);
        if param_type != *TYPE_INT && param_type != *TYPE_BOOL {
            clean_up_list.push(offset);
        }

        let (flatten_name, array_level) = param.tv().type_.flatten();
        params_debug.push(VarDebug {
            offset,
            line: param.base().location.start.row,
            name: name.clone(),
            var_type: TypeDebug {
                name: flatten_name.to_owned(),
                array_level,
            },
        })
    }

    let mut locals_debug = vec![];

    for declaration in &function.declarations {
        if let Declaration::VarDef(v) = declaration {
            let name = &v.var.tv().identifier.id().name;
            let offset = local_offset;
            local_offset -= 8;
            locals.insert(
                name.clone(),
                LocalSlot::Var(VarSlot {
                    offset,
                    level: level + 1,
                }),
            );
            let local_type = ValueType::from_annotation(&v.var.tv().type_);
            if local_type != *TYPE_INT && local_type != *TYPE_BOOL {
                clean_up_list.push(offset);
            }

            let (flatten_name, array_level) = v.var.tv().type_.flatten();
            locals_debug.push(VarDebug {
                offset,
                line: v.base().location.start.row,
                name: name.clone(),
                var_type: TypeDebug {
                    name: flatten_name.to_owned(),
                    array_level,
                },
            })
        } else if let Declaration::FuncDef(f) = declaration {
            let name = &f.name.id().name;
            locals.insert(
                name.clone(),
                LocalSlot::Func(FuncSlot {
                    link_name: link_name.clone() + "." + name,
                    level: level + 1,
                }),
            );
        }
    }

    let mut handle = storage_env.push(locals);

    let mut code = Emitter::new(&link_name, handle.inner(), classes, &clean_up_list, level);

    if level != 0 {
        code.emit_push_r10();
    }

    if function.params.len() >= 1 {
        code.emit_push_rdi();
    }

    if function.params.len() >= 2 {
        code.emit_push_rsi();
    }

    if function.params.len() >= 3 {
        code.emit_push_rdx();
    }

    if function.params.len() >= 4 {
        code.emit_push_rcx();
    }

    if function.params.len() >= 5 {
        code.emit_push_r8();
    }

    if function.params.len() >= 6 {
        code.emit_push_r9();
    }

    for declaration in &function.declarations {
        if let Declaration::VarDef(v) = declaration {
            code.emit_local_var_init(v);
        }
    }

    let mut lines = vec![(0, function.base().location.start.row)];

    for statement in &function.statements {
        code.emit_statement(statement, &mut lines);
    }

    code.emit_none_literal();
    code.end_proc();

    let (flatten_name, array_level) = function.return_type.flatten();

    let return_type_debug = TypeDebug {
        name: flatten_name.to_owned(),
        array_level,
    };

    let mut chunks = vec![code.finalize(ProcedureDebug {
        decl_line: function.statements[0].base().location.start.row,
        artificial: false,
        parent: if level == 0 {
            None
        } else {
            parent.map(str::to_owned)
        },
        lines,
        return_type: return_type_debug,
        params: params_debug,
        locals: locals_debug,
    })];

    // Note: put children functions after the parent one
    // so that debug tree can be generated sequentially
    for declaration in &function.declarations {
        if let Declaration::FuncDef(f) = declaration {
            chunks.append(&mut gen_function(
                &f,
                handle.inner(),
                classes,
                level + 1,
                Some(&link_name),
            ));
        }
    }

    chunks
}

fn gen_ctor(
    class_name: &str,
    class_slot: &ClassSlot,
    storage_env: &mut StorageEnv,
    classes: &HashMap<String, ClassSlot>,
) -> Chunk {
    let mut code = Emitter::new(class_name, storage_env, classes, &[], 0);

    code.prepare_call(2);
    // lea rdi,[rip+{}]
    code.emit(&[0x48, 0x8D, 0x3D]);
    code.links.push(ChunkLink {
        pos: code.pos(),
        to: class_name.to_owned() + ".$proto",
    });
    code.emit(&[0; 4]);
    // xor rsi,rsi
    code.emit(&[0x48, 0x31, 0xF6]);
    code.call(BUILTIN_ALLOC_OBJ);
    code.emit_push_rax();

    for (_, attribute) in &class_slot.attributes {
        match &attribute.init {
            LiteralContent::NoneLiteral(_) => {
                code.emit_none_literal();
            }
            LiteralContent::IntegerLiteral(i) => {
                code.emit_int_literal(i);
            }
            LiteralContent::BooleanLiteral(b) => {
                code.emit_bool_literal(b);
            }
            LiteralContent::StringLiteral(s) => {
                code.emit_string_literal(s);
            }
        }

        code.emit_coerce(&attribute.source_type, &attribute.target_type);
        // mov rdi,[rsp]
        code.emit(&[0x48, 0x8B, 0x3C, 0x24]);

        if attribute.target_type == *TYPE_INT {
            // mov [rdi+{}],eax
            code.emit(&[0x89, 0x87]);
        } else if attribute.target_type == *TYPE_BOOL {
            // mov [rdi+{}],al
            code.emit(&[0x88, 0x87]);
        } else {
            // mov [rdi+{}],rax
            code.emit(&[0x48, 0x89, 0x87]);
        }
        code.emit(&attribute.offset.to_le_bytes());
    }

    // mov rax,[rsp]
    code.emit(&[0x48, 0x8B, 0x04, 0x24]);
    code.emit_clone();
    code.prepare_call(1);
    // mov rdi,rax
    code.emit(&[0x48, 0x89, 0xC7]);
    code.call_virtual(16);

    code.emit_pop_rax();
    code.end_proc();
    code.finalize(ProcedureDebug {
        decl_line: 0,
        artificial: true,
        parent: None,
        lines: vec![],
        return_type: TypeDebug {
            name: class_name.to_owned(),
            array_level: 0,
        },
        params: vec![],
        locals: vec![],
    })
}

fn gen_dtor(
    class_name: &str,
    class_slot: &ClassSlot,
    storage_env: &mut StorageEnv,
    classes: &HashMap<String, ClassSlot>,
) -> Chunk {
    let mut code = Emitter::new(
        &(class_name.to_owned() + ".$dtor"),
        storage_env,
        classes,
        &[],
        0,
    );
    code.emit_push_rdi();
    for (_, attribute) in &class_slot.attributes {
        if attribute.target_type != *TYPE_INT && attribute.target_type != *TYPE_BOOL {
            // mov rax,[rsp]
            code.emit(&[0x48, 0x8B, 0x04, 0x24]);
            // mov rax,[rax+{}]
            code.emit(&[0x48, 0x8B, 0x80]);
            code.emit(&attribute.offset.to_le_bytes());
            code.emit_drop();
        }
    }
    code.emit_pop_rax();
    code.end_proc();
    code.finalize(ProcedureDebug {
        decl_line: 0,
        artificial: true,
        parent: None,
        lines: vec![],
        return_type: TypeDebug {
            name: "<None>".to_owned(),
            array_level: 0,
        },
        params: vec![VarDebug {
            offset: -8,
            line: 0,
            name: "self".to_owned(),
            var_type: TypeDebug {
                name: class_name.to_owned(),
                array_level: 0,
            },
        }],
        locals: vec![],
    })
}

pub(super) fn gen_code_set(ast: Ast) -> CodeSet {
    let mut globals = HashMap::new();
    let mut classes = HashMap::new();
    let mut base_methods = HashMap::new();
    base_methods.insert(
        "$dtor".to_owned(),
        MethodSlot {
            offset: 8,
            link_name: "object.$dtor".to_owned(),
        },
    );
    base_methods.insert(
        "__init__".to_owned(),
        MethodSlot {
            offset: 16,
            link_name: "object.__init__".to_owned(),
        },
    );
    classes.insert(
        "object".to_owned(),
        ClassSlot {
            attributes: HashMap::new(),
            object_size: 0,
            methods: base_methods,
            prototype_size: 24,
        },
    );
    let mut global_offset = 0;
    let mut globals_debug = vec![];
    for declaration in &ast.program().declarations {
        if let Declaration::VarDef(v) = declaration {
            let name = &v.var.tv().identifier.id().name;
            let target_type = ValueType::from_annotation(&v.var.tv().type_);
            let size = if target_type == *TYPE_INT {
                4
            } else if target_type == *TYPE_BOOL {
                1
            } else {
                8
            };
            global_offset += (size - global_offset % size) % size;
            globals.insert(
                name.clone(),
                LocalSlot::Var(VarSlot {
                    offset: global_offset,
                    level: 0,
                }),
            );

            let (flatten_name, array_level) = v.var.tv().type_.flatten();
            globals_debug.push(VarDebug {
                offset: global_offset,
                line: v.base().location.start.row,
                name: name.clone(),
                var_type: TypeDebug {
                    name: flatten_name.to_owned(),
                    array_level,
                },
            });

            global_offset += size;
        } else if let Declaration::FuncDef(f) = declaration {
            let name = &f.name.id().name;
            globals.insert(
                name.clone(),
                LocalSlot::Func(FuncSlot {
                    link_name: name.clone(),
                    level: 0,
                }),
            );
        } else if let Declaration::ClassDef(c) = declaration {
            let class_name = &c.name.id().name;
            let mut class_slot = classes.get(&c.super_class.id().name).unwrap().clone();
            class_slot.methods.get_mut("$dtor").unwrap().link_name = class_name.clone() + ".$dtor";
            globals.insert(
                class_name.clone(),
                LocalSlot::Func(FuncSlot {
                    link_name: class_name.clone(),
                    level: 0,
                }),
            );
            for declaration in &c.declarations {
                if let Declaration::VarDef(v) = declaration {
                    let source_type = v.value.inferred_type.clone().unwrap();
                    let target_type = ValueType::from_annotation(&v.var.tv().type_);
                    let size = if target_type == *TYPE_INT {
                        4
                    } else if target_type == *TYPE_BOOL {
                        1
                    } else {
                        8
                    };
                    class_slot.object_size += (size - class_slot.object_size % size) % size;
                    class_slot.attributes.insert(
                        v.var.tv().identifier.id().name.clone(),
                        AttributeSlot {
                            offset: class_slot.object_size + 16,
                            source_type,
                            target_type,
                            init: v.value.content.clone(),
                        },
                    );
                    class_slot.object_size += size;
                } else if let Declaration::FuncDef(f) = declaration {
                    let method_name = &f.name.id().name;
                    let link_name = class_name.clone() + "." + method_name;
                    if let Some(method) = class_slot.methods.get_mut(method_name) {
                        method.link_name = link_name;
                    } else {
                        class_slot.methods.insert(
                            method_name.clone(),
                            MethodSlot {
                                offset: class_slot.prototype_size,
                                link_name,
                            },
                        );
                        class_slot.prototype_size += 8;
                    }
                }
            }
            classes.insert(class_name.clone(), class_slot);
        }
    }

    let insert_builtin = |globals: &mut HashMap<_, _>, name: &str| {
        globals.insert(
            name.to_owned(),
            LocalSlot::Func(FuncSlot {
                link_name: name.to_owned(),
                level: 0,
            }),
        )
    };

    insert_builtin(&mut globals, "len");
    insert_builtin(&mut globals, "print");
    insert_builtin(&mut globals, "input");
    insert_builtin(&mut globals, "str");
    insert_builtin(&mut globals, "int");
    insert_builtin(&mut globals, "bool");
    insert_builtin(&mut globals, "object");

    let mut storage_env = StorageEnv::new(globals);

    let mut main_code = Emitter::new(BUILTIN_CHOCOPY_MAIN, &mut storage_env, &classes, &[], 0);

    // mov rax,0x12345678
    main_code.emit(&[0x48, 0xC7, 0xC0, 0x78, 0x56, 0x34, 0x12]);
    main_code.emit_push_rax();

    for declaration in &ast.program().declarations {
        if let Declaration::VarDef(v) = declaration {
            main_code.emit_global_var_init(v);
        }
    }

    let mut lines = vec![];

    for statement in &ast.program().statements {
        main_code.emit_statement(statement, &mut lines);
    }

    for declaration in &ast.program().declarations {
        if let Declaration::VarDef(v) = declaration {
            main_code.emit_global_var_drop(v);
        }
    }

    main_code.emit_pop_rax();
    // cmp rax,0x12345678
    main_code.emit(&[0x48, 0x3D, 0x78, 0x56, 0x34, 0x12]);

    // je
    main_code.emit(&[0x0f, 0x84]);
    let pos = main_code.pos();
    main_code.emit(&[0; 4]);

    main_code.prepare_call(0);
    main_code.call(BUILTIN_BROKEN_STACK);

    let delta = (main_code.pos() - pos - 4) as u32;
    main_code.code[pos..pos + 4].copy_from_slice(&delta.to_le_bytes());

    main_code.end_proc();

    let main_procedure = main_code.finalize(ProcedureDebug {
        decl_line: ast
            .program()
            .statements
            .get(0)
            .map_or(1, |s| s.base().location.start.row),
        artificial: false,
        parent: None,
        lines,
        return_type: TypeDebug {
            name: "<None>".to_owned(),
            array_level: 0,
        },
        params: vec![],
        locals: vec![],
    });

    let mut chunks = vec![main_procedure];

    for declaration in &ast.program().declarations {
        if let Declaration::FuncDef(f) = declaration {
            chunks.append(&mut gen_function(&f, &mut storage_env, &classes, 0, None));
        } else if let Declaration::ClassDef(c) = declaration {
            for declaration in &c.declarations {
                if let Declaration::FuncDef(f) = declaration {
                    chunks.append(&mut gen_function(
                        &f,
                        &mut storage_env,
                        &classes,
                        0,
                        Some(&c.name.id().name),
                    ));
                }
            }
        }
    }

    for (class_name, class_slot) in &classes {
        chunks.push(gen_ctor(
            &class_name,
            &class_slot,
            &mut storage_env,
            &classes,
        ));
        chunks.push(gen_dtor(
            &class_name,
            &class_slot,
            &mut storage_env,
            &classes,
        ));

        let mut prototype = vec![0; class_slot.prototype_size as usize];
        prototype[0..8].copy_from_slice(&(class_slot.object_size as u64).to_le_bytes());
        let links = class_slot
            .methods
            .iter()
            .map(|(_, method)| ChunkLink {
                pos: method.offset as usize,
                to: method.link_name.clone(),
            })
            .collect();
        chunks.push(Chunk {
            name: class_name.clone() + ".$proto",
            code: prototype,
            links,
            extra: ChunkExtra::Data,
        });
    }

    CodeSet {
        chunks,
        global_size: global_offset as usize,
        globals_debug,
    }
}
