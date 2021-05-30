use core::{ffi::c_void, panic::PanicInfo};

use xenon_soc::uart;

#[lang = "eh_personality"]
extern "C" fn rust_eh_personality() {}

#[panic_handler]
#[no_mangle]
pub fn panic(_info: &PanicInfo) -> ! {
    let uart = unsafe { uart::UART.get_mut_unchecked() };

    uart.write(b"RUST PANIC!\r\n");

    loop {}
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn _Unwind_Resume(_exception: *mut c_void) -> ! {
    loop {}
}
