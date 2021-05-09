use core::panic::PanicInfo;

#[lang = "eh_personality"]
extern fn rust_eh_personality() {}

#[panic_handler]
#[no_mangle]
pub fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
