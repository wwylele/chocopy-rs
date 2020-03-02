extern "C" {
    fn chocopy_main();
}

#[no_mangle]
pub extern "C" fn main() {
    println!("ChocoPy program starting.");
    unsafe {
        chocopy_main();
    }
    println!("ChocoPy program ended.");
}

#[no_mangle]
pub extern "C" fn debug_print(input: i64) {
    println!("debug_print: {}", input);
}

fn crash() -> ! {
    std::process::exit(-1)
}

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
