use std::alloc::*;
use std::mem::*;
use std::process::exit;
use std::sync::atomic::*;

static ALLOC_COUNTER: AtomicU64 = AtomicU64::new(0);

#[repr(C)]
pub struct Prototype {
    size: i64,
    dtor: unsafe extern "C" fn(*mut u8),
    ctor: unsafe extern "C" fn(),
    // followed by other method pointers
}

#[export_name = "bool.$proto"]
pub static BOOL_PROTOTYPE: Prototype = Prototype {
    size: 1,
    dtor: dtor_noop,
    ctor: object_init,
};

#[export_name = "int.$proto"]
pub static INT_PROTOTYPE: Prototype = Prototype {
    size: 4,
    dtor: dtor_noop,
    ctor: object_init,
};

#[export_name = "str.$proto"]
pub static STR_PROTOTYPE: Prototype = Prototype {
    size: -1,
    dtor: dtor_noop,
    ctor: object_init,
};

#[export_name = "[bool].$proto"]
pub static BOOL_LIST_PROTOTYPE: Prototype = Prototype {
    size: -1,
    dtor: dtor_noop,
    ctor: object_init,
};

#[export_name = "[int].$proto"]
pub static INT_LIST_PROTOTYPE: Prototype = Prototype {
    size: -4,
    dtor: dtor_noop,
    ctor: object_init,
};

#[export_name = "[object].$proto"]
pub static OBJECT_LIST_PROTOTYPE: Prototype = Prototype {
    size: -8,
    dtor: dtor_list,
    ctor: object_init,
};

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

extern "C" fn dtor_noop(_: *mut u8) {}

unsafe extern "C" fn dtor_list(pointer: *mut u8) {
    let object = pointer as *mut ArrayObject;
    let len = (*object).len;
    let elements = object.offset(1) as *mut *mut Object;
    for i in 0..len {
        let element = *elements.offset(i as isize);
        if !element.is_null() {
            (*element).ref_count -= 1;
            if (*element).ref_count == 0 {
                free_obj(element as *mut u8);
            }
        }
    }
}

#[cfg(not(test))]
extern "C" {
    #[link_name = "object.__init__"]
    fn object_init();
}

#[cfg(test)]
extern "C" fn object_init() {}

#[export_name = "$alloc_obj"]
pub unsafe extern "C" fn alloc_obj(prototype: *const Prototype, len: u64) -> *mut u8 {
    let size = align_up(if (*prototype).size > 0 {
        assert!(len == 0);
        size_of::<Object>() + (*prototype).size as usize
    } else {
        size_of::<ArrayObject>() + (-(*prototype).size as u64 * len) as usize
    });
    let pointer = alloc(Layout::from_size_align(size, 8).unwrap());
    if pointer.is_null() {
        out_of_memory();
    }
    (*(pointer as *mut Object)).prototype = prototype;
    (*(pointer as *mut Object)).ref_count = 1;
    if (*prototype).size < 0 {
        (*(pointer as *mut ArrayObject)).len = len;
    }
    ALLOC_COUNTER.fetch_add(1, Ordering::SeqCst);
    pointer
}

#[export_name = "$free_obj"]
pub unsafe extern "C" fn free_obj(pointer: *mut u8) {
    assert!((*(pointer as *mut Object)).ref_count == 0);
    let prototype = (*(pointer as *mut Object)).prototype;
    ((*prototype).dtor)(pointer);
    let size = align_up(if (*prototype).size > 0 {
        size_of::<Object>() + (*prototype).size as usize
    } else {
        let len = (*(pointer as *mut ArrayObject)).len;
        size_of::<ArrayObject>() + (-(*prototype).size as u64 * len) as usize
    });
    dealloc(pointer, Layout::from_size_align(size, 8).unwrap());
    ALLOC_COUNTER.fetch_sub(1, Ordering::SeqCst);
}

#[export_name = "$len"]
pub unsafe extern "C" fn len(pointer: *mut u8) -> i32 {
    if pointer.is_null() {
        invalid_arg();
    }
    let object = pointer as *mut ArrayObject;
    let prototype = (*object).object.prototype;
    if prototype != &BOOL_LIST_PROTOTYPE as *const Prototype
        && prototype != &INT_LIST_PROTOTYPE as *const Prototype
        && prototype != &OBJECT_LIST_PROTOTYPE as *const Prototype
        && prototype != &STR_PROTOTYPE as *const Prototype
    {
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
pub unsafe extern "C" fn print(pointer: *mut u8) -> *mut u8 {
    if pointer.is_null() {
        invalid_arg();
    }
    let object = pointer as *mut Object;
    let prototype = (*object).prototype;
    if prototype == &INT_PROTOTYPE as *const Prototype {
        println!("{}", *(object.offset(1) as *const i32));
    } else if prototype == &BOOL_PROTOTYPE as *const Prototype {
        println!(
            "{}",
            if *(object.offset(1) as *const bool) {
                "True"
            } else {
                "False"
            }
        );
    } else if prototype == &STR_PROTOTYPE as *const Prototype {
        let object = object as *mut ArrayObject;
        let slice = std::str::from_utf8(std::slice::from_raw_parts(
            object.offset(1) as *const u8,
            (*object).len as usize,
        ))
        .unwrap();
        println!("{}", slice);
    } else {
        invalid_arg();
    }

    (*object).ref_count -= 1;
    if (*object).ref_count == 0 {
        free_obj(pointer);
    }
    std::ptr::null_mut()
}

#[export_name = "$input"]
pub unsafe extern "C" fn input() -> *mut u8 {
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
    let pointer = alloc_obj(&STR_PROTOTYPE as *const Prototype, len);
    std::ptr::copy_nonoverlapping(
        input.as_ptr(),
        pointer.offset(size_of::<ArrayObject>() as isize),
        input.len(),
    );
    pointer
}

#[cfg(not(test))]
fn memory_leak() -> ! {
    println!("--- Memory leak detected! ---");
    exit(-1)
}

#[export_name = "$broken_stack"]
pub unsafe extern "C" fn broken_stack() -> ! {
    println!("--- Broken stack detected! ---");
    exit(-2)
}

fn invalid_arg() -> ! {
    println!("--- Invalid argument ---");
    exit(1)
}

#[export_name = "$div_zero"]
pub unsafe extern "C" fn div_zero() -> ! {
    println!("--- Division by zero ---");
    exit(2)
}

#[export_name = "$out_of_bound"]
pub unsafe extern "C" fn out_of_bound() -> ! {
    println!("--- Index out of bounds ---");
    exit(3)
}

#[export_name = "$none_op"]
pub unsafe extern "C" fn none_op() -> ! {
    println!("--- Operation on None ---");
    exit(4)
}

fn out_of_memory() -> ! {
    println!("--- Out of memory ---");
    exit(5)
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
        memory_leak();
    }
    0
}
