use super::*;

struct FuncSlot {
    link_name: String,
    level: u32, // 0 = global function / method
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
    return_type: Option<&'a ValueType>,
    storage_env: Option<&'a StorageEnv>,
    classes: Option<&'a HashMap<String, ClassSlot>>,
    current_stack_top: i32, // relative to rbp, non-positive
    max_stack_top: i32,     // relative to rbp, non-positive
    max_parameter: usize,
    clean_up_list: Vec<i32>, // offsets relative to rbp
    level: u32,
    code: Vec<u8>,
    links: Vec<ChunkLink>,
    platform: Platform,
}

impl Platform {
    fn stack_reserve(self) -> usize {
        match self {
            Platform::Windows => 4,
            Platform::Linux | Platform::Macos => 0,
        }
    }
}

#[must_use]
struct ForwardJumper {
    from: usize,
}

#[must_use]
struct BackwardJumper {
    to: usize,
}

#[must_use]
struct StackTicket {
    offset: i32, // relative to rbp
}

impl Drop for StackTicket {
    fn drop(&mut self) {
        panic!()
    }
}

#[derive(PartialEq, Eq)]
enum TicketType {
    Unknown,
    Plain,
    Reference,
}

impl<'a> Emitter<'a> {
    pub fn new_simple(name: &str, platform: Platform) -> Emitter<'a> {
        Emitter::new(name, None, None, None, vec![], 0, platform)
    }

    pub fn new(
        name: &str,
        return_type: Option<&'a ValueType>,
        storage_env: Option<&'a StorageEnv>,
        classes: Option<&'a HashMap<String, ClassSlot>>,
        clean_up_list: Vec<i32>,
        level: u32,
        platform: Platform,
    ) -> Emitter<'a> {
        Emitter {
            name: name.to_owned(),
            return_type,
            storage_env,
            classes,
            current_stack_top: 0,
            max_stack_top: 0,
            max_parameter: 0,
            clean_up_list,
            level,
            // push rbp; mov rbp,rsp; add rsp,{}
            code: vec![0x55, 0x48, 0x89, 0xe5, 0x48, 0x81, 0xEC, 0, 0, 0, 0],
            links: vec![],
            platform,
        }
    }

    pub fn storage_env(&self) -> &'a StorageEnv {
        self.storage_env.as_ref().unwrap()
    }

    pub fn classes(&self) -> &'a HashMap<String, ClassSlot> {
        self.classes.as_ref().unwrap()
    }

    pub fn emit(&mut self, instruction: &[u8]) {
        self.code.extend_from_slice(&instruction);
    }

    pub fn pos(&self) -> usize {
        self.code.len()
    }

    pub fn alloc_stack(&mut self, ticket_type: TicketType) -> StackTicket {
        self.current_stack_top -= 8;
        self.max_stack_top = std::cmp::min(self.max_stack_top, self.current_stack_top);
        if ticket_type == TicketType::Reference {
            self.clean_up_list.push(self.current_stack_top);
        }
        StackTicket {
            offset: self.current_stack_top,
        }
    }

    pub fn free_stack(&mut self, ticket: StackTicket) {
        assert!(ticket.offset == self.current_stack_top);
        if self.clean_up_list.last() == Some(&self.current_stack_top) {
            self.clean_up_list.pop();
        }
        self.current_stack_top += 8;
        std::mem::forget(ticket);
    }

    pub fn emit_with_stack(&mut self, instruction: &[u8], ticket: &StackTicket) {
        self.emit(instruction);
        self.emit(&ticket.offset.to_le_bytes());
    }

    pub fn jump_from(&mut self) -> ForwardJumper {
        let from = self.pos();
        self.emit(&[0; 4]);
        ForwardJumper { from }
    }

    #[allow(clippy::wrong_self_convention)] // No, this is not a to_type function
    pub fn to_here(&mut self, jump: ForwardJumper) {
        let from = jump.from;
        let delta = (self.pos() - from - 4) as u32;
        self.code[from..from + 4].copy_from_slice(&delta.to_le_bytes());
    }

    pub fn jump_to(&self) -> BackwardJumper {
        BackwardJumper { to: self.pos() }
    }

    pub fn from_here(&mut self, jump: BackwardJumper) {
        let delta = -((self.pos() - jump.to + 4) as i32);
        self.emit(&delta.to_le_bytes());
    }

    pub fn end_proc(&mut self) {
        if !self.clean_up_list.is_empty() {
            let return_value = self.alloc_stack(TicketType::Unknown);
            // mov [rbp+{}],rax
            self.emit_with_stack(&[0x48, 0x89, 0x85], &return_value);
            for offset in self.clean_up_list.clone() {
                // mov rax,[rbp+{}]
                self.emit(&[0x48, 0x8B, 0x85]);
                self.emit(&offset.to_le_bytes());
                self.emit_drop();
            }
            // mov rax,[rbp+{}]
            self.emit_with_stack(&[0x48, 0x8B, 0x85], &return_value);
            self.free_stack(return_value);
        }
        // leave; ret
        self.emit(&[0xc9, 0xc3])
    }

    pub fn prepare_call(&mut self, stack_reserve: usize) {
        self.max_parameter = std::cmp::max(self.max_parameter, stack_reserve * 8);
    }

    pub fn emit_link(&mut self, name: impl Into<String>, offset: i32) {
        self.links.push(ChunkLink {
            pos: self.pos(),
            to: ChunkLinkTarget::Symbol(name.into()),
        });
        self.emit(&offset.to_le_bytes());
    }

    pub fn call(&mut self, name: &str) {
        self.emit(&[0xe8]);
        self.emit_link(name, 0);
    }

    pub fn call_virtual(&mut self, offset: u32) {
        // mov rdi,[rsp]
        self.emit(&[0x48, 0x8B, 0x3C, 0x24]);
        // mov rax,[rdi]
        self.emit(&[0x48, 0x8B, 0x07]);
        // call [rax+{}]
        self.emit(&[0xFF, 0x90]);
        self.emit(&offset.to_le_bytes());
    }

    pub fn finalize(mut self, procedure_debug: ProcedureDebug) -> Chunk {
        let mut frame_size = self.max_parameter as i32 - self.max_stack_top;
        if frame_size % 16 == 8 {
            frame_size += 8;
        }
        self.code[7..11].copy_from_slice(&frame_size.to_le_bytes());
        Chunk {
            name: self.name,
            code: self.code,
            links: self.links,
            extra: ChunkExtra::Procedure(procedure_debug),
        }
    }

    pub fn call_builtin_alloc(&mut self, prototype: &str) {
        match self.platform {
            Platform::Windows => {
                // mov rdx,rsi
                self.emit(&[0x48, 0x89, 0xF2]);
                // lea rcx,[rip+{_PROTOTYPE}]
                self.emit(&[0x48, 0x8D, 0x0D]);
            }
            Platform::Linux | Platform::Macos => {
                // lea rdi,[rip+{_PROTOTYPE}]
                self.emit(&[0x48, 0x8D, 0x3D]);
            }
        }
        self.emit_link(prototype, 0);
        self.prepare_call(self.platform.stack_reserve());
        self.call(BUILTIN_ALLOC_OBJ);
    }

    pub fn emit_check_none(&mut self) {
        // test rax,rax
        self.emit(&[0x48, 0x85, 0xC0]);
        // jne
        self.emit(&[0x0F, 0x85]);
        let ok = self.jump_from();
        self.prepare_call(self.platform.stack_reserve());
        self.call(BUILTIN_NONE_OP);
        self.to_here(ok);
    }

    pub fn emit_box_int(&mut self) {
        let value = self.alloc_stack(TicketType::Plain);
        // mov [rbp+{}],rax
        self.emit_with_stack(&[0x48, 0x89, 0x85], &value);
        // xor rsi,rsi
        self.emit(&[0x48, 0x31, 0xF6]);
        self.call_builtin_alloc(INT_PROTOTYPE);
        // mov rcx,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0x8D], &value);
        self.free_stack(value);
        // mov DWORD PTR [rax+0x10],ecx
        self.emit(&[0x89, 0x48, 0x10]);
    }

    pub fn emit_box_bool(&mut self) {
        let value = self.alloc_stack(TicketType::Plain);
        // mov [rbp+{}],rax
        self.emit_with_stack(&[0x48, 0x89, 0x85], &value);
        // xor rsi,rsi
        self.emit(&[0x48, 0x31, 0xF6]);
        self.call_builtin_alloc(BOOL_PROTOTYPE);
        // mov rcx,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0x8D], &value);
        self.free_stack(value);
        // mov BYTE PTR [rax+0x10],cl
        self.emit(&[0x88, 0x48, 0x10]);
    }

    pub fn emit_drop(&mut self) {
        // test rax,rax
        self.emit(&[0x48, 0x85, 0xC0]);
        // je
        self.emit(&[0x0f, 0x84]);
        let skip_a = self.jump_from();
        // sub QWORD PTR [rax+8],1
        self.emit(&[0x48, 0x83, 0x68, 0x08, 0x01]);

        // jne
        self.emit(&[0x0f, 0x85]);
        let skip_b = self.jump_from();

        self.prepare_call(self.platform.stack_reserve());
        match self.platform {
            Platform::Windows => self.emit(&[0x48, 0x89, 0xc1]), // mov rcx,rax
            Platform::Linux | Platform::Macos => self.emit(&[0x48, 0x89, 0xc7]), // mov rdi,rax
        }
        self.call(BUILTIN_FREE_OBJ);

        self.to_here(skip_b);
        self.to_here(skip_a);
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

    pub fn emit_int_literal(&mut self, i: i32) {
        // mov eax,{i}
        self.emit(&[0xB8]);
        self.emit(&i.to_le_bytes());
    }

    pub fn emit_bool_literal(&mut self, b: bool) {
        // mov al,{}
        self.emit(&[0xB0, b as u8]);
    }

    pub fn emit_string_literal(&mut self, s: &str) {
        // mov rsi,{len}
        self.emit(&[0x48, 0xc7, 0xc6]);
        self.emit(&(s.len() as u32).to_le_bytes());
        self.call_builtin_alloc(STR_PROTOTYPE);
        if !s.is_empty() {
            // lea rdi,[rax+24]
            self.emit(&[0x48, 0x8D, 0x78, 0x18]);
            // lea rsi,[rip+{STR}]
            self.emit(&[0x48, 0x8d, 0x35]);
            self.links.push(ChunkLink {
                pos: self.pos(),
                to: ChunkLinkTarget::Data(s.into()),
            });
            self.emit(&[0; 4]);
            // mov rcx,{len}
            self.emit(&[0x48, 0xc7, 0xc1]);
            self.emit(&(s.len() as u32).to_le_bytes());
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
        let left = self.alloc_stack(TicketType::Reference);
        let left_len = self.alloc_stack(TicketType::Plain);
        // mov [rbp+{}],rax
        self.emit_with_stack(&[0x48, 0x89, 0x85], &left);
        // mov [rbp+{}],rsi
        self.emit_with_stack(&[0x48, 0x89, 0xB5], &left_len);

        self.emit_expression(&expr.right);
        // mov rsi,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0xB5], &left_len);
        self.free_stack(left_len);
        // add rsi,QWORD PTR [rax+0x10]
        self.emit(&[0x48, 0x03, 0x70, 0x10]);
        let right = self.alloc_stack(TicketType::Reference);
        // mov [rbp+{}],rax
        self.emit_with_stack(&[0x48, 0x89, 0x85], &right);
        self.call_builtin_alloc(STR_PROTOTYPE);

        // mov r11,[rbp+{}]
        self.emit_with_stack(&[0x4C, 0x8B, 0x9D], &right);
        // mov r10,[rbp+{}]
        self.emit_with_stack(&[0x4C, 0x8B, 0x95], &left);

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

        let result = self.alloc_stack(TicketType::Reference);
        // mov [rbp+{}],rax
        self.emit_with_stack(&[0x48, 0x89, 0x85], &result);
        // mov rax,r10
        self.emit(&[0x4C, 0x89, 0xD0]);
        self.emit_drop();
        // mov rax,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0x85], &right);
        self.emit_drop();
        // mov rax,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0x85], &result);

        self.free_stack(result);
        self.free_stack(right);
        self.free_stack(left);
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
        let skip = self.jump_from();
        // add rsi,24
        self.emit(&[0x48, 0x83, 0xC6, 0x18]);
        let loop_pos = self.jump_to();

        let dest = self.alloc_stack(TicketType::Plain);
        let src = self.alloc_stack(TicketType::Plain);
        let counter = self.alloc_stack(TicketType::Plain);

        // mov [rbp+{}],rax
        self.emit_with_stack(&[0x48, 0x89, 0x85], &dest);

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

        // mov [rbp+{}],rsi
        self.emit_with_stack(&[0x48, 0x89, 0xB5], &src);
        // mov [rbp+{}],rcx
        self.emit_with_stack(&[0x48, 0x89, 0x8D], &counter);
        self.emit_coerce(source_element, target_element);
        // mov r11,rax
        self.emit(&[0x49, 0x89, 0xC3]);
        // mov rcx,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0x8D], &counter);
        // mov rsi,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0xB5], &src);
        // mov rax,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0x85], &dest);

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
        self.from_here(loop_pos);

        self.to_here(skip);

        self.free_stack(counter);
        self.free_stack(src);
        self.free_stack(dest);
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
        self.emit_check_none();
        // mov rsi,QWORD PTR [rax+0x10]
        self.emit(&[0x48, 0x8B, 0x70, 0x10]);
        let left = self.alloc_stack(TicketType::Reference);
        // mov [rbp+{}],rax
        self.emit_with_stack(&[0x48, 0x89, 0x85], &left);
        let left_size = self.alloc_stack(TicketType::Plain);
        // mov [rbp+{}],rsi
        self.emit_with_stack(&[0x48, 0x89, 0xB5], &left_size);
        self.emit_expression(&expr.right);
        self.emit_check_none();
        // mov rsi,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0xB5], &left_size);
        self.free_stack(left_size);
        // add rsi,QWORD PTR [rax+0x10]
        self.emit(&[0x48, 0x03, 0x70, 0x10]);
        let right = self.alloc_stack(TicketType::Reference);
        // mov [rbp+{}],rax
        self.emit_with_stack(&[0x48, 0x89, 0x85], &right);
        self.call_builtin_alloc(prototype);
        let result = self.alloc_stack(TicketType::Reference);
        // mov [rbp+{}],rax
        self.emit_with_stack(&[0x48, 0x89, 0x85], &result);
        // add rax,24
        self.emit(&[0x48, 0x83, 0xC0, 0x18]);

        // mov rsi,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0xB5], &left);
        let source_element = if let ValueType::ListValueType(l) = expr.left.get_type() {
            &*l.element_type
        } else {
            panic!()
        };
        self.emit_list_add_half(source_element, target_element);

        // mov rsi,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0xB5], &right);
        let source_element = if let ValueType::ListValueType(l) = expr.right.get_type() {
            &*l.element_type
        } else {
            panic!()
        };
        self.emit_list_add_half(source_element, target_element);

        // mov rax,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0x85], &left);
        self.emit_drop();
        // mov rax,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0x85], &right);
        self.emit_drop();
        // mov rax,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0x85], &result);
        self.free_stack(result);
        self.free_stack(right);
        self.free_stack(left);
    }

    pub fn emit_str_compare(&mut self, expr: &BinaryExpr) {
        self.emit_expression(&expr.left);
        let left = self.alloc_stack(TicketType::Reference);
        // mov [rbp+{}],rax
        self.emit_with_stack(&[0x48, 0x89, 0x85], &left);
        self.emit_expression(&expr.right);
        // mov r11,[rbp+{}]
        self.emit_with_stack(&[0x4C, 0x8B, 0x9D], &left);

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

        let result = self.alloc_stack(TicketType::Plain);
        // mov [rbp+{}],rdx
        self.emit_with_stack(&[0x48, 0x89, 0x95], &result);
        self.emit_drop();
        // mov rax,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0x85], &left);
        self.emit_drop();
        // mov rax,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0x85], &result);
        self.free_stack(result);
        self.free_stack(left);
    }

    pub fn emit_binary_expr(&mut self, expr: &BinaryExpr, target_type: &ValueType) {
        let left_type = expr.left.get_type();
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
            let skip = self.jump_from();
            self.emit_expression(&expr.right);
            self.to_here(skip);
        } else {
            self.emit_expression(&expr.left);
            let left = self.alloc_stack(TicketType::Unknown);
            // mov [rbp+{}],rax
            self.emit_with_stack(&[0x48, 0x89, 0x85], &left);
            self.emit_expression(&expr.right);
            // mov r11,[rbp+{}]
            self.emit_with_stack(&[0x4C, 0x8B, 0x9D], &left);
            self.free_stack(left);

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
                    let ok = self.jump_from();
                    self.prepare_call(self.platform.stack_reserve());
                    self.call(BUILTIN_DIV_ZERO);
                    self.to_here(ok);
                    // xchg eax,r11d
                    self.emit(&[0x41, 0x93]);
                    // mov ecx,r11d
                    self.emit(&[0x44, 0x89, 0xD9]);
                    // xor ecx,eax
                    self.emit(&[0x31, 0xC1]);
                    // shr ecx,31
                    self.emit(&[0xC1, 0xE9, 0x1F]);
                    // cdq
                    self.emit(&[0x99]);
                    // idiv,r11d
                    self.emit(&[0x41, 0xF7, 0xFB]);
                    if expr.operator == BinaryOp::Mod {
                        // mov eax,edx
                        self.emit(&[0x89, 0xD0]);
                        // test edx,edx
                        self.emit(&[0x85, 0xD2]);
                        // cmove r11d,edx
                        self.emit(&[0x44, 0x0F, 0x44, 0xDA]);
                        // test ecx,ecx
                        self.emit(&[0x85, 0xC9]);
                        // cmove r11d,ecx
                        self.emit(&[0x44, 0x0F, 0x44, 0xD9]);
                        // add eax,r11d
                        self.emit(&[0x44, 0x01, 0xD8]);
                    } else {
                        // test edx,edx
                        self.emit(&[0x85, 0xD2]);
                        // cmove ecx,edx
                        self.emit(&[0x0F, 0x44, 0xCA]);
                        // sub eax,ecx
                        self.emit(&[0x29, 0xC8]);
                    }
                }
                BinaryOp::Is => {
                    // cmp r11,rax
                    self.emit(&[0x49, 0x39, 0xC3]);
                    // sete cl
                    self.emit(&[0x0F, 0x94, 0xC1]);
                    let result = self.alloc_stack(TicketType::Plain);
                    let left = self.alloc_stack(TicketType::Reference);
                    // mov [rbp+{}],rcx
                    self.emit_with_stack(&[0x48, 0x89, 0x8D], &result);
                    // mov [rbp+{}],r11
                    self.emit_with_stack(&[0x4C, 0x89, 0x9D], &left);
                    self.emit_drop();
                    // mov rax,[rbp+{}]
                    self.emit_with_stack(&[0x48, 0x8B, 0x85], &left);
                    self.emit_drop();
                    // mov rax,[rbp+{}]
                    self.emit_with_stack(&[0x48, 0x8B, 0x85], &result);
                    self.free_stack(left);
                    self.free_stack(result);
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
        func_type: &Option<FuncType>,
        name: &str,
        virtual_call: bool,
    ) {
        self.prepare_call(args.len());

        let mut args_stack = vec![];

        for (i, arg) in args.iter().enumerate() {
            self.emit_expression(arg);

            let param_type = &func_type.as_ref().unwrap().parameters[i];

            self.emit_coerce(arg.get_type(), param_type);

            if i == 0 && virtual_call {
                self.emit_check_none();
            }

            let arg_stack = self.alloc_stack(TicketType::Unknown);
            // mov [rbp+{}],rax
            self.emit_with_stack(&[0x48, 0x89, 0x85], &arg_stack);
            args_stack.push(arg_stack);
        }

        for (i, arg_stack) in args_stack.into_iter().enumerate().rev() {
            // mov rax,[rbp+{}]
            self.emit_with_stack(&[0x48, 0x8B, 0x85], &arg_stack);
            let offset = i * 8;
            // mov QWORD PTR [rsp+{offset}],rax
            self.emit(&[0x48, 0x89, 0x84, 0x24]);
            self.emit(&(offset as u32).to_le_bytes());
            self.free_stack(arg_stack);
        }

        if virtual_call {
            let offset = if let ValueType::ClassValueType(c) = args[0].get_type() {
                if matches!(
                    c.class_name.as_str(),
                    "int" | "bool" | "str" | "<None>" | "<Empty>"
                ) {
                    assert!(name == "__init__");
                    16
                } else {
                    self.classes()[&c.class_name].methods[name].offset
                }
            } else {
                panic!()
            };
            self.call_virtual(offset);
        } else {
            let slot = if let Some(EnvSlot::Func(f)) = self.storage_env().get(name) {
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
        let list = self.alloc_stack(TicketType::Reference);
        // mov [rbp+{}],rax
        self.emit_with_stack(&[0x48, 0x89, 0x85], &list);

        self.emit_expression(&expr.index);
        // cdqe
        self.emit(&[0x48, 0x98]);
        let index = self.alloc_stack(TicketType::Plain);
        // mov [rbp+{}],rax
        self.emit_with_stack(&[0x48, 0x89, 0x85], &index);
        // mov rsi,1
        self.emit(&[0x48, 0xc7, 0xc6, 0x01, 0x00, 0x00, 0x00]);
        self.call_builtin_alloc(STR_PROTOTYPE);
        // mov rsi,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0xB5], &index);
        self.free_stack(index);
        // mov r11,[rbp+{}]
        self.emit_with_stack(&[0x4C, 0x8B, 0x9D], &list);
        self.free_stack(list);
        // cmp rsi,[r11+16]
        self.emit(&[0x49, 0x3B, 0x73, 0x10]);
        // jb
        self.emit(&[0x0F, 0x82]);
        let ok = self.jump_from();
        self.prepare_call(self.platform.stack_reserve());
        self.call(BUILTIN_OUT_OF_BOUND);
        self.to_here(ok);
        // mov r10b,[r11+rsi+24]
        self.emit(&[0x45, 0x8A, 0x54, 0x33, 0x18]);
        // mov [rax+24],r10b
        self.emit(&[0x44, 0x88, 0x50, 0x18]);
        let result = self.alloc_stack(TicketType::Reference);
        // mov [rbp+{}],rax
        self.emit_with_stack(&[0x48, 0x89, 0x85], &result);
        // mov rax,r11
        self.emit(&[0x4C, 0x89, 0xD8]);
        self.emit_drop();
        // mov rax,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0x85], &result);
        self.free_stack(result);
    }

    pub fn emit_list_index(&mut self, expr: &IndexExpr) {
        self.emit_expression(&expr.list);
        self.emit_check_none();
        let list = self.alloc_stack(TicketType::Reference);
        // mov [rbp+{}],rax
        self.emit_with_stack(&[0x48, 0x89, 0x85], &list);
        self.emit_expression(&expr.index);
        // cdqe
        self.emit(&[0x48, 0x98]);
        // mov rsi,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0xB5], &list);
        self.free_stack(list);
        let element_type = if let ValueType::ListValueType(l) = expr.list.get_type() {
            &*l.element_type
        } else {
            panic!()
        };

        // cmp rax,[rsi+16]
        self.emit(&[0x48, 0x3B, 0x46, 0x10]);
        // jb
        self.emit(&[0x0F, 0x82]);
        let ok = self.jump_from();
        self.prepare_call(self.platform.stack_reserve());
        self.call(BUILTIN_OUT_OF_BOUND);
        self.to_here(ok);

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

        let result = self.alloc_stack(TicketType::Unknown);
        // mov [rbp+{}],rax
        self.emit_with_stack(&[0x48, 0x89, 0x85], &result);
        // mov rax,rsi
        self.emit(&[0x48, 0x89, 0xF0]);
        self.emit_drop();
        // mov rax,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0x85], &result);
        self.free_stack(result);
    }

    pub fn emit_member_expr(&mut self, expr: &MemberExpr) {
        self.emit_expression(&expr.object);
        self.emit_check_none();
        // mov rsi,rax
        self.emit(&[0x48, 0x89, 0xC6]);

        let slot = if let ValueType::ClassValueType(c) = expr.object.get_type() {
            &self.classes()[&c.class_name].attributes[&expr.member.name]
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

        let result = self.alloc_stack(TicketType::Unknown);
        // mov [rbp+{}],rax
        self.emit_with_stack(&[0x48, 0x89, 0x85], &result);
        // mov rax,rsi
        self.emit(&[0x48, 0x89, 0xF0]);
        self.emit_drop();
        // mov rax,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0x85], &result);
        self.free_stack(result);
    }

    pub fn emit_if_expr(&mut self, expr: &IfExpr, target_type: &ValueType) {
        self.emit_expression(&expr.condition);
        // test al,al
        self.emit(&[0x84, 0xC0]);
        // je
        self.emit(&[0x0f, 0x84]);
        let label_else = self.jump_from();

        self.emit_expression(&expr.then_expr);
        self.emit_coerce(&expr.then_expr.get_type(), target_type);

        // jmp
        self.emit(&[0xe9]);
        let label_end = self.jump_from();
        self.to_here(label_else);

        self.emit_expression(&expr.else_expr);
        self.emit_coerce(&expr.else_expr.get_type(), target_type);

        self.to_here(label_end);
    }

    pub fn emit_if_stmt(&mut self, stmt: &IfStmt, lines: &mut Vec<LineMap>) {
        self.emit_expression(&stmt.condition);
        // test al,al
        self.emit(&[0x84, 0xC0]);
        // je
        self.emit(&[0x0f, 0x84]);
        let label_else = self.jump_from();

        for stmt in &stmt.then_body {
            self.emit_statement(stmt, lines);
        }

        // jmp
        self.emit(&[0xe9]);
        let label_end = self.jump_from();
        self.to_here(label_else);

        for stmt in &stmt.else_body {
            self.emit_statement(stmt, lines);
        }

        self.to_here(label_end);
    }

    pub fn emit_list_expr(&mut self, expr: &ListExpr, target_type: &ValueType) {
        if target_type == &*TYPE_EMPTY {
            // mov rsi,{len}
            self.emit(&[0x48, 0xc7, 0xc6]);
            self.emit(&(expr.elements.len() as u32).to_le_bytes());
            self.call_builtin_alloc(OBJECT_LIST_PROTOTYPE);
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

        // mov rsi,{len}
        self.emit(&[0x48, 0xc7, 0xc6]);
        self.emit(&(expr.elements.len() as u32).to_le_bytes());
        self.call_builtin_alloc(prototype);
        let result = self.alloc_stack(TicketType::Reference);
        // mov [rbp+{}],rax
        self.emit_with_stack(&[0x48, 0x89, 0x85], &result);

        for (i, element) in expr.elements.iter().enumerate() {
            self.emit_expression(element);
            self.emit_coerce(element.get_type(), element_type);
            // mov rdi,[rbp+{}]
            self.emit_with_stack(&[0x48, 0x8B, 0xBD], &result);
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

        // mov rax,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0x85], &result);
        self.free_stack(result);
    }

    pub fn emit_load_var(&mut self, identifier: &Variable, target_type: &ValueType) {
        let (offset, level) =
            if let Some(EnvSlot::Var(v, _)) = self.storage_env().get(&identifier.name) {
                (v.offset, v.level)
            } else {
                panic!()
            };

        if level == 0 {
            if target_type == &*TYPE_INT {
                // mov eax,[rip+{}]
                self.emit(&[0x8B, 0x05]);
                self.emit_link(GLOBAL_SECTION, offset);
            } else if target_type == &*TYPE_BOOL {
                // mov al,[rip+{}]
                self.emit(&[0x8A, 0x05]);
                self.emit_link(GLOBAL_SECTION, offset);
            } else {
                // mov rax,[rip+{}]
                self.emit(&[0x48, 0x8B, 0x05]);
                self.emit_link(GLOBAL_SECTION, offset);
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
            ExprContent::Variable(identifier) => {
                self.emit_load_var(identifier, expression.get_type());
            }
            ExprContent::NoneLiteral(_) => {
                self.emit_none_literal();
            }
            ExprContent::IntegerLiteral(i) => {
                self.emit_int_literal(i.value);
            }
            ExprContent::BooleanLiteral(b) => {
                self.emit_bool_literal(b.value);
            }
            ExprContent::StringLiteral(s) => {
                self.emit_string_literal(&s.value);
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
                self.emit_binary_expr(expr, expression.get_type());
            }
            ExprContent::CallExpr(expr) => {
                self.emit_call_expr(
                    &expr.args,
                    &expr.function.inferred_type,
                    &expr.function.name,
                    false,
                );
            }
            ExprContent::MethodCallExpr(expr) => {
                let method = &expr.method;
                let args: Vec<Expr> = std::iter::once(method.object.clone())
                    .chain(expr.args.iter().cloned())
                    .collect();
                self.emit_call_expr(&args, &method.inferred_type, &method.member.name, true);
            }
            ExprContent::IndexExpr(expr) => {
                if expr.list.get_type() == &*TYPE_STR {
                    self.emit_str_index(&*expr);
                } else {
                    self.emit_list_index(&*expr);
                }
            }
            ExprContent::IfExpr(expr) => self.emit_if_expr(expr, expression.get_type()),
            ExprContent::ListExpr(expr) => {
                self.emit_list_expr(expr, expression.get_type());
            }
            ExprContent::MemberExpr(expr) => {
                self.emit_member_expr(expr);
            }
        }
    }

    pub fn emit_while_stmt(&mut self, stmt: &WhileStmt, lines: &mut Vec<LineMap>) {
        let start = self.jump_to();
        self.emit_expression(&stmt.condition);
        // test al,al
        self.emit(&[0x84, 0xC0]);
        // je
        self.emit(&[0x0f, 0x84]);
        let end = self.jump_from();

        for stmt in &stmt.body {
            self.emit_statement(stmt, lines);
        }

        // jmp
        self.emit(&[0xe9]);
        self.from_here(start);
        self.to_here(end);
    }

    pub fn emit_assign_identifier(
        &mut self,
        name: &str,
        source_type: &ValueType,
        target_type: &ValueType,
    ) {
        // rax: value to assign

        let (offset, level) = if let Some(EnvSlot::Var(v, _)) = self.storage_env().get(name) {
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
                let value = self.alloc_stack(TicketType::Reference);
                // mov [rbp+{}],rax
                self.emit_with_stack(&[0x48, 0x89, 0x85], &value);
                // mov rax,[rip+{}]
                self.emit(&[0x48, 0x8B, 0x05]);
                self.emit_link(GLOBAL_SECTION, offset);
                self.emit_drop();
                // mov rax,[rbp+{}]
                self.emit_with_stack(&[0x48, 0x8B, 0x85], &value);
                self.free_stack(value);
                // mov [rip+{}],rax
                self.emit(&[0x48, 0x89, 0x05]);
            }
            self.emit_link(GLOBAL_SECTION, offset);
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
                let dest = self.alloc_stack(TicketType::Plain);
                let value = self.alloc_stack(TicketType::Reference);
                // mov [rbp+{}],rdi
                self.emit_with_stack(&[0x48, 0x89, 0xBD], &dest);
                // mov [rbp+{}],rax
                self.emit_with_stack(&[0x48, 0x89, 0x85], &value);
                // mov rax,[rdi]
                self.emit(&[0x48, 0x8B, 0x07]);
                self.emit_drop();
                // mov rax,[rbp+{}]
                self.emit_with_stack(&[0x48, 0x8B, 0x85], &value);
                // mov rdi,[rbp+{}]
                self.emit_with_stack(&[0x48, 0x8B, 0xBD], &dest);
                self.free_stack(value);
                self.free_stack(dest);
            }
            // mov [rdi],rax
            self.emit(&[0x48, 0x89, 0x07]);
        }
    }

    pub fn emit_assign(&mut self, stmt: &AssignStmt) {
        let source_type = stmt.value.get_type();
        self.emit_expression(&stmt.value);
        let value = self.alloc_stack(TicketType::Unknown);
        // mov [rbp+{}],rax
        self.emit_with_stack(&[0x48, 0x89, 0x85], &value);

        for target in &stmt.targets {
            let target_type = target.get_type();
            match &target.content {
                ExprContent::Variable(identifier) => {
                    // mov rax,[rbp+{}]
                    self.emit_with_stack(&[0x48, 0x8B, 0x85], &value);
                    if source_type != &*TYPE_INT && source_type != &*TYPE_BOOL {
                        self.emit_clone();
                    }

                    self.emit_assign_identifier(&identifier.name, source_type, target_type);
                }
                ExprContent::IndexExpr(expr) => {
                    self.emit_expression(&expr.list);
                    self.emit_check_none();
                    let list = self.alloc_stack(TicketType::Reference);
                    // mov [rbp+{}],rax
                    self.emit_with_stack(&[0x48, 0x89, 0x85], &list);
                    self.emit_expression(&expr.index);
                    // mov rsi,[rbp+{}]
                    self.emit_with_stack(&[0x48, 0x8B, 0xB5], &list);

                    // cmp rax,[rsi+16]
                    self.emit(&[0x48, 0x3B, 0x46, 0x10]);
                    // jb
                    self.emit(&[0x0F, 0x82]);
                    let ok = self.jump_from();
                    self.prepare_call(self.platform.stack_reserve());
                    self.call(BUILTIN_OUT_OF_BOUND);
                    self.to_here(ok);

                    let dest = self.alloc_stack(TicketType::Plain);
                    if target_type == &*TYPE_INT {
                        // lea rsi,[rsi+rax*4+24]
                        self.emit(&[0x48, 0x8D, 0x74, 0x86, 0x18]);
                        // mov [rbp+{}],rsi
                        self.emit_with_stack(&[0x48, 0x89, 0xB5], &dest);
                    } else if target_type == &*TYPE_BOOL {
                        // lea rsi,[rsi+rax+24]
                        self.emit(&[0x48, 0x8D, 0x74, 0x06, 0x18]);
                        // mov [rbp+{}],rsi
                        self.emit_with_stack(&[0x48, 0x89, 0xB5], &dest);
                    } else {
                        // lea rsi,[rsi+rax*8+24]
                        self.emit(&[0x48, 0x8D, 0x74, 0xC6, 0x18]);
                        // mov rax,[rsi]
                        self.emit(&[0x48, 0x8B, 0x06]);
                        // mov [rbp+{}],rsi
                        self.emit_with_stack(&[0x48, 0x89, 0xB5], &dest);
                        self.emit_drop();
                    }

                    // mov rax,[rbp+{}]
                    self.emit_with_stack(&[0x48, 0x8B, 0x85], &value);
                    if source_type != &*TYPE_INT && source_type != &*TYPE_BOOL {
                        self.emit_clone();
                    }
                    self.emit_coerce(source_type, target_type);
                    // mov rsi,[rbp+{}]
                    self.emit_with_stack(&[0x48, 0x8B, 0xB5], &dest);
                    self.free_stack(dest);

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

                    // mov rax,[rbp+{}]
                    self.emit_with_stack(&[0x48, 0x8B, 0x85], &list);
                    self.emit_drop();
                    self.free_stack(list);
                }
                ExprContent::MemberExpr(expr) => {
                    self.emit_expression(&expr.object);
                    self.emit_check_none();
                    let object = self.alloc_stack(TicketType::Reference);
                    // mov [rbp+{}],rax
                    self.emit_with_stack(&[0x48, 0x89, 0x85], &object);

                    let slot = if let ValueType::ClassValueType(c) = expr.object.get_type() {
                        &self.classes()[&c.class_name].attributes[&expr.member.name]
                    } else {
                        panic!()
                    };

                    if slot.target_type != *TYPE_INT && slot.target_type != *TYPE_BOOL {
                        // mov rax,[rax+{}]
                        self.emit(&[0x48, 0x8B, 0x80]);
                        self.emit(&slot.offset.to_le_bytes());
                        self.emit_drop();
                    }

                    // mov rax,[rbp+{}]
                    self.emit_with_stack(&[0x48, 0x8B, 0x85], &value);
                    if source_type != &*TYPE_INT && source_type != &*TYPE_BOOL {
                        self.emit_clone();
                    }
                    self.emit_coerce(source_type, &slot.target_type);

                    // mov rsi,[rbp+{}]
                    self.emit_with_stack(&[0x48, 0x8B, 0xB5], &object);
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

                    // mov rax,[rbp+{}]
                    self.emit_with_stack(&[0x48, 0x8B, 0x85], &object);
                    self.emit_drop();
                    self.free_stack(object);
                }
                _ => panic!(),
            }
        }

        if source_type != &*TYPE_INT && source_type != &*TYPE_BOOL {
            // mov rax,[rbp+{}]
            self.emit_with_stack(&[0x48, 0x8B, 0x85], &value);
            self.emit_drop();
        }
        self.free_stack(value);
    }

    #[allow(clippy::useless_let_if_seq)] // Tell me which is more readable
    pub fn emit_for_stmt(&mut self, stmt: &ForStmt, lines: &mut Vec<LineMap>) {
        //// Compute the iterable
        self.emit_expression(&stmt.iterable);
        self.emit_check_none();
        let list = self.alloc_stack(TicketType::Reference);
        // mov [rbp+{}],rax
        self.emit_with_stack(&[0x48, 0x89, 0x85], &list);
        // xor rax,rax
        self.emit(&[0x48, 0x31, 0xC0]);

        let start = self.jump_to();
        //// Check the index range
        // mov rsi,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0xB5], &list);
        // cmp rax,[rsi+16]
        self.emit(&[0x48, 0x3B, 0x46, 0x10]);
        // je
        self.emit(&[0x0f, 0x84]);
        let end = self.jump_from();

        let counter = self.alloc_stack(TicketType::Plain);
        // mov [rbp+{}],rax
        self.emit_with_stack(&[0x48, 0x89, 0x85], &counter);

        //// Compute the element
        let iterable_type = stmt.iterable.get_type();
        let source_type;
        if iterable_type == &*TYPE_STR {
            // mov rsi,1
            self.emit(&[0x48, 0xc7, 0xc6, 0x01, 0x00, 0x00, 0x00]);
            self.call_builtin_alloc(STR_PROTOTYPE);
            // mov rsi,[rbp+{}]
            self.emit_with_stack(&[0x48, 0x8B, 0xB5], &counter);
            // mov r11,[rbp+{}]
            self.emit_with_stack(&[0x4C, 0x8B, 0x9D], &list);
            // mov r10b,[r11+rsi+24]
            self.emit(&[0x45, 0x8A, 0x54, 0x33, 0x18]);
            // mov [rax+24],r10b
            self.emit(&[0x44, 0x88, 0x50, 0x18]);

            source_type = &*TYPE_STR;
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

            source_type = element_type;
        }

        //// Assign the element
        let target_type = stmt.identifier.get_type();
        self.emit_assign_identifier(&stmt.identifier.name, source_type, target_type);

        //// Execute the loop body
        for stmt in &stmt.body {
            self.emit_statement(stmt, lines);
        }

        //// Increase the index and loop back
        // mov rax,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0x85], &counter);
        // inc rax
        self.emit(&[0x48, 0xFF, 0xC0]);
        // jmp
        self.emit(&[0xe9]);
        self.from_here(start);
        self.to_here(end);

        // mov rax,[rbp+{}]
        self.emit_with_stack(&[0x48, 0x8B, 0x85], &list);
        self.emit_drop();
        self.free_stack(counter);
        self.free_stack(list);
    }

    pub fn emit_statement(&mut self, statement: &Stmt, lines: &mut Vec<LineMap>) {
        lines.push(LineMap {
            code_pos: self.pos(),
            line_number: statement.base().location.start.row,
        });
        match statement {
            Stmt::ExprStmt(e) => {
                self.emit_expression(&e.expr);
                if e.expr.get_type() != &*TYPE_INT && e.expr.get_type() != &*TYPE_BOOL {
                    self.emit_drop();
                }
            }
            Stmt::AssignStmt(stmt) => {
                self.emit_assign(stmt);
            }
            Stmt::IfStmt(stmt) => {
                self.emit_if_stmt(stmt, lines);
            }
            Stmt::WhileStmt(stmt) => {
                self.emit_while_stmt(stmt, lines);
            }
            Stmt::ForStmt(stmt) => {
                self.emit_for_stmt(stmt, lines);
            }
            Stmt::ReturnStmt(stmt) => {
                if let Some(value) = &stmt.value {
                    self.emit_expression(value);
                    self.emit_coerce(value.get_type(), self.return_type.as_ref().unwrap());
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
                self.emit_int_literal(i.value);
            }
            LiteralContent::BooleanLiteral(b) => {
                self.emit_bool_literal(b.value);
            }
            LiteralContent::StringLiteral(s) => {
                self.emit_string_literal(&s.value);
            }
        }

        let target_type = ValueType::from_annotation(&decl.var.type_);
        self.emit_coerce(decl.value.get_type(), &target_type);
        let local = self.alloc_stack(if target_type == *TYPE_INT || target_type == *TYPE_BOOL {
            TicketType::Plain
        } else {
            TicketType::Reference
        });
        // mov [rbp+{}],rax
        self.emit_with_stack(&[0x48, 0x89, 0x85], &local);
        std::mem::forget(local);
    }

    pub fn emit_global_var_init(&mut self, decl: &VarDef) {
        let offset =
            if let Some(EnvSlot::Var(v, _)) = self.storage_env().get(&decl.var.identifier.name) {
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
                self.emit_int_literal(i.value);
            }
            LiteralContent::BooleanLiteral(b) => {
                self.emit_bool_literal(b.value);
            }
            LiteralContent::StringLiteral(s) => {
                self.emit_string_literal(&s.value);
            }
        }

        let target_type = ValueType::from_annotation(&decl.var.type_);
        self.emit_coerce(decl.value.get_type(), &target_type);

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
        self.emit_link(GLOBAL_SECTION, offset);
    }

    pub fn emit_global_var_drop(&mut self, decl: &VarDef) {
        let offset =
            if let Some(EnvSlot::Var(v, _)) = self.storage_env().get(&decl.var.identifier.name) {
                assert!(v.level == 0);
                v.offset
            } else {
                panic!()
            };

        let target_type = ValueType::from_annotation(&decl.var.type_);
        if target_type != *TYPE_INT && target_type != *TYPE_BOOL {
            // mov rax,[rip+{}]
            self.emit(&[0x48, 0x8B, 0x05]);
            self.emit_link(GLOBAL_SECTION, offset);
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
    platform: Platform,
) -> Vec<Chunk> {
    let link_name = if let Some(parent) = parent {
        parent.to_owned() + "." + &function.name.name
    } else {
        function.name.name.clone()
    };

    let mut locals = HashMap::new();
    let mut clean_up_list = vec![];

    let mut params_debug = vec![];

    for (i, param) in function.params.iter().enumerate() {
        let offset;
        offset = i as i32 * 8 + 16;
        let name = &param.identifier.name;
        locals.insert(
            name.clone(),
            LocalSlot::Var(VarSlot {
                offset,
                level: level + 1,
            }),
        );
        let param_type = ValueType::from_annotation(&param.type_);
        if param_type != *TYPE_INT && param_type != *TYPE_BOOL {
            clean_up_list.push(offset);
        }

        params_debug.push(VarDebug {
            offset,
            line: param.base().location.start.row,
            name: name.clone(),
            var_type: TypeDebug::from_annotation(&param.type_),
        })
    }

    let mut locals_debug = vec![];

    let mut local_offset = if level == 0 { -8 } else { -16 };

    for declaration in &function.declarations {
        match declaration {
            Declaration::VarDef(v) => {
                let name = &v.var.identifier.name;
                let offset = local_offset;
                local_offset -= 8;
                locals.insert(
                    name.clone(),
                    LocalSlot::Var(VarSlot {
                        offset,
                        level: level + 1,
                    }),
                );

                locals_debug.push(VarDebug {
                    offset,
                    line: v.base().location.start.row,
                    name: name.clone(),
                    var_type: TypeDebug::from_annotation(&v.var.type_),
                })
            }
            Declaration::FuncDef(f) => {
                let name = &f.name.name;
                locals.insert(
                    name.clone(),
                    LocalSlot::Func(FuncSlot {
                        link_name: link_name.clone() + "." + name,
                        level: level + 1,
                    }),
                );
            }
            _ => (),
        }
    }

    let mut handle = storage_env.push(locals);
    let return_type = ValueType::from_annotation(&function.return_type);

    let mut code = Emitter::new(
        &link_name,
        Some(&return_type),
        Some(handle.inner()),
        Some(classes),
        clean_up_list,
        level,
        platform,
    );

    if level != 0 {
        let static_link = code.alloc_stack(TicketType::Plain);
        // mov [rbp+{}],r10
        code.emit_with_stack(&[0x4C, 0x89, 0x95], &static_link);
        std::mem::forget(static_link);
    }

    for declaration in &function.declarations {
        if let Declaration::VarDef(v) = declaration {
            code.emit_local_var_init(v);
        }
    }

    let mut lines = vec![LineMap {
        code_pos: 0,
        line_number: function.base().location.start.row,
    }];

    for statement in &function.statements {
        code.emit_statement(statement, &mut lines);
    }

    code.emit_none_literal();
    code.end_proc();

    let mut chunks = vec![code.finalize(ProcedureDebug {
        decl_line: function.statements[0].base().location.start.row,
        artificial: false,
        parent: if level == 0 {
            None
        } else {
            parent.map(str::to_owned)
        },
        lines,
        return_type: TypeDebug::from_annotation(&function.return_type),
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
                platform,
            ));
        }
    }

    chunks
}

fn gen_ctor(class_name: &str, class_slot: &ClassSlot, platform: Platform) -> Chunk {
    let mut code = Emitter::new(class_name, None, None, None, vec![], 0, platform);

    code.prepare_call(platform.stack_reserve());
    match platform {
        Platform::Windows => {
            // xor rdx,rdx
            code.emit(&[0x48, 0x31, 0xD2]);
            // lea rcx,[rip+{}]
            code.emit(&[0x48, 0x8D, 0x0D]);
        }
        Platform::Linux | Platform::Macos => {
            // xor rsi,rsi
            code.emit(&[0x48, 0x31, 0xF6]);
            // lea rdi,[rip+{}]
            code.emit(&[0x48, 0x8D, 0x3D]);
        }
    }
    code.emit_link(class_name.to_owned() + ".$proto", 0);

    code.call(BUILTIN_ALLOC_OBJ);
    let object = code.alloc_stack(TicketType::Reference);
    // mov [rbp+{}],rax
    code.emit_with_stack(&[0x48, 0x89, 0x85], &object);

    for attribute in class_slot.attributes.values() {
        match &attribute.init {
            LiteralContent::NoneLiteral(_) => {
                code.emit_none_literal();
            }
            LiteralContent::IntegerLiteral(i) => {
                code.emit_int_literal(i.value);
            }
            LiteralContent::BooleanLiteral(b) => {
                code.emit_bool_literal(b.value);
            }
            LiteralContent::StringLiteral(s) => {
                code.emit_string_literal(&s.value);
            }
        }

        code.emit_coerce(&attribute.source_type, &attribute.target_type);
        // mov rdi,[rbp+{}]
        code.emit_with_stack(&[0x48, 0x8B, 0xBD], &object);

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

    // mov rax,[rbp+{}]
    code.emit_with_stack(&[0x48, 0x8B, 0x85], &object);
    code.emit_clone();
    code.prepare_call(1);
    // mov [rsp],rax
    code.emit(&[0x48, 0x89, 0x04, 0x24]);
    code.call_virtual(16);

    // mov rax,[rbp+{}]
    code.emit_with_stack(&[0x48, 0x8B, 0x85], &object);
    code.free_stack(object);
    code.end_proc();
    code.finalize(ProcedureDebug {
        decl_line: 0,
        artificial: true,
        parent: None,
        lines: vec![],
        return_type: TypeDebug::class_type(class_name),
        params: vec![],
        locals: vec![],
    })
}

fn gen_dtor(class_name: &str, class_slot: &ClassSlot, platform: Platform) -> Chunk {
    let mut code = Emitter::new_simple(&(class_name.to_owned() + ".$dtor"), platform);
    // Note: This uses C ABI instead of chocopy ABI

    let mut rdi = None;
    let mut rsi = None;
    let object = code.alloc_stack(TicketType::Reference);

    match platform {
        Platform::Windows => {
            // mov [rbp+{}],rcx
            code.emit_with_stack(&[0x48, 0x89, 0x8D], &object);
            rdi = Some(code.alloc_stack(TicketType::Plain));
            rsi = Some(code.alloc_stack(TicketType::Plain));
            // mov [rbp+{}],rdi
            code.emit_with_stack(&[0x48, 0x89, 0xBD], &rdi.as_ref().unwrap());
            // mov [rbp+{}],rsi
            code.emit_with_stack(&[0x48, 0x89, 0xB5], &rsi.as_ref().unwrap());
        }
        Platform::Linux | Platform::Macos => {
            // mov [rbp+{}],rdi
            code.emit_with_stack(&[0x48, 0x89, 0xBD], &object);
        }
    }
    for attribute in class_slot.attributes.values() {
        if attribute.target_type != *TYPE_INT && attribute.target_type != *TYPE_BOOL {
            // mov rax,[rbp+{}]
            code.emit_with_stack(&[0x48, 0x8B, 0x85], &object);
            // mov rax,[rax+{}]
            code.emit(&[0x48, 0x8B, 0x80]);
            code.emit(&attribute.offset.to_le_bytes());
            code.emit_drop();
        }
    }
    match platform {
        Platform::Windows => {
            // mov rdi,[rbp+{}]
            code.emit_with_stack(&[0x48, 0x8B, 0xBD], &rdi.as_ref().unwrap());
            // mov rsi,[rbp+{}]
            code.emit_with_stack(&[0x48, 0x8B, 0xB5], &rsi.as_ref().unwrap());

            code.free_stack(rsi.unwrap());
            code.free_stack(rdi.unwrap())
        }
        Platform::Linux | Platform::Macos => (),
    }
    code.free_stack(object);
    code.end_proc();
    code.finalize(ProcedureDebug {
        decl_line: 0,
        artificial: true,
        parent: None,
        lines: vec![],
        return_type: TypeDebug::class_type("<None>"),
        params: vec![VarDebug {
            offset: -8,
            line: 0,
            name: "self".to_owned(),
            var_type: TypeDebug::class_type(class_name),
        }],
        locals: vec![],
    })
}

fn gen_int(platform: Platform) -> Chunk {
    let mut code = Emitter::new_simple("int", platform);
    code.emit_int_literal(0);
    code.end_proc();
    code.finalize(ProcedureDebug {
        decl_line: 0,
        artificial: true,
        parent: None,
        lines: vec![],
        return_type: TypeDebug::class_type("int"),
        params: vec![],
        locals: vec![],
    })
}

fn gen_bool(platform: Platform) -> Chunk {
    let mut code = Emitter::new_simple("bool", platform);
    code.emit_bool_literal(false);
    code.end_proc();
    code.finalize(ProcedureDebug {
        decl_line: 0,
        artificial: true,
        parent: None,
        lines: vec![],
        return_type: TypeDebug::class_type("bool"),
        params: vec![],
        locals: vec![],
    })
}

fn gen_str(platform: Platform) -> Chunk {
    let mut code = Emitter::new_simple("str", platform);
    code.emit_string_literal("");
    code.end_proc();
    code.finalize(ProcedureDebug {
        decl_line: 0,
        artificial: true,
        parent: None,
        lines: vec![],
        return_type: TypeDebug::class_type("str"),
        params: vec![],
        locals: vec![],
    })
}

fn gen_object_init(platform: Platform) -> Chunk {
    let mut code = Emitter::new_simple("object.__init__", platform);
    // mov rax,[rbp+16]
    code.emit(&[0x48, 0x8B, 0x45, 0x10]);
    code.emit_drop();
    code.emit_none_literal();
    code.end_proc();
    code.finalize(ProcedureDebug {
        decl_line: 0,
        artificial: true,
        parent: None,
        lines: vec![],
        return_type: TypeDebug::class_type("<None>"),
        params: vec![VarDebug {
            offset: 16,
            line: 0,
            name: "self".to_owned(),
            var_type: TypeDebug::class_type("object"),
        }],
        locals: vec![],
    })
}

fn gen_len(platform: Platform) -> Chunk {
    let mut code = Emitter::new_simple("len", platform);
    match platform {
        Platform::Windows => code.emit(&[0x48, 0x8B, 0x4D, 0x10]), //  mov rcx,[rbp+16]
        Platform::Linux | Platform::Macos => code.emit(&[0x48, 0x8B, 0x7D, 0x10]), // mov rdi,[rbp+16]
    }
    code.prepare_call(platform.stack_reserve());
    code.call(BUILTIN_LEN);
    code.end_proc();
    code.finalize(ProcedureDebug {
        decl_line: 0,
        artificial: true,
        parent: None,
        lines: vec![],
        return_type: TypeDebug::class_type("int"),
        params: vec![VarDebug {
            offset: 16,
            line: 0,
            name: "object".to_owned(),
            var_type: TypeDebug::class_type("object"),
        }],
        locals: vec![],
    })
}

fn gen_input(platform: Platform) -> Chunk {
    let mut code = Emitter::new_simple("input", platform);
    match platform {
        Platform::Windows => code.emit(&[0x48, 0x8D, 0x0D]), // lea rcx,[rip+{}]
        Platform::Linux | Platform::Macos => code.emit(&[0x48, 0x8D, 0x3D]), // lea rdi,[rip+{}]
    }
    code.emit_link(STR_PROTOTYPE, 0);
    code.prepare_call(platform.stack_reserve());
    code.call(BUILTIN_INPUT);
    code.end_proc();
    code.finalize(ProcedureDebug {
        decl_line: 0,
        artificial: true,
        parent: None,
        return_type: TypeDebug::class_type("str"),
        params: vec![],
        lines: vec![],
        locals: vec![],
    })
}

fn gen_print(platform: Platform) -> Chunk {
    let mut code = Emitter::new_simple("print", platform);
    match platform {
        Platform::Windows => code.emit(&[0x48, 0x8B, 0x4D, 0x10]), //  mov rcx,[rbp+16]
        Platform::Linux | Platform::Macos => code.emit(&[0x48, 0x8B, 0x7D, 0x10]), // mov rdi,[rbp+16]
    }
    code.prepare_call(platform.stack_reserve());
    code.call(BUILTIN_PRINT);
    code.end_proc();
    code.finalize(ProcedureDebug {
        decl_line: 0,
        artificial: true,
        parent: None,
        lines: vec![],
        return_type: TypeDebug::class_type("<None>"),
        params: vec![VarDebug {
            offset: 16,
            line: 0,
            name: "object".to_owned(),
            var_type: TypeDebug::class_type("object"),
        }],
        locals: vec![],
    })
}

fn gen_main(
    ast: &Program,
    storage_env: &mut StorageEnv,
    classes: &HashMap<String, ClassSlot>,
    platform: Platform,
) -> Chunk {
    let mut main_code = Emitter::new(
        BUILTIN_CHOCOPY_MAIN,
        None,
        Some(storage_env),
        Some(classes),
        vec![],
        0,
        platform,
    );

    /*// mov rax,0x12345678
    main_code.emit(&[0x48, 0xC7, 0xC0, 0x78, 0x56, 0x34, 0x12]);
    main_code.emit_push_rax();*/

    let mut rdi = None;
    let mut rsi = None;

    if platform == Platform::Windows {
        rdi = Some(main_code.alloc_stack(TicketType::Plain));
        rsi = Some(main_code.alloc_stack(TicketType::Plain));
        // mov [rbp+{}],rdi
        main_code.emit_with_stack(&[0x48, 0x89, 0xBD], &rdi.as_ref().unwrap());
        // mov [rbp+{}],rsi
        main_code.emit_with_stack(&[0x48, 0x89, 0xB5], &rsi.as_ref().unwrap());
    }

    for declaration in &ast.declarations {
        if let Declaration::VarDef(v) = declaration {
            main_code.emit_global_var_init(v);
        }
    }

    let mut lines = vec![];

    for statement in &ast.statements {
        main_code.emit_statement(statement, &mut lines);
    }

    for declaration in &ast.declarations {
        if let Declaration::VarDef(v) = declaration {
            main_code.emit_global_var_drop(v);
        }
    }

    if platform == Platform::Windows {
        // mov rdi,[rbp+{}]
        main_code.emit_with_stack(&[0x48, 0x8B, 0xBD], &rdi.as_ref().unwrap());
        // mov rsi,[rbp+{}]
        main_code.emit_with_stack(&[0x48, 0x8B, 0xB5], &rsi.as_ref().unwrap());

        main_code.free_stack(rsi.unwrap());
        main_code.free_stack(rdi.unwrap())
    }
    /*
    main_code.emit_pop_rax();
    // cmp rax,0x12345678
    main_code.emit(&[0x48, 0x3D, 0x78, 0x56, 0x34, 0x12]);

    // je
    main_code.emit(&[0x0f, 0x84]);
    let ok = main_code.jump_from();

    main_code.prepare_call(platform.stack_reserve());
    main_code.call(BUILTIN_BROKEN_STACK);

    main_code.to_here(ok);*/

    main_code.end_proc();

    main_code.finalize(ProcedureDebug {
        decl_line: ast
            .statements
            .get(0)
            .map_or(1, |s| s.base().location.start.row),
        artificial: false,
        parent: None,
        lines,
        return_type: TypeDebug::class_type("<None>"),
        params: vec![],
        locals: vec![],
    })
}

fn add_class(
    globals: &mut HashMap<String, LocalSlot<FuncSlot, VarSlot>>,
    classes: &mut HashMap<String, ClassSlot>,
    classes_debug: &mut HashMap<String, ClassDebug>,
    c: &ClassDef,
) {
    let class_name = &c.name.name;
    let super_name = &c.super_class.name;
    let mut class_slot = classes.get(super_name).unwrap().clone();
    let mut class_debug = classes_debug.get(super_name).unwrap().clone();
    class_slot.methods.get_mut("$dtor").unwrap().link_name = class_name.clone() + ".$dtor";
    globals.insert(
        class_name.clone(),
        LocalSlot::Func(FuncSlot {
            link_name: class_name.clone(),
            level: 0,
        }),
    );
    for declaration in &c.declarations {
        match declaration {
            Declaration::VarDef(v) => {
                let source_type = v.value.get_type().clone();
                let target_type = ValueType::from_annotation(&v.var.type_);
                let size = if target_type == *TYPE_INT {
                    4
                } else if target_type == *TYPE_BOOL {
                    1
                } else {
                    8
                };
                class_slot.object_size += (size - class_slot.object_size % size) % size;
                let offset = class_slot.object_size + 16;
                let name = &v.var.identifier.name;
                class_slot.attributes.insert(
                    name.clone(),
                    AttributeSlot {
                        offset,
                        source_type,
                        target_type,
                        init: v.value.content.clone(),
                    },
                );
                class_slot.object_size += size;

                class_debug.attributes.push(VarDebug {
                    offset: offset as i32,
                    line: v.base().location.start.row,
                    name: name.clone(),
                    var_type: TypeDebug::from_annotation(&v.var.type_),
                });
            }
            Declaration::FuncDef(f) => {
                let method_name = &f.name.name;
                let link_name = class_name.clone() + "." + method_name;
                if let Some(method) = class_slot.methods.get_mut(method_name) {
                    method.link_name = link_name;

                    let self_type = TypeDebug::from_annotation(&f.params[0].type_);
                    class_debug
                        .methods
                        .get_mut(&method.offset)
                        .unwrap()
                        .1
                        .params[0] = self_type;
                } else {
                    let offset = class_slot.prototype_size;
                    class_slot
                        .methods
                        .insert(method_name.clone(), MethodSlot { offset, link_name });
                    class_slot.prototype_size += 8;

                    let params = f
                        .params
                        .iter()
                        .map(|tv| TypeDebug::from_annotation(&tv.type_))
                        .collect();
                    let return_type = TypeDebug::from_annotation(&f.return_type);

                    class_debug.methods.insert(
                        offset,
                        (
                            method_name.clone(),
                            MethodDebug {
                                params,
                                return_type,
                            },
                        ),
                    );
                }
            }
            _ => panic!(),
        }
    }
    class_debug.size = class_slot.object_size;
    classes.insert(class_name.clone(), class_slot);
    classes_debug.insert(class_name.clone(), class_debug);
}

fn gen_special_proto(name: &str, size: i32, tag: i32, dtor: &str) -> Chunk {
    let mut code = vec![0; 24];
    code[0..4].copy_from_slice(&size.to_le_bytes());
    code[4..8].copy_from_slice(&tag.to_le_bytes());
    let links = vec![
        ChunkLink {
            pos: 8,
            to: ChunkLinkTarget::Symbol(dtor.to_owned()),
        },
        ChunkLink {
            pos: 16,
            to: ChunkLinkTarget::Symbol("object.__init__".to_owned()),
        },
    ];
    Chunk {
        name: name.to_owned(),
        code,
        links,
        extra: ChunkExtra::Data,
    }
}

pub(super) fn gen_code_set(ast: Program, platform: Platform) -> CodeSet {
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
    let mut classes_debug = HashMap::new();
    classes_debug.insert(
        "object".to_owned(),
        ClassDebug {
            size: 0,
            attributes: vec![],
            methods: std::iter::once((
                16,
                (
                    "__init__".to_owned(),
                    MethodDebug {
                        params: vec![TypeDebug::class_type("object")],
                        return_type: TypeDebug::class_type("<None>"),
                    },
                ),
            ))
            .collect(),
        },
    );
    for declaration in &ast.declarations {
        match declaration {
            Declaration::VarDef(v) => {
                let name = &v.var.identifier.name;
                let target_type = ValueType::from_annotation(&v.var.type_);
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

                globals_debug.push(VarDebug {
                    offset: global_offset,
                    line: v.base().location.start.row,
                    name: name.clone(),
                    var_type: TypeDebug::from_annotation(&v.var.type_),
                });

                global_offset += size;
            }
            Declaration::FuncDef(f) => {
                let name = &f.name.name;
                globals.insert(
                    name.clone(),
                    LocalSlot::Func(FuncSlot {
                        link_name: name.clone(),
                        level: 0,
                    }),
                );
            }
            Declaration::ClassDef(c) => {
                add_class(&mut globals, &mut classes, &mut classes_debug, c)
            }
            _ => panic!(),
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

    let mut chunks = vec![gen_main(&ast, &mut storage_env, &classes, platform)];

    for declaration in &ast.declarations {
        match declaration {
            Declaration::FuncDef(f) => {
                chunks.append(&mut gen_function(
                    &f,
                    &mut storage_env,
                    &classes,
                    0,
                    None,
                    platform,
                ));
            }
            Declaration::ClassDef(c) => {
                for declaration in &c.declarations {
                    if let Declaration::FuncDef(f) = declaration {
                        chunks.append(&mut gen_function(
                            &f,
                            &mut storage_env,
                            &classes,
                            0,
                            Some(&c.name.name),
                            platform,
                        ));
                    }
                }
            }
            _ => (),
        }
    }

    for (class_name, class_slot) in &classes {
        chunks.push(gen_ctor(&class_name, &class_slot, platform));
        chunks.push(gen_dtor(&class_name, &class_slot, platform));

        let mut prototype = vec![0; class_slot.prototype_size as usize];
        prototype[0..4].copy_from_slice(&class_slot.object_size.to_le_bytes());
        // prototype[4..8] is type tag fill with 0
        let links = class_slot
            .methods
            .iter()
            .map(|(_, method)| ChunkLink {
                pos: method.offset as usize,
                to: ChunkLinkTarget::Symbol(method.link_name.clone()),
            })
            .collect();
        chunks.push(Chunk {
            name: class_name.clone() + ".$proto",
            code: prototype,
            links,
            extra: ChunkExtra::Data,
        });
    }

    chunks.push(gen_int(platform));
    chunks.push(gen_bool(platform));
    chunks.push(gen_str(platform));
    chunks.push(gen_object_init(platform));
    chunks.push(gen_len(platform));
    chunks.push(gen_input(platform));
    chunks.push(gen_print(platform));

    chunks.push(gen_special_proto(INT_PROTOTYPE, 4, 1, "object.$dtor"));
    chunks.push(gen_special_proto(BOOL_PROTOTYPE, 1, 2, "object.$dtor"));
    chunks.push(gen_special_proto(STR_PROTOTYPE, -1, 3, "object.$dtor"));
    chunks.push(gen_special_proto(
        INT_LIST_PROTOTYPE,
        -4,
        -1,
        "object.$dtor",
    ));
    chunks.push(gen_special_proto(
        BOOL_LIST_PROTOTYPE,
        -1,
        -1,
        "object.$dtor",
    ));
    chunks.push(gen_special_proto(
        OBJECT_LIST_PROTOTYPE,
        -8,
        -1,
        "[object].$dtor",
    ));

    CodeSet {
        chunks,
        global_size: global_offset as u64,
        globals_debug,
        classes_debug,
    }
}
