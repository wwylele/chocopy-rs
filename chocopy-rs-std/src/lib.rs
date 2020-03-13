use std::alloc::*;
use std::mem::*;
use std::sync::atomic::*;

static ALLOC_COUNTER: AtomicU64 = AtomicU64::new(0);

#[repr(C)]
pub struct Prototype {
    size: i64,
}

#[no_mangle]
#[export_name = "$BOOL_PROTOTYPE"]
pub static BOOL_PROTOTYPE: Prototype = Prototype { size: 1 };

#[no_mangle]
#[export_name = "$INT_PROTOTYPE"]
pub static INT_PROTOTYPE: Prototype = Prototype { size: 4 };

#[no_mangle]
#[export_name = "$STR_PROTOTYPE"]
pub static STR_PROTOTYPE: Prototype = Prototype { size: -1 };

#[no_mangle]
#[export_name = "$OBJECT_PROTOTYPE"]
pub static OBJECT_PROTOTYPE: Prototype = Prototype { size: 0 };

#[no_mangle]
#[export_name = "$BOOL_LIST_PROTOTYPE"]
pub static BOOL_LIST_PROTOTYPE: Prototype = Prototype { size: -1 };

#[no_mangle]
#[export_name = "$INT_LIST_PROTOTYPE"]
pub static INT_LIST_PROTOTYPE: Prototype = Prototype { size: -4 };

#[no_mangle]
#[export_name = "$OBJECT_LIST_PROTOTYPE"]
pub static OBJECT_LIST_PROTOTYPE: Prototype = Prototype { size: -8 };

#[repr(C)]
pub struct Object {
    prototype: *const Prototype,
    ref_count: u64,
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

#[no_mangle]
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
        runtime_error("Out of memory");
    }
    (*(pointer as *mut Object)).prototype = prototype;
    (*(pointer as *mut Object)).ref_count = 1;
    if (*prototype).size < 0 {
        (*(pointer as *mut ArrayObject)).len = len;
    }
    ALLOC_COUNTER.fetch_add(1, Ordering::SeqCst);
    pointer
}

#[no_mangle]
#[export_name = "$free_obj"]
pub unsafe extern "C" fn free_obj(pointer: *mut u8) {
    assert!((*(pointer as *mut Object)).ref_count == 0);
    let prototype = (*(pointer as *mut Object)).prototype;
    let size = align_up(if (*prototype).size > 0 {
        size_of::<Object>() + (*prototype).size as usize
    } else {
        let len = (*(pointer as *mut ArrayObject)).len;
        size_of::<ArrayObject>() + (-(*prototype).size as u64 * len) as usize
    });
    dealloc(pointer, Layout::from_size_align(size, 8).unwrap());
    ALLOC_COUNTER.fetch_sub(1, Ordering::SeqCst);
}

#[no_mangle]
pub unsafe extern "C" fn len(pointer: *mut u8) -> u32 {
    let object = pointer as *mut ArrayObject;
    let prototype = (*object).object.prototype;
    if prototype != &BOOL_LIST_PROTOTYPE as *const Prototype
        && prototype != &INT_LIST_PROTOTYPE as *const Prototype
        && prototype != &OBJECT_LIST_PROTOTYPE as *const Prototype
        && prototype != &STR_PROTOTYPE as *const Prototype
    {
        runtime_error("len() only works for list or str");
    }
    let len = (*object).len as u32;
    (*object).object.ref_count -= 1;
    if (*object).object.ref_count == 0 {
        free_obj(pointer);
    }
    len
}

#[no_mangle]
pub unsafe extern "C" fn print(pointer: *mut u8) -> *mut u8 {
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
        runtime_error("print() only works for int, bool or str");
    }

    (*object).ref_count -= 1;
    if (*object).ref_count == 0 {
        free_obj(pointer);
    }
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn input() -> *mut u8 {
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    let len = (input.len() - 1) as u64; // remove the trailing line break
    let pointer = alloc_obj(&STR_PROTOTYPE as *const Prototype, len);
    std::ptr::copy_nonoverlapping(
        input.as_ptr(),
        pointer.offset(size_of::<ArrayObject>() as isize),
        input.len(),
    );
    pointer
}

#[no_mangle]
#[export_name = "$report_broken_stack"]
pub unsafe extern "C" fn report_broken_stack() {
    println!("--- broken stack detected! ---");
    crash()
}

extern "C" {
    #[cfg(not(test))]
    #[link_name = "$chocopy_main"]
    fn chocopy_main();
}
#[no_mangle]
#[cfg(not(test))]
pub unsafe extern "C" fn main() {
    chocopy_main();
    if ALLOC_COUNTER.load(Ordering::SeqCst) != 0 {
        println!("--- memory leak detected! ---");
    }
}

fn runtime_error(message: &str) -> ! {
    println!("Runtime error: {}", message);
    crash()
}

fn crash() -> ! {
    std::process::exit(-1)
}

/*
#[no_mangle]
pub extern "C" fn _Unwind_Resume() -> ! {
    crash()
}

#[no_mangle]
pub extern "C" fn _Unwind_Backtrace() -> ! {
    crash()
}

#[no_mangle]
pub extern "C" fn _Unwind_GetTextRelBase() -> ! {
    crash()
}

#[no_mangle]
pub extern "C" fn _Unwind_GetDataRelBase() -> ! {
    crash()
}

#[no_mangle]
pub extern "C" fn _Unwind_DeleteException() -> ! {
    crash()
}

#[no_mangle]
pub extern "C" fn _Unwind_RaiseException() -> ! {
    crash()
}

#[no_mangle]
pub extern "C" fn _Unwind_GetLanguageSpecificData() -> ! {
    crash()
}

#[no_mangle]
pub extern "C" fn _Unwind_GetIPInfo() -> ! {
    crash()
}

#[no_mangle]
pub extern "C" fn _Unwind_GetRegionStart() -> ! {
    crash()
}

#[no_mangle]
pub extern "C" fn _Unwind_SetGR() -> ! {
    crash()
}

#[no_mangle]
pub extern "C" fn _Unwind_SetIP() -> ! {
    crash()
}

#[no_mangle]
pub extern "C" fn _Unwind_GetIP() -> ! {
    crash()
}

#[no_mangle]
pub extern "C" fn _Unwind_FindEnclosingFunction() -> ! {
    crash()
}
*/
