use crate::local_env::*;
use crate::node::*;
use faerie::*;
use std::collections::HashMap;
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
const GLOBAL_SECTION: &'static str = "$global";

struct FuncSlot {
    link_name: String,
    level: u32, // 0 = global function
}

struct VarSlot {
    offset: i32, // relative to global seciton or rbp
    level: u32,  // 0 = global variable
}

type StorageEnv = LocalEnv<FuncSlot, VarSlot>;

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
    global_size: usize,
}

struct Emitter<'a> {
    name: String,
    storage_env: &'a mut StorageEnv,
    clean_up_list: &'a [i32], // offsets relative to rbp
    level: u32,
    code: Vec<u8>,
    links: Vec<ProcedureLink>,
    rsp_aligned: bool,
    rsp_call_restore: Vec<(usize, bool)>,
    strings: Vec<(usize, String)>,
}

impl<'a> Emitter<'a> {
    pub fn new(
        name: &str,
        storage_env: &'a mut StorageEnv,
        clean_up_list: &'a [i32],
        level: u32,
    ) -> Emitter<'a> {
        Emitter {
            name: name.to_owned(),
            storage_env,
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
        // mov rax,{i}
        self.emit(&[0x48, 0xc7, 0xc0]);
        self.emit(&i.value.to_le_bytes());
    }

    pub fn emit_bool_literal(&mut self, b: &BooleanLiteral) {
        if b.value {
            // mov rax,1
            self.emit(&[0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00]);
        } else {
            // xor rax,rax
            self.emit(&[0x48, 0x31, 0xC0]);
        }
    }

    pub fn emit_string_literal(&mut self, s: &StringLiteral) {
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
        self.links.push(ProcedureLink {
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
            // movsx rax,eax
            self.emit(&[0x48, 0x63, 0xC0]);
            // add rsi,4
            self.emit(&[0x48, 0x83, 0xC6, 0x04]);
        } else if source_element == &*TYPE_BOOL {
            // mov al,[rsi]
            self.emit(&[0x8A, 0x06]);
            // movzx rax,al
            self.emit(&[0x48, 0x0F, 0xB6, 0xC0]);
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
        self.links.push(ProcedureLink {
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
        if expr.operator == BinaryOp::Add && expr.left.inferred_type.as_ref().unwrap() == &*TYPE_STR
        {
            self.emit_string_add(expr);
        } else if expr.operator == BinaryOp::Add
            && expr.left.inferred_type.as_ref().unwrap() != &*TYPE_INT
        {
            let target_element = if let ValueType::ListValueType(l) = &target_type {
                &*l.element_type
            } else {
                panic!()
            };
            self.emit_list_add(expr, target_element);
        } else if (expr.operator == BinaryOp::Eq || expr.operator == BinaryOp::Ne)
            && expr.left.inferred_type.as_ref().unwrap() == &*TYPE_STR
        {
            self.emit_str_compare(expr);
        } else if expr.operator == BinaryOp::Or || expr.operator == BinaryOp::And {
            self.emit_expression(&expr.left);
            // test rax,rax
            self.emit(&[0x48, 0x85, 0xC0]);
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
                    // add rax,r11
                    self.emit(&[0x4C, 0x01, 0xD8]);
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
                    // cmp r11,rax
                    self.emit(&[0x49, 0x39, 0xC3]);
                    // set* al
                    self.emit(&[0x0f, 0x90 + code, 0xc0]);
                    // movzx rax,al
                    self.emit(&[0x48, 0x0f, 0xb6, 0xc0]);
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

    pub fn emit_call_expr(&mut self, expr: &CallExpr) {
        self.prepare_call(expr.args.len());

        for (i, arg) in expr.args.iter().enumerate() {
            self.emit_expression(arg);

            let param_type = &expr
                .function
                .id()
                .inferred_type
                .as_ref()
                .unwrap()
                .func_type()
                .parameters[i];

            self.emit_coerce(arg.inferred_type.as_ref().unwrap(), param_type);

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

        if expr.args.len() >= 6 {
            self.emit_pop_r9();
        }

        if expr.args.len() >= 5 {
            self.emit_pop_r8();
        }

        if expr.args.len() >= 4 {
            self.emit_pop_rcx();
        }

        if expr.args.len() >= 3 {
            self.emit_pop_rdx();
        }

        if expr.args.len() >= 2 {
            self.emit_pop_rsi();
        }

        if expr.args.len() >= 1 {
            self.emit_pop_rdi();
        }

        let slot = if let Some(EnvSlot::Func(f)) = self.storage_env.get(&expr.function.id().name) {
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

    pub fn emit_str_index(&mut self, expr: &IndexExpr) {
        self.emit_expression(&expr.list);
        self.emit_push_rax();
        self.emit_expression(&expr.index);
        self.emit_push_rax();
        self.prepare_call(2);
        // mov rdi,[rip+{STR_PROTOTYPE}]
        self.emit(&[0x48, 0x8B, 0x3D]);
        self.links.push(ProcedureLink {
            pos: self.pos(),
            to: STR_PROTOTYPE.to_owned(),
        });
        self.emit(&[0; 4]);
        // mov rsi,1
        self.emit(&[0x48, 0xc7, 0xc6, 0x01, 0x00, 0x00, 0x00]);
        self.call(BUILTIN_ALLOC_OBJ);
        self.emit_pop_rsi();
        self.emit_pop_r11();
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
        self.emit_push_rax();
        self.emit_expression(&expr.index);
        self.emit_pop_rsi();
        let element_type = if let Some(ValueType::ListValueType(l)) = &expr.list.inferred_type {
            &*l.element_type
        } else {
            panic!()
        };

        if element_type == &*TYPE_INT {
            // mov eax,[rsi+rax*4+24]
            self.emit(&[0x8B, 0x44, 0x86, 0x18]);
            // movsx rax,eax
            self.emit(&[0x48, 0x63, 0xC0]);
        } else if element_type == &*TYPE_BOOL {
            // mov al,[rsi+rax+24]
            self.emit(&[0x8A, 0x44, 0x06, 0x18]);
            // movzx rax,al
            self.emit(&[0x48, 0x0F, 0xB6, 0xC0]);
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

    pub fn emit_if_expr(&mut self, expr: &IfExpr, target_type: &ValueType) {
        self.emit_expression(&expr.condition);
        // test rax,rax
        self.emit(&[0x48, 0x85, 0xC0]);
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

    pub fn emit_if_stmt(&mut self, stmt: &IfStmt) {
        self.emit_expression(&stmt.condition);
        // test rax,rax
        self.emit(&[0x48, 0x85, 0xC0]);
        // je
        self.emit(&[0x0f, 0x84]);
        let pos_if = self.pos();
        self.emit(&[0; 4]);

        for stmt in &stmt.then_body {
            self.emit_statement(stmt);
        }

        // jmp
        self.emit(&[0xe9]);
        let pos_else = self.pos();
        self.emit(&[0; 4]);
        let if_delta = self.pos() - pos_if - 4;
        self.code[pos_if..pos_if + 4].copy_from_slice(&(if_delta as u32).to_le_bytes());

        for stmt in &stmt.else_body {
            self.emit_statement(stmt);
        }

        let else_delta = self.pos() - pos_else - 4;
        self.code[pos_else..pos_else + 4].copy_from_slice(&(else_delta as u32).to_le_bytes());
    }

    pub fn emit_list_expr(&mut self, expr: &ListExpr, target_type: &ValueType) {
        if target_type == &*TYPE_EMPTY {
            self.prepare_call(2);
            // mov rdi,[rip+{_PROTOTYPE}]
            self.emit(&[0x48, 0x8B, 0x3D]);
            self.links.push(ProcedureLink {
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
        self.links.push(ProcedureLink {
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
                self.links.push(ProcedureLink {
                    pos: self.pos(),
                    to: GLOBAL_SECTION.to_owned(),
                });
                self.emit(&offset.to_le_bytes());
                // movsx rax,eax
                self.emit(&[0x48, 0x63, 0xC0]);
            } else if target_type == &*TYPE_BOOL {
                // mov al,[rip+{}]
                self.emit(&[0x8A, 0x05]);
                self.links.push(ProcedureLink {
                    pos: self.pos(),
                    to: GLOBAL_SECTION.to_owned(),
                });
                self.emit(&offset.to_le_bytes());
                // movzx rax,al
                self.emit(&[0x48, 0x0F, 0xB6, 0xC0]);
            } else {
                // mov rax,[rip+{}]
                self.emit(&[0x48, 0x8B, 0x05]);
                self.links.push(ProcedureLink {
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
                self.emit_call_expr(expr);
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
            _ => unimplemented!(),
        }
    }

    pub fn emit_while_stmt(&mut self, stmt: &WhileStmt) {
        let pos_start = self.pos();
        self.emit_expression(&stmt.condition);
        // test rax,rax
        self.emit(&[0x48, 0x85, 0xC0]);
        // je
        self.emit(&[0x0f, 0x84]);
        let pos_condition = self.pos();
        self.emit(&[0; 4]);

        for stmt in &stmt.body {
            self.emit_statement(stmt);
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
                self.links.push(ProcedureLink {
                    pos: self.pos(),
                    to: GLOBAL_SECTION.to_owned(),
                });
                self.emit(&offset.to_le_bytes());
                self.emit_drop();
                self.emit_pop_rax();
                // mov [rip+{}],rax
                self.emit(&[0x48, 0x89, 0x05]);
            }
            self.links.push(ProcedureLink {
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
                    self.emit_push_rax();
                    self.emit_expression(&expr.index);
                    // mov rsi,[rsp]
                    self.emit(&[0x48, 0x8B, 0x34, 0x24]);

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
                ExprContent::MemberExpr(_expr) => unimplemented!(),
                _ => panic!(),
            }
        }

        self.emit_pop_rax();
        if source_type != &*TYPE_INT && source_type != &*TYPE_BOOL {
            self.emit_drop();
        }
    }

    pub fn emit_for_stmt(&mut self, stmt: &ForStmt) {
        //// Compute the iterable
        self.emit_expression(&stmt.iterable);
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
            self.links.push(ProcedureLink {
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
                // movsx rax,eax
                self.emit(&[0x48, 0x63, 0xC0]);
            } else if element_type == &*TYPE_BOOL {
                // mov al,[rsi+rax+24]
                self.emit(&[0x8A, 0x44, 0x06, 0x18]);
                // movzx rax,al
                self.emit(&[0x48, 0x0F, 0xB6, 0xC0]);
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
            self.emit_statement(stmt);
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
            Stmt::AssignStmt(stmt) => {
                self.emit_assign(stmt);
            }
            Stmt::IfStmt(stmt) => {
                self.emit_if_stmt(stmt);
            }
            Stmt::WhileStmt(stmt) => {
                self.emit_while_stmt(stmt);
            }
            Stmt::ForStmt(stmt) => {
                self.emit_for_stmt(stmt);
            }
            Stmt::ReturnStmt(stmt) => {
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
        self.links.push(ProcedureLink {
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
            self.links.push(ProcedureLink {
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
    level: u32,
    prefix: &str,
) -> Vec<Procedure> {
    let link_name = prefix.to_owned() + &function.name.id().name;
    let prefix = link_name.clone() + ".";

    let mut locals = HashMap::new();
    let mut clean_up_list = vec![];

    let mut local_offset = if level == 0 { -8 } else { -16 };

    for (i, param) in function.params.iter().enumerate() {
        let offset;
        if i < 6 {
            offset = local_offset;
            local_offset -= 8;
        } else {
            offset = (i - 6) as i32 * 8 + 16;
        }
        locals.insert(
            param.tv().identifier.id().name.clone(),
            LocalSlot::Var(VarSlot {
                offset,
                level: level + 1,
            }),
        );
        let param_type = ValueType::from_annotation(&param.tv().type_);
        if param_type != *TYPE_INT && param_type != *TYPE_BOOL {
            clean_up_list.push(offset);
        }
    }
    for declaration in &function.declarations {
        if let Declaration::VarDef(v) = declaration {
            let offset = local_offset;
            local_offset -= 8;
            locals.insert(
                v.var.tv().identifier.id().name.clone(),
                LocalSlot::Var(VarSlot {
                    offset,
                    level: level + 1,
                }),
            );
            let local_type = ValueType::from_annotation(&v.var.tv().type_);
            if local_type != *TYPE_INT && local_type != *TYPE_BOOL {
                clean_up_list.push(offset);
            }
        } else if let Declaration::FuncDef(f) = declaration {
            let name = &f.name.id().name;
            locals.insert(
                name.clone(),
                LocalSlot::Func(FuncSlot {
                    link_name: prefix.clone() + name,
                    level: level + 1,
                }),
            );
        }
    }

    let mut handle = storage_env.push(locals);

    let mut code = Emitter::new(&link_name, handle.inner(), &clean_up_list, level);

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

    for statement in &function.statements {
        code.emit_statement(statement);
    }

    code.emit_none_literal();
    code.end_proc();

    let mut procedures = vec![code.finalize()];

    for declaration in &function.declarations {
        if let Declaration::FuncDef(f) = declaration {
            procedures.append(&mut gen_function(&f, handle.inner(), level + 1, &prefix));
        }
    }

    procedures
}

fn gen_code_set(ast: Ast) -> CodeSet {
    let mut globals = HashMap::new();
    let mut global_offset = 0;
    for declaration in &ast.program().declarations {
        if let Declaration::VarDef(v) = declaration {
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
                v.var.tv().identifier.id().name.clone(),
                LocalSlot::Var(VarSlot {
                    offset: global_offset,
                    level: 0,
                }),
            );
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

    let mut storage_env = StorageEnv::new(globals);

    let mut main_code = Emitter::new(BUILTIN_CHOCOPY_MAIN, &mut storage_env, &[], 0);

    // mov rax,0x12345678
    main_code.emit(&[0x48, 0xC7, 0xC0, 0x78, 0x56, 0x34, 0x12]);
    main_code.emit_push_rax();

    for declaration in &ast.program().declarations {
        if let Declaration::VarDef(v) = declaration {
            main_code.emit_global_var_init(v);
        }
    }

    for statement in &ast.program().statements {
        main_code.emit_statement(statement);
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
    main_code.call(BUILTIN_REPORT_BROKEN_STACK);

    let delta = (main_code.pos() - pos - 4) as u32;
    main_code.code[pos..pos + 4].copy_from_slice(&delta.to_le_bytes());

    main_code.end_proc();

    let main_procedure = main_code.finalize();

    let mut procedures = vec![main_procedure];

    for declaration in &ast.program().declarations {
        if let Declaration::FuncDef(f) = declaration {
            procedures.append(&mut gen_function(&f, &mut storage_env, 0, ""));
        }
    }

    CodeSet {
        procedures,
        global_size: global_offset as usize,
    }
}

pub fn gen(ast: Ast, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let obj_name = format!("chocopy-{}.o", rand::random::<u32>());
    let mut obj_path = std::env::temp_dir();
    obj_path.push(obj_name.clone());

    let mut obj = ArtifactBuilder::new(triple!("x86_64-pc-linux-gnu-elf"))
        .name(obj_name)
        .finish();

    let mut declarations = vec![
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
        // global
        (GLOBAL_SECTION, Decl::data().writable().into()),
    ];

    let code_set = gen_code_set(ast);

    for procedure in &code_set.procedures {
        declarations.push((&procedure.name, Decl::function().global().into()));
    }

    obj.declarations(declarations.into_iter())?;

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

    obj.define(GLOBAL_SECTION, vec![0; code_set.global_size])?;

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
