#![allow(clippy::missing_safety_doc)]

use std::alloc::*;
use std::mem::*;
use std::process::exit;
use std::sync::atomic::*;

static ALLOC_COUNTER: AtomicU64 = AtomicU64::new(0);

#[repr(i32)]
pub enum TypeTag {
    Other = 0,
    Int = 1,
    Bool = 2,
    Str = 3,
    List = -1,
}

#[repr(C)]
pub struct Prototype {
    size: i32,
    tag: TypeTag,
    dtor: unsafe extern "C" fn(*mut Object),
    // followed by other method pointers
}

#[repr(C)]
pub struct Object {
    prototype: *const Prototype,
    ref_count: u64,
    // followed by attributes
}

#[repr(C)]
pub struct ArrayObject {
    object: Object,
    len: u64,
}

fn align_up(size: usize) -> usize {
    let unit = 8;
    let m = size % unit;
    if m == 0 {
        size
    } else {
        size + 8 - m
    }
}

#[export_name = "[object].$dtor"]
pub unsafe extern "C" fn dtor_list(pointer: *mut ArrayObject) {
    let len = (*pointer).len;
    let elements = pointer.offset(1) as *mut *mut Object;
    for i in 0..len {
        let element = *elements.offset(i as isize);
        if !element.is_null() {
            (*element).ref_count -= 1;
            if (*element).ref_count == 0 {
                free_obj(element);
            }
        }
    }
}

#[export_name = "$alloc_obj"]
pub unsafe extern "C" fn alloc_obj(prototype: *const Prototype, len: u64) -> *mut Object {
    let size = align_up(if (*prototype).size > 0 {
        assert!(len == 0);
        size_of::<Object>() + (*prototype).size as usize
    } else {
        size_of::<ArrayObject>() + (-(*prototype).size as u64 * len) as usize
    });
    let pointer = alloc(Layout::from_size_align(size, 8).unwrap()).cast::<Object>();
    if pointer.is_null() {
        out_of_memory();
    }
    (*pointer).prototype = prototype;
    (*pointer).ref_count = 1;
    if (*prototype).size < 0 {
        (*(pointer as *mut ArrayObject)).len = len;
    }
    ALLOC_COUNTER.fetch_add(1, Ordering::SeqCst);
    pointer
}

#[export_name = "$free_obj"]
pub unsafe extern "C" fn free_obj(pointer: *mut Object) {
    assert!((*pointer).ref_count == 0);
    let prototype = (*pointer).prototype;
    ((*prototype).dtor)(pointer);
    let size = align_up(if (*prototype).size > 0 {
        size_of::<Object>() + (*prototype).size as usize
    } else {
        let len = (*(pointer as *mut ArrayObject)).len;
        size_of::<ArrayObject>() + (-(*prototype).size as u64 * len) as usize
    });
    dealloc(
        pointer as *mut u8,
        Layout::from_size_align(size, 8).unwrap(),
    );
    ALLOC_COUNTER.fetch_sub(1, Ordering::SeqCst);
}

#[export_name = "$len"]
pub unsafe extern "C" fn len(pointer: *mut Object) -> i32 {
    if pointer.is_null() {
        invalid_arg();
    }
    let object = pointer as *mut ArrayObject;
    let prototype = (*object).object.prototype;
    if !matches!((*prototype).tag, TypeTag::Str | TypeTag::List) {
        invalid_arg();
    }
    let len = (*object).len as i32;
    (*object).object.ref_count -= 1;
    if (*object).object.ref_count == 0 {
        free_obj(pointer);
    }
    len
}

#[export_name = "$print"]
pub unsafe extern "C" fn print(pointer: *mut Object) -> *mut u8 {
    if pointer.is_null() {
        invalid_arg();
    }
    let prototype = (*pointer).prototype;
    match (*prototype).tag {
        TypeTag::Int => {
            println!("{}", *(pointer.offset(1) as *const i32));
        }
        TypeTag::Bool => {
            println!(
                "{}",
                if *(pointer.offset(1) as *const bool) {
                    "True"
                } else {
                    "False"
                }
            );
        }
        TypeTag::Str => {
            let object = pointer as *mut ArrayObject;
            let slice = std::str::from_utf8(std::slice::from_raw_parts(
                object.offset(1) as *const u8,
                (*object).len as usize,
            ))
            .unwrap();
            println!("{}", slice);
        }
        _ => {
            invalid_arg();
        }
    }

    (*pointer).ref_count -= 1;
    if (*pointer).ref_count == 0 {
        free_obj(pointer);
    }
    std::ptr::null_mut()
}

#[export_name = "$input"]
pub unsafe extern "C" fn input(str_proto: *const Prototype) -> *mut Object {
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    let input = input.as_bytes();
    let mut len = input.len();
    while len > 0 {
        if input[len - 1] != b'\n' && input[len - 1] != b'\r' {
            break;
        }
        len -= 1;
    }
    let len = len as u64;
    let pointer = alloc_obj(str_proto, len);
    std::ptr::copy_nonoverlapping(
        input.as_ptr(),
        (pointer as *mut u8).add(size_of::<ArrayObject>()),
        input.len(),
    );
    pointer
}

fn exit_code(code: i32) -> ! {
    println!("Exited with error code {}", code);
    exit(code);
}

#[export_name = "$broken_stack"]
pub extern "C" fn broken_stack() -> ! {
    println!("--- Broken stack detected! ---");
    exit_code(-2)
}

fn invalid_arg() -> ! {
    println!("Invalid argument");
    exit_code(1)
}

#[export_name = "$div_zero"]
pub extern "C" fn div_zero() -> ! {
    println!("Division by zero");
    exit_code(2)
}

#[export_name = "$out_of_bound"]
pub extern "C" fn out_of_bound() -> ! {
    println!("Index out of bounds");
    exit_code(3)
}

#[export_name = "$none_op"]
pub extern "C" fn none_op() -> ! {
    println!("Operation on None");
    exit_code(4)
}

fn out_of_memory() -> ! {
    println!("Out of memory");
    exit_code(5)
}

extern "C" {
    #[cfg(not(test))]
    #[link_name = "$chocopy_main"]
    fn chocopy_main();
}

#[no_mangle]
#[cfg(not(test))]
pub unsafe extern "C" fn main() -> i32 {
    chocopy_main();
    if ALLOC_COUNTER.load(Ordering::SeqCst) != 0 {
        println!("--- Memory leak detected! ---");
        exit_code(-1);
    }
    0
}
