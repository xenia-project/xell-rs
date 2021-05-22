//! This file includes routines to communicate with the SMC.
const SMC_ADDRESS: *mut u32 = 0x8000_0200_EA00_1000 as *mut u32;

pub unsafe fn send_message(msg: &[u32; 4]) {
    while (core::ptr::read_volatile(SMC_ADDRESS.offset(33)) & 0x04000000) == 0 {}

    core::ptr::write_volatile::<u32>(SMC_ADDRESS.offset(33), 0x04000000);
    core::ptr::write_volatile::<u32>(SMC_ADDRESS.offset(32), msg[0]);
    core::ptr::write_volatile::<u32>(SMC_ADDRESS.offset(32), msg[1]);
    core::ptr::write_volatile::<u32>(SMC_ADDRESS.offset(32), msg[2]);
    core::ptr::write_volatile::<u32>(SMC_ADDRESS.offset(32), msg[3]);
    core::ptr::write_volatile::<u32>(SMC_ADDRESS.offset(33), 0x00000000);
}

pub unsafe fn set_led(override_val: u8, value: u8) {
    let buf: [u32; 4] = [
        0x99000000 | ((override_val as u32) << 16) | ((value as u32) << 8),
        0x00000000,
        0x00000000,
        0x00000000,
    ];

    send_message(&buf);
}
