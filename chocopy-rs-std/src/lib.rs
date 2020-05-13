use chocopy_rs_common::*;
use std::mem::*;
use std::process::exit;
use std::sync::atomic::*;

static ALLOC_COUNTER: AtomicU64 = AtomicU64::new(0);

#[repr(transparent)]
#[derive(Clone, Copy)]
struct AllocUnit(u64);

pub fn divide_up(value: usize) -> usize {
    let align = size_of::<AllocUnit>();
    if value == 0 {
        0
    } else {
        1 + (value - 1) / align
    }
}

/// Destructor for [object] type
///
/// # Safety
///  - `pointer` must be previouly returned by returned by `alloc_obj`.
///  - The object must be allocated with a [object] prototype (`-prototype.size` is size of a pointer).
///  - The `prototype` and `len` field must be intact.
///  - Each list element must be either a valid `Object` pointer (returned by `alloc_obj`) or null.
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

/// Allocates a ChocoPy object
///
/// # Safety
///  - `prototype.size` is not 0.
///  - `prototype.tag` is Str or List if and only if `prototype.size < 0`.
///  - `prototype.dtor` points to a valid function.
#[export_name = "$alloc_obj"]
pub unsafe extern "C" fn alloc_obj(prototype: *const Prototype, len: u64) -> *mut Object {
    let size = divide_up(if (*prototype).size > 0 {
        assert!(len == 0);
        size_of::<Object>() + (*prototype).size as usize
    } else {
        size_of::<ArrayObject>() + (-(*prototype).size as u64 * len) as usize
    });

    let pointer =
        Box::into_raw(vec![AllocUnit(0); size].into_boxed_slice()) as *mut AllocUnit as *mut Object;

    let object = Object {
        prototype,
        ref_count: 1,
    };

    if (*prototype).size > 0 {
        pointer.write(object);
    } else {
        let object = ArrayObject { object, len };
        (pointer as *mut ArrayObject).write(object);
    }

    ALLOC_COUNTER.fetch_add(1, Ordering::SeqCst);
    pointer
}

/// Deallocates a ChocoPy object
///
/// # Safety
///  - `pointer` must be previously returned by `alloc_obj`.
///  - The `prototype` field must be intact.
///  - For `ArrayObject`, the `len` field must be intact.
///  - Other safety requirements to call `dtor` on `pointer` must be hold.
#[export_name = "$free_obj"]
pub unsafe extern "C" fn free_obj(pointer: *mut Object) {
    assert!((*pointer).ref_count == 0);
    let prototype = (*pointer).prototype;
    ((*prototype).dtor)(pointer);
    let size = divide_up(if (*prototype).size > 0 {
        size_of::<Object>() + (*prototype).size as usize
    } else {
        let len = (*(pointer as *mut ArrayObject)).len;
        size_of::<ArrayObject>() + (-(*prototype).size as u64 * len) as usize
    });

    drop(Box::from_raw(std::slice::from_raw_parts_mut(
        pointer as *mut AllocUnit,
        size,
    )));

    ALLOC_COUNTER.fetch_sub(1, Ordering::SeqCst);
}

/// Gets the array length of a ChocoPy object
///
/// # Safety
///  - `pointer` must be previously returned by `alloc_obj`.
///  - The `prototype` field must be intact.
///  - For `ArrayObject`, the `len` field must be intact.
///  - Other safety requirements to call `dtor` on `pointer` must be hold.
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

/// Prints a ChocoPy object
///
/// # Safety
///  - `pointer` must be previously returned by `alloc_obj`.
///  - The `prototype` field must be intact.
///  - Other safety requirements to call `dtor` on `pointer` must be hold.
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

/// Creates a new str object that holds a line of user input
///
/// # Safety
///  - `str_proto.size == -1` .
///  - `str_proto.tag == TypeTag::Str`.
///  - `str_proto.dtor` points to a no-op function.
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
    use super::*;
    extern "C" {
        #[link_name = "$chocopy_main"]
        fn chocopy_main();
    }

    /// # Safety
    /// `$chocopy_main` is linked to a valid ChocoPy program entry point
    #[export_name = "main"]
    pub unsafe extern "C" fn entry_point() -> i32 {
        chocopy_main();
        if ALLOC_COUNTER.load(Ordering::SeqCst) != 0 {
            println!("--- Memory leak detected! ---");
            exit_code(-1);
        }
        0
    }
}
