//! This module defines exception handlers.

use atomic::{Atomic, Ordering};
use core::fmt::{self, Debug};

use crate::{smc, uart};

use ufmt::{derive::*, uDebug};
use xenon_cpu::mfspr;

pub const EXCEPTION_VECTORS: [usize; 17] = [
    0x00000000_00000100, // Reset
    0x00000000_00000200, // Machine check
    0x00000000_00000300, // Data storage
    0x00000000_00000380, // Data segment
    0x00000000_00000400, // Instruction storage
    0x00000000_00000480, // Instruction segment
    0x00000000_00000500, // External interrupt
    0x00000000_00000600, // Alignment
    0x00000000_00000700, // Program
    0x00000000_00000800, // Floating point
    0x00000000_00000900, // Decrementer
    0x00000000_00000980,
    0x00000000_00000c00, // System call
    0x00000000_00000d00, // Trace
    0x00000000_00000f00, // Performance
    0x00000000_00001600,
    0x00000000_00001800,
];

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, uDebug)]
#[non_exhaustive] // N.B: NECESSARY because we cast from integers.
#[repr(u32)]
pub enum ExceptionType {
    Reset = 0x10,
    MachineCheck = 0x20,
    Dsi = 0x30,
    DataSegment = 0x38,
    Isi = 0x40,
    InstructionSegment = 0x48,
    ExternalInterrupt = 0x50,
    Alignment = 0x60,
    Program = 0x70,
    FloatingPoint = 0x80,
    Decrementer = 0x90,
    SystemCall = 0xC0,
    Trace = 0xD0,
    Performance = 0xF0,
}

#[repr(C, align(512))]
#[derive(Copy, Clone, Default)]
pub struct CpuContext {
    r: [u64; 32],
    cr: u64,  // 0x100 (256)
    lr: u64,  // 0x108 (264)
    ctr: u64, // 0x110 (272)
    pc: u64,  // 0x118 (280)
    msr: u64, // 0x120 (288)
}

impl Debug for CpuContext {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> core::fmt::Result {
        core::writeln!(fmt, "r:")?;
        for i in 0..32 {
            core::writeln!(fmt, "  {}: {}", i, self.r[i])?;
        }

        core::writeln!(fmt, "cr: {}", self.cr)?;
        core::writeln!(fmt, "lr: {}", self.lr)?;
        core::writeln!(fmt, "ctr: {}", self.ctr)?;
        core::writeln!(fmt, "pc: {}", self.pc)?;
        core::writeln!(fmt, "msr: {}", self.msr)?;

        Ok(())
    }
}

impl uDebug for CpuContext {
    fn fmt<W>(&self, fmt: &mut ufmt::Formatter<'_, W>) -> Result<(), W::Error>
    where
        W: ufmt::uWrite + ?Sized,
    {
        ufmt::uwriteln!(fmt, "r:")?;
        for i in 0..32 {
            ufmt::uwriteln!(fmt, "  {}: {}", i, self.r[i])?;
        }

        ufmt::uwriteln!(fmt, "cr: {}", self.cr)?;
        ufmt::uwriteln!(fmt, "lr: {}", self.lr)?;
        ufmt::uwriteln!(fmt, "ctr: {}", self.ctr)?;
        ufmt::uwriteln!(fmt, "pc: {}", self.pc)?;
        ufmt::uwriteln!(fmt, "msr: {}", self.msr)?;

        Ok(())
    }
}

#[allow(dead_code)]
impl CpuContext {
    pub const fn new() -> Self {
        Self {
            r: [0u64; 32],
            cr: 0u64,
            lr: 0u64,
            ctr: 0u64,
            pc: 0u64,
            msr: 0u64,
        }
    }

    pub const fn with_hvcall(func: usize, r1: u64) -> Self {
        Self {
            r: [
                0xBEBEBEBE_BEBEBEBE, // r0
                r1,                  // r1
                0xBEBEBEBE_BEBEBEBE, // r2
                0xBEBEBEBE_BEBEBEBE, // r3
                0xBEBEBEBE_BEBEBEBE, // r4
                0xBEBEBEBE_BEBEBEBE, // r5
                0xBEBEBEBE_BEBEBEBE, // r6
                0xBEBEBEBE_BEBEBEBE, // r7
                0xBEBEBEBE_BEBEBEBE, // r8
                0xBEBEBEBE_BEBEBEBE, // r9
                0xBEBEBEBE_BEBEBEBE, // r10
                0xBEBEBEBE_BEBEBEBE, // r11
                func as u64,         // r12
                0xBEBEBEBE_BEBEBEBE, // r13
                0xBEBEBEBE_BEBEBEBE, // r14
                0xBEBEBEBE_BEBEBEBE, // r15
                0xBEBEBEBE_BEBEBEBE, // r16
                0xBEBEBEBE_BEBEBEBE, // r17
                0xBEBEBEBE_BEBEBEBE, // r18
                0xBEBEBEBE_BEBEBEBE, // r19
                0xBEBEBEBE_BEBEBEBE, // r20
                0xBEBEBEBE_BEBEBEBE, // r21
                0xBEBEBEBE_BEBEBEBE, // r22
                0xBEBEBEBE_BEBEBEBE, // r23
                0xBEBEBEBE_BEBEBEBE, // r24
                0xBEBEBEBE_BEBEBEBE, // r25
                0xBEBEBEBE_BEBEBEBE, // r26
                0xBEBEBEBE_BEBEBEBE, // r27
                0xBEBEBEBE_BEBEBEBE, // r28
                0xBEBEBEBE_BEBEBEBE, // r29
                0xBEBEBEBE_BEBEBEBE, // r30
                0xBEBEBEBE_BEBEBEBE, // r31
            ],
            cr: 0xBEBEBEBE_BEBEBEBE,
            lr: 0xBEBEBEBE_BEBEBEBE,
            ctr: 0xBEBEBEBE_BEBEBEBE,
            pc: func as u64,
            msr: 0x90000000_00001000,
        }
    }
}

/// This is a per-processor area where context information is saved when
/// an exception is encountered.
#[no_mangle]
static mut EXCEPTION_SAVE_AREA: [CpuContext; 6] = [CpuContext::new(); 6];

/// This area contains context information for per-process exception handlers.
/// This is generally static and unmodified.
#[no_mangle]
static mut EXCEPTION_LOAD_AREA: [CpuContext; 6] = [CpuContext::new(); 6];

/// The definition of the application-defined exception handler.
pub type ExceptionHandler = fn(ExceptionType, &CpuContext) -> Result<(), ()>;

/// The application-defined exception handler.
static EXCEPTION_HANDLER: Atomic<Option<ExceptionHandler>> = Atomic::new(None);

#[no_mangle]
extern "C" fn handle_exception() -> ! {
    // FIXME: This may allow for unencoded enum discriminants to exist.
    let id: ExceptionType = {
        unsafe { core::mem::transmute(mfspr!(304) as u32) } // HPSRG0
    };

    let save_area: &mut CpuContext = unsafe {
        let pir = mfspr!(1023);
        &mut EXCEPTION_SAVE_AREA[pir as usize]
    };

    match EXCEPTION_HANDLER.load(Ordering::Relaxed) {
        Some(ex) => {
            // If the handler successfully handles the exception, reload the calling context.
            if ex(id, save_area).is_ok() {
                unsafe {
                    load_context(save_area);
                }
            }
        }

        None => {}
    }

    match id {
        _ => {
            let pir = unsafe { mfspr!(1023) };

            let closure = |uart: &mut uart::UART| {
                ufmt::uwriteln!(uart, "UNHANDLED EXCEPTION! Hit exception vector {:?}", id)
                    .unwrap();
                ufmt::uwriteln!(uart, "MSR:   {:#?}", xenon_cpu::intrin::mfmsr()).unwrap();
                ufmt::uwriteln!(uart, "PIR:   {:#?}", pir).unwrap();
                ufmt::uwriteln!(uart, "---- Saved registers:").unwrap();
                ufmt::uwriteln!(uart, "    MSR:   {:#?}", save_area.msr).unwrap();
                ufmt::uwriteln!(uart, "    LR:    {:#?}", save_area.lr).unwrap();
                ufmt::uwriteln!(uart, "    PC:    {:#?}", save_area.pc).unwrap();
            };

            // Attempt to lock the UART. If that fails (for example, because we took an exception
            // while the UART was locked), forcibly take it to print out error text.
            let res = {
                let mut tries = 0u64;

                loop {
                    match uart::UART.try_lock(&closure) {
                        Ok(_) => break Ok(()),
                        Err(_) => {
                            if tries > 50 {
                                break Err(());
                            }

                            tries += 1;
                            xenon_cpu::time::delay(core::time::Duration::from_millis(100));
                        }
                    }
                }
            };

            if res.is_err() {
                let mut uart = unsafe { uart::UART.get_mut_unchecked() };
                closure(&mut uart);
            }

            if pir == 0 {
                // Not good. Auto-reset the system.
                smc::SMC.lock(|smc| {
                    smc.send_message(&[0x82043000u32, 0x00000000u32, 0x00000000u32, 0x00000000u32]);
                });
            }

            loop {}
        }
    }
}

#[naked]
#[no_mangle]
pub unsafe extern "C" fn load_context(_ctx: &CpuContext) -> ! {
    asm!(
        "ld     %r0, 0x100(%r3)",
        "mtcr   %r0",
        "ld     %r0, 0x108(%r3)",
        "mtlr   %r0",
        "ld     %r0, 0x110(%r3)",
        "mtctr  %r0",
        "ld     %r0, 0x118(%r3)",
        "mtsrr0 %r0",
        "ld     %r0, 0x120(%r3)",
        "mtsrr1 %r0",
        "ld     %r0, 0x00(%r3)",
        "ld     %r1, 0x08(%r3)",
        "ld     %r2, 0x10(%r3)",
        // N.B: r3 is loaded last.
        "ld     %r4, 0x20(%r3)",
        "ld     %r5, 0x28(%r3)",
        "ld     %r6, 0x30(%r3)",
        "ld     %r7, 0x38(%r3)",
        "ld     %r8, 0x40(%r3)",
        "ld     %r9, 0x48(%r3)",
        "ld     %r10, 0x50(%r3)",
        "ld     %r11, 0x58(%r3)",
        "ld     %r12, 0x60(%r3)",
        "ld     %r13, 0x68(%r3)",
        "ld     %r14, 0x70(%r3)",
        "ld     %r15, 0x78(%r3)",
        "ld     %r16, 0x80(%r3)",
        "ld     %r17, 0x88(%r3)",
        "ld     %r18, 0x90(%r3)",
        "ld     %r19, 0x98(%r3)",
        "ld     %r20, 0xA0(%r3)",
        "ld     %r21, 0xA8(%r3)",
        "ld     %r22, 0xB0(%r3)",
        "ld     %r23, 0xB8(%r3)",
        "ld     %r24, 0xC0(%r3)",
        "ld     %r25, 0xC8(%r3)",
        "ld     %r26, 0xD0(%r3)",
        "ld     %r27, 0xD8(%r3)",
        "ld     %r28, 0xE0(%r3)",
        "ld     %r29, 0xE8(%r3)",
        "ld     %r30, 0xF0(%r3)",
        "ld     %r31, 0xF8(%r3)",
        "ld     %r3, 0x18(%r3)",
        "rfid",
        options(noreturn),
    );
}

#[naked]
unsafe extern "C" fn except_thunk() -> ! {
    asm!(
        "mtctr  %r4",       // Reload CTR with original value
        "mfspr  %r4, 1023", // r4 = PIR
        "sldi   %r4, %r4, 32 + 9",
        "oris   %r4, %r4, EXCEPTION_SAVE_AREA@highest",
        "ori    %r4, %r4, EXCEPTION_SAVE_AREA@higher",
        "rotldi %r4, %r4, 32",
        "oris   %r4, %r4, EXCEPTION_SAVE_AREA@high",
        "ori    %r4, %r4, EXCEPTION_SAVE_AREA@l",
        // Now save registers.
        "std    %r0, 0x00(%r4)",
        "std    %r1, 0x08(%r4)",
        "std    %r2, 0x10(%r4)",
        "mfspr  %r0, 304", // Reload R3, which was saved in HPSRG0.
        "std    %r0, 0x18(%r4)",
        "mfspr  %r0, 305", // Reload R4, which was saved in HSPRG1.
        "std    %r0, 0x20(%r4)",
        "std    %r5, 0x28(%r4)",
        "std    %r6, 0x30(%r4)",
        "std    %r7, 0x38(%r4)",
        "std    %r8, 0x40(%r4)",
        "std    %r9, 0x48(%r4)",
        "std    %r10, 0x50(%r4)",
        "std    %r11, 0x58(%r4)",
        "std    %r12, 0x60(%r4)",
        "std    %r13, 0x68(%r4)",
        "std    %r14, 0x70(%r4)",
        "std    %r15, 0x78(%r4)",
        "std    %r16, 0x80(%r4)",
        "std    %r17, 0x88(%r4)",
        "std    %r18, 0x90(%r4)",
        "std    %r19, 0x98(%r4)",
        "std    %r20, 0xA0(%r4)",
        "std    %r21, 0xA8(%r4)",
        "std    %r22, 0xB0(%r4)",
        "std    %r23, 0xB8(%r4)",
        "std    %r24, 0xC0(%r4)",
        "std    %r25, 0xC8(%r4)",
        "std    %r26, 0xD0(%r4)",
        "std    %r27, 0xD8(%r4)",
        "std    %r28, 0xE0(%r4)",
        "std    %r29, 0xE8(%r4)",
        "std    %r30, 0xF0(%r4)",
        "std    %r31, 0xF8(%r4)",
        "mfcr   %r0",
        "std    %r0, 0x100(%r4)",
        "mflr   %r0",
        "std    %r0, 0x108(%r4)",
        "mfctr  %r0",
        "std    %r0, 0x110(%r4)",
        "mfsrr0 %r0",
        "std    %r0, 0x118(%r4)",
        "mfsrr1 %r0",
        "std    %r0, 0x120(%r4)",
        "mtspr  304, %r3", // HPSRG0 = exception ID
        // Now load the exception load context.
        "b      except_load_thunk",
        options(noreturn)
    );
}

#[naked]
#[no_mangle]
unsafe extern "C" fn except_load_thunk() -> ! {
    asm!(
        "mfspr  %r3, 1023", // r3 = PIR
        "sldi   %r3, %r3, 32 + 9",
        // N.B: These instructions are patched later.
        "trap",
        "trap",
        "rotldi %r3, %r3, 32",
        "trap",
        "trap",
        "b      load_context",
        options(noreturn)
    )
}

/// Create a longjmp for an exception vector.
/// This will preverse r3/r4 in HSPRG0 and HSPRG1, respectively.
/// r3 will be loaded with the constant specified in the `id` parameter.
/// r4 will be loaded with the value of CTR.
const fn make_longjmp_exc(id: u16, target: usize) -> [u32; 11] {
    [
        0x7C704BA6,                                      // mtspr   HSPRG0, %r3
        0x7C914BA6,                                      // mtspr   HSPRG1, %r4
        (0x3C600000 | ((target >> 48) & 0xFFFF)) as u32, // lis     %r3, target[64:48]
        (0x60630000 | ((target >> 32) & 0xFFFF)) as u32, // ori     %r3, %r3, target[48:32]
        0x786307C6,                                      // rldicr  %r3, %r3, 32, 31
        (0x64630000 | ((target >> 16) & 0xFFFF)) as u32, // oris    %r3, %r3, target[32:16]
        (0x60630000 | ((target >> 00) & 0xFFFF)) as u32, // ori     %r3, %r3, target[16:0]
        0x7C8902A6,                                      // mfctr   %r4
        0x7C6903A6,                                      // mtctr   %r3
        (0x38600000 | (id as u32)),                      // li      %r3, id
        0x4E800420,                                      // bctr
    ]
}

/// Create an address suitable for loading using signed arithmetic,
/// i.e:
///
/// ```asm
/// lis %rX, <addr>@ha
/// addi %rX, <addr>@l
/// ```
const fn make_arithaddr(addr: u32) -> (u16, u16) {
    let lo = (addr & 0xFFFF) as u16;
    let hi = { ((addr >> 16) as u16) + if (lo & 0x8000) != 0 { 1 } else { 0 } };

    (hi, lo)
}

pub unsafe fn cause_exception() -> ! {
    // Trap.
    asm!("trap", options(noreturn));
}

pub unsafe fn init_except(handler: Option<ExceptionHandler>) {
    EXCEPTION_HANDLER.store(handler, Ordering::Relaxed);

    // Set up the load area.
    EXCEPTION_LOAD_AREA = [
        CpuContext::with_hvcall(handle_exception as usize, 0x8000_0000_1FFF_0000),
        CpuContext::with_hvcall(handle_exception as usize, 0x8000_0000_1FFE_0000),
        CpuContext::with_hvcall(handle_exception as usize, 0x8000_0000_1FFD_0000),
        CpuContext::with_hvcall(handle_exception as usize, 0x8000_0000_1FFC_0000),
        CpuContext::with_hvcall(handle_exception as usize, 0x8000_0000_1FFB_0000),
        CpuContext::with_hvcall(handle_exception as usize, 0x8000_0000_1FFA_0000),
    ];

    // N.B: We have to patch the exception thunk to deal with PIE.
    {
        let save_area = &mut EXCEPTION_SAVE_AREA[0] as *mut _ as usize;
        let thunk_area = except_thunk as usize as *mut u32;

        // We have to use addition here because the PIR is pre-loaded into r4 by
        // the thunk, and a bitwise OR will not properly add it as an offset.
        // We only have to use addition on the lowest chunk, because the highest
        // offset is `0xA00` (5 << 9).
        let (arith_hi, arith_lo) = make_arithaddr(save_area as u32);

        // "oris   %r4, %r4, EXCEPTION_SAVE_AREA@highest"
        thunk_area
            .offset(3)
            .write_volatile(0x64840000 | ((save_area >> 48) & 0xFFFF) as u32);
        // "ori    %r4, %r4, EXCEPTION_SAVE_AREA@higher"
        thunk_area
            .offset(4)
            .write_volatile(0x60840000 | ((save_area >> 32) & 0xFFFF) as u32);
        // "oris   %r4, %r4, EXCEPTION_SAVE_AREA@ha"
        thunk_area
            .offset(6)
            .write_volatile(0x64840000 | arith_hi as u32);
        // "addi   %r4, %r4, EXCEPTION_SAVE_AREA@l"
        thunk_area
            .offset(7)
            .write_volatile(0x38840000 | arith_lo as u32);
    }

    // Ditto for the load thunk.
    {
        let load_area = &mut EXCEPTION_LOAD_AREA[0] as *mut _ as usize;
        let thunk_area = except_load_thunk as usize as *mut u32;

        let (arith_hi, arith_lo) = make_arithaddr(load_area as u32);

        // "oris   %r3, %r3, EXCEPTION_LOAD_AREA@highest"
        thunk_area
            .offset(2)
            .write_volatile(0x64630000 | ((load_area >> 48) & 0xFFFF) as u32);
        // "ori    %r3, %r3, EXCEPTION_LOAD_AREA@higher"
        thunk_area
            .offset(3)
            .write_volatile(0x60630000 | ((load_area >> 32) & 0xFFFF) as u32);
        // "oris   %r3, %r3, EXCEPTION_LOAD_AREA@ha"
        thunk_area
            .offset(5)
            .write_volatile(0x64630000 | arith_hi as u32);
        // "addi   %r3, %r3, EXCEPTION_LOAD_AREA@l"
        thunk_area
            .offset(6)
            .write_volatile(0x38630000 | arith_lo as u32);
    }

    for vec in EXCEPTION_VECTORS.iter() {
        let buf = make_longjmp_exc((*vec >> 4) as u16, except_thunk as usize);
        core::ptr::copy_nonoverlapping(buf.as_ptr(), *vec as *mut u32, buf.len());
    }
}

#[cfg(test)]
mod test {
    use crate::except::make_arithaddr;

    #[test]
    fn test_arithaddr() {
        assert_eq!(make_arithaddr(0x0B0B8018), 0x0B0C8018);
    }
}
