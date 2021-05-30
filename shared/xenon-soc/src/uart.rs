//! This file defines the UART interface on the SMC.
use sync::mutex::SpinMutex;

use core::fmt::Write;
use ufmt::uWrite;

const UART_BASE: *mut u32 = 0x8000_0200_EA00_1000 as *mut u32;

#[allow(dead_code)]
pub enum Speed {
    S115200, // 0b11100110
    S38400,  // 0b10110010
    S19200,  // 0b01100011
}

pub struct UART {}

impl UART {
    const fn new() -> Self {
        Self {}
    }

    pub fn reset(&mut self, speed: Speed) {
        unsafe {
            core::ptr::write_volatile(
                UART_BASE.offset(7),
                match speed {
                    Speed::S115200 => 0xE6010000,
                    Speed::S38400 => 0xB2010000,
                    Speed::S19200 => 0x63010000,
                },
            );
        }
    }

    fn data_pending(&mut self) -> bool {
        unsafe {
            // Busy loop while the SMC is busy.
            while (core::ptr::read_volatile(UART_BASE.offset(6)) & 0xFCFF_FFFF) != 0 {}

            core::ptr::read_volatile(UART_BASE.offset(6)) & 0x01000000 != 0
        }
    }

    pub fn read_byte(&mut self) -> u8 {
        // Wait for available character.
        while !self.data_pending() {}

        unsafe { (core::ptr::read_volatile(UART_BASE.offset(4)) >> 24) as u8 }
    }

    pub fn write_byte(&mut self, byte: u8) {
        unsafe {
            // Wait for the SMC to be ready.
            while (core::ptr::read_volatile(UART_BASE.offset(6)) & 0x02000000) == 0 {}

            core::ptr::write_volatile(UART_BASE.offset(5), (byte as u32) << 24);
        }
    }

    pub fn read(&mut self, mut data: &mut [u8]) {
        while !data.is_empty() {
            data[0] = self.read_byte();
            data = &mut data[1..];
        }
    }

    pub fn write(&mut self, data: &[u8]) {
        for b in data {
            self.write_byte(*b);
        }
    }
}

impl Write for UART {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.as_bytes().iter() {
            // Prepend newline characters with a carriage return.
            if *c == b'\n' {
                self.write_byte(b'\r');
            }

            self.write_byte(*c);
        }

        Ok(())
    }
}

impl uWrite for UART {
    type Error = core::convert::Infallible;

    fn write_str(&mut self, s: &str) -> Result<(), Self::Error> {
        for c in s.as_bytes().iter() {
            // Prepend newline characters with a carriage return.
            if *c == b'\n' {
                self.write_byte(b'\r');
            }

            self.write_byte(*c);
        }

        Ok(())
    }
}

pub static UART: SpinMutex<UART> = SpinMutex::new(UART::new());
