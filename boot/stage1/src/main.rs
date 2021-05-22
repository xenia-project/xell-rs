#![feature(
    const_raw_ptr_to_usize_cast,
    const_panic,
    global_asm,
    lang_items,
    naked_functions,
    asm
)]
#![no_std]
#![no_main]

use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

global_asm!(include_str!("startup.s"));

extern crate core_reqs;

mod except;
mod iic;
mod panic;
mod smc;

extern "C" {
    fn start_from_libxenon() -> !;
}

static PROCESSORS: AtomicU32 = AtomicU32::new(0);
static SECONDARY_BRANCH_TARGET: AtomicUsize = AtomicUsize::new(0);

const fn make_longjmp(target: usize) -> [u32; 11] {
    [
        (0x3C600000 | ((target >> 48) & 0xFFFF)) as u32, // lis     %r3, target[64:48]
        (0x60630000 | ((target >> 32) & 0xFFFF)) as u32, // ori     %r3, %r3, target[48:32]
        0x786307C6,                                      // rldicr  %r3, %r3, 32, 31
        (0x64630000 | ((target >> 16) & 0xFFFF)) as u32, // oris    %r3, %r3, target[32:16]
        (0x60630000 | ((target >> 00) & 0xFFFF)) as u32, // ori     %r3, %r3, target[16:0]
        0x7C7A03A6,                                      // mtsrr0  %r3
        // Clear MSR[IR/DR]
        0x38800030, // li       %r4, 0x30
        0x7C6000A6, // mfmsr    %r3
        0x7C632078, // andc     %r3, %r3, %r4
        0x7C7B03A6, // mtsrr1   %r3
        // Branch to target.
        0x4C000024, // rfid
    ]
}

const fn abs_diff(a: usize, b: usize) -> usize {
    if a > b {
        a - b
    } else {
        b - a
    }
}

const fn make_reljump(address: usize, target: usize) -> u32 {
    let diff = abs_diff(target, address);
    let offset = target.wrapping_sub(address);

    // If the offset can fit within a single branch instruction, use it.
    if diff < 0x7F_FFFF {
        (0x4800_0000 | (offset & 0x00FF_FFFC)) as u32
    } else {
        panic!("Offset too large for relative jump!");
    }
}

#[no_mangle]
#[link_section = ".text.startup"]
pub extern "C" fn __start_rust(pir: u32, _hrmor: u32, _pvr: u32, src: u32) -> ! {
    unsafe { smc::set_led(1, 0xF0); }

    PROCESSORS.fetch_or(1 << pir, Ordering::Relaxed);
    if pir != 0 {
        loop {
            let target = SECONDARY_BRANCH_TARGET.load(Ordering::Relaxed);
            if target != 0 {
                unsafe {
                    let func: fn() -> ! = core::mem::transmute(target);
                    func();
                }
            }
        }
    }

    match src {
        // Startup from ROM
        0 => loop {},

        // Startup from OS
        1 => {
            // We'll need to catch all other cores that may still be running the OS.
            // Set a branch on the external interrupt vector, and trigger an IPI.
            unsafe {
                // Create a jump buffer. This will perform a longjmp to our target address in real-mode.
                let jmpbuf = make_longjmp(start_from_libxenon as usize);

                // Copy the jump buffer to some unused bytes at the beginning of the hypervisor.
                core::ptr::copy_nonoverlapping(
                    jmpbuf.as_ptr(),
                    0x000000A0 as *mut u32,
                    jmpbuf.len(),
                );

                // Ensure the compiler does not reorder instructions.
                core::sync::atomic::compiler_fence(Ordering::SeqCst);

                // Make the external interrupt vector jump to our trampoline.
                let insn = make_reljump(0x00000500usize, 0x000000A0 as usize);
                core::ptr::write_volatile(0x00000500 as *mut u32, insn);

                // Ensure the compiler does not reorder instructions.
                core::sync::atomic::compiler_fence(Ordering::SeqCst);

                // Set the IRQL on all other processors to 0 (to unmask all interrupts).
                // The hypervisor isn't going to like this, but we set a detour on the interrupt vector earlier.
                for i in 1usize..6usize {
                    let ptr = (0x8000_0200_0005_0000 + (i * 0x1000)) as *mut u32;
                    ptr.offset(2).write_volatile(0x0000_0000);
                }

                // Trigger an IPI on all other processors, with vector 0x78.
                core::ptr::write_volatile(0x8000_0200_0005_0010 as *mut u32, 0x003E_0078);
            };

            // Loop...
            while PROCESSORS.load(Ordering::Relaxed) != 0x3F {
                unsafe { smc::set_led(1, PROCESSORS.load(Ordering::Relaxed) as u8); }
            }
        }

        // Shouldn't hit this case.
        _ => loop {},
    }

    // Now the system is in a defined state. All secondary processors are captured,
    // and we are free to modify the system.
    unsafe { except::init_except() };

    unsafe { smc::set_led(1, 0xF0) };

    loop {}
}
