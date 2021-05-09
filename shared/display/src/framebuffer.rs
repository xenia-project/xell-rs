use core::slice;
use embedded_graphics::{pixelcolor::Rgb888, prelude::*};

// XeLL framebuffer address.
const FRAMEBUFFER_ADDRESS: usize = 0x8000_0000_1E00_0000;
const FRAMEBUFFER_SIZE: usize = ((1280 + 31) & !31) + ((720 + 31) & !31);

pub struct XenosDisplay<'a> {
    framebuffer: &'a mut [u8],

    width: u32,
    height: u32,
}

impl XenosDisplay<'_> {
    pub unsafe fn new() -> Self {
        Self {
            framebuffer: slice::from_raw_parts_mut(
                FRAMEBUFFER_ADDRESS as *mut u8,
                FRAMEBUFFER_SIZE,
            ),

            width: 1280,
            height: 720,
        }
    }
}
