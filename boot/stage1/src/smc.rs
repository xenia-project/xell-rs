//! This file includes routines to communicate with the SMC.
use crate::mutex::SpinMutex;

const SMC_ADDRESS: *mut u32 = 0x8000_0200_EA00_1000 as *mut u32;

pub struct SMC {}

impl SMC {
    const fn new() -> Self {
        Self {}
    }

    pub fn send_message(&mut self, msg: &[u32; 4]) {
        unsafe {
            while (core::ptr::read_volatile(SMC_ADDRESS.offset(33)) & 0x04000000) == 0 {}

            core::ptr::write_volatile::<u32>(SMC_ADDRESS.offset(33), 0x04000000);
            core::ptr::write_volatile::<u32>(SMC_ADDRESS.offset(32), msg[0]);
            core::ptr::write_volatile::<u32>(SMC_ADDRESS.offset(32), msg[1]);
            core::ptr::write_volatile::<u32>(SMC_ADDRESS.offset(32), msg[2]);
            core::ptr::write_volatile::<u32>(SMC_ADDRESS.offset(32), msg[3]);
            core::ptr::write_volatile::<u32>(SMC_ADDRESS.offset(33), 0x00000000);
        }
    }

    pub fn set_led(&mut self, ovflag: bool, value: u8) {
        let buf: [u32; 4] = [
            0x99000000 | ((ovflag as u32) << 16) | ((value as u32) << 8),
            0x00000000,
            0x00000000,
            0x00000000,
        ];

        self.send_message(&buf);
    }
}

pub static SMC: SpinMutex<SMC> = SpinMutex::new(SMC::new());
