use core::{ffi::c_void, panic::PanicInfo};

#[lang = "eh_personality"]
extern "C" fn rust_eh_personality() {}

#[panic_handler]
#[no_mangle]
pub fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn _Unwind_Resume(_exception: *mut c_void) -> ! {
    loop {}
}
