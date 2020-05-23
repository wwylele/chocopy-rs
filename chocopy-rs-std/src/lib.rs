use chocopy_rs_common::*;
use std::cell::*;
use std::mem::*;
use std::process::exit;
use std::ptr::*;

mod gc;

#[repr(transparent)]
#[derive(Clone, Copy)]
struct AllocUnit(u64);

thread_local! {
    static INIT_PARAM: Cell<*const InitParam> = Cell::new(std::ptr::null());
    static GC_COUNTER: Cell<u64> = Cell::new(0);
    static GC_HEAD: Cell<Option<NonNull<Object>>> = Cell::new(None);
    static CURRENT_SPACE: Cell<usize> = Cell::new(0);
    static THRESHOLD_SPACE: Cell<usize> = Cell::new(1024);
}

pub fn divide_up(value: usize) -> usize {
    let align = size_of::<AllocUnit>();
    if value == 0 {
        0
    } else {
        1 + (value - 1) / align
    }
}

/// Allocates a ChocoPy object
///
/// # Safety
///  - `init` already called
///  - `prototype.size` is not 0.
///  - `prototype.tag` is Str or List if and only if `prototype.size < 0`.
///  - `prototype.map` points to a valid object reference map
///  - `rbp` and `rsp` points to the bottom and the top of the top stack frame
#[export_name = "$alloc_obj"]
pub unsafe extern "C" fn alloc_obj(
    prototype: *const Prototype,
    len: u64,
    rbp: *const u64,
    rsp: *const u64,
) -> *mut Object {
    if CURRENT_SPACE.with(|current_space| current_space.get())
        >= THRESHOLD_SPACE.with(|threshold_space| threshold_space.get())
    {
        gc::collect(rbp, rsp);
        let current = CURRENT_SPACE.with(|current_space| current_space.get());
        let threshold = std::cmp::max(1024, current * 2);
        THRESHOLD_SPACE.with(|threshold_space| threshold_space.set(threshold));
    }

    let size = divide_up(if (*prototype).size > 0 {
        assert!(len == 0);
        size_of::<Object>() + (*prototype).size as usize
    } else {
        size_of::<ArrayObject>() + (-(*prototype).size as u64 * len) as usize
    });

    let pointer =
        Box::into_raw(vec![AllocUnit(0); size].into_boxed_slice()) as *mut AllocUnit as *mut Object;

    CURRENT_SPACE.with(|current_space| current_space.set(current_space.get() + size));

    let gc_next = GC_HEAD.with(|gc_next| gc_next.replace(NonNull::new(pointer)));

    let object = Object {
        prototype,
        gc_count: GC_COUNTER.with(|gc_counter| gc_counter.get()),
        gc_next,
    };

    if (*prototype).size > 0 {
        pointer.write(object);
    } else {
        let object = ArrayObject { object, len };
        (pointer as *mut ArrayObject).write(object);
    }

    pointer
}

/// Gets the array length of a ChocoPy object
///
/// # Safety
///  - `init` already called
///  - `pointer` must be previously returned by `alloc_obj`.
///  - The `prototype` field must be intact.
///  - For `ArrayObject`, the `len` field must be intact.
#[export_name = "$len"]
pub unsafe extern "C" fn len(pointer: *mut Object) -> i32 {
    if pointer.is_null() {
        invalid_arg();
    }
    let object = pointer as *mut ArrayObject;
    let prototype = (*object).object.prototype;
    if !matches!(
        (*prototype).tag,
        TypeTag::Str | TypeTag::PlainList | TypeTag::RefList
    ) {
        invalid_arg();
    }
    (*object).len as i32
}

/// Prints a ChocoPy object
///
/// # Safety
///  - `init` already called
///  - `pointer` must be previously returned by `alloc_obj`.
///  - The `prototype` field must be intact.
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

    std::ptr::null_mut()
}

/// Creates a new str object that holds a line of user input
///
/// # Safety
///  - `init` already called
///  - `rbp` and `rsp` points to the bottom and the top of the top stack frame
#[export_name = "$input"]
pub unsafe extern "C" fn input(rbp: *const u64, rsp: *const u64) -> *mut Object {
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
    let str_proto = INIT_PARAM.with(|init_param| (*init_param.get()).str_prototype);
    let pointer = alloc_obj(str_proto, len, rbp, rsp);
    std::ptr::copy_nonoverlapping(
        input.as_ptr(),
        (pointer as *mut u8).add(size_of::<ArrayObject>()),
        input.len(),
    );
    pointer
}

/// Initialize runtime
///
/// # Safety
///  - `*init_param` never changes after calling this function
///  - Other safety requirements on `InitParam`
#[export_name = "$init"]
pub unsafe extern "C" fn init(init_param: *const InitParam) {
    INIT_PARAM.with(|i| i.set(init_param));
}

fn exit_code(code: i32) -> ! {
    println!("Exited with error code {}", code);
    exit(code);
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

#[cfg(not(test))]
pub mod crt0_glue {
    extern "C" {
        #[link_name = "$chocopy_main"]
        fn chocopy_main();
    }

    /// # Safety
    /// `$chocopy_main` is linked to a valid ChocoPy program entry point
    #[export_name = "main"]
    pub unsafe extern "C" fn entry_point() -> i32 {
        chocopy_main();
        0
    }
}
