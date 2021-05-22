//! This module defines exception handlers.

use crate::iic::Iic;

#[allow(dead_code)]
#[repr(u32)]
enum ExceptionType {
    Reset = 1,
    MachineCheck = 2,
    Dsi = 3,
    Isi = 4,
    ExternalInterrupt = 5,
    Alignment = 6,
    Program = 7,
    Decrementer = 9,
}

#[repr(C, align(4096))]
#[derive(Copy, Clone)]
struct CpuContext {
    r: [u64; 32],
    lr: u64,
    ctr: u64,
    srr0: u64,
    srr1: u64,
}

impl CpuContext {
    const fn new() -> Self {
        Self {
            r: [0u64; 32],
            lr: 0,
            ctr: 0,
            srr0: 0,
            srr1: 0,
        }
    }
}

#[no_mangle]
static mut EXCEPTION_SAVE_AREA: [CpuContext; 6] = [CpuContext::new(); 6];

#[no_mangle]
extern "C" fn except(id: ExceptionType, save_area: &mut CpuContext) {
    match id {
        ExceptionType::ExternalInterrupt => {
            let iic = unsafe { Iic::local() };
        }
        
        _ => loop {}
    }

    load_context(save_area);
}

fn load_context(ctx: &CpuContext) -> ! {
    unsafe {
        asm!(
            "ld     %r0, 256(%r4)",
            "mtlr   %r0",
            "ld     %r0, 264(%r4)",
            "mtctr  %r0",
            "ld     %r0, 272(%r4)",
            "mtsrr0 %r0",
            "ld     %r0, 280(%r4)",
            "mtsrr1 %r0",
            "ld     %r0, 0(%r4)",
            "ld     %r1, 8(%r4)",
            "ld     %r2, 16(%r4)",
            "ld     %r3, 24(%r4)",
            "ld     %r4, 32(%r4)",
            "ld     %r5, 40(%r4)",
            "ld     %r6, 48(%r4)",
            "ld     %r7, 56(%r4)",
            "ld     %r8, 64(%r4)",
            "ld     %r9, 72(%r4)",
            "ld     %r10, 80(%r4)",
            "ld     %r11, 88(%r4)",
            "ld     %r12, 96(%r4)",
            "ld     %r13, 104(%r4)",
            "ld     %r14, 112(%r4)",
            "ld     %r15, 120(%r4)",
            "ld     %r16, 128(%r4)",
            "ld     %r17, 136(%r4)",
            "ld     %r18, 144(%r4)",
            "ld     %r19, 152(%r4)",
            "ld     %r20, 160(%r4)",
            "ld     %r21, 168(%r4)",
            "ld     %r22, 176(%r4)",
            "ld     %r23, 184(%r4)",
            "ld     %r24, 192(%r4)",
            "ld     %r25, 200(%r4)",
            "ld     %r26, 208(%r4)",
            "ld     %r27, 216(%r4)",
            "ld     %r28, 224(%r4)",
            "ld     %r29, 232(%r4)",
            "ld     %r30, 240(%r4)",
            "ld     %r31, 248(%r4)",
            "rfid",
            in("4") ctx,
            options(noreturn),
        );
    }
}

#[naked]
unsafe extern "C" fn except_thunk() {
    asm!(
        "mtctr  %r4",       // Reload CTR with original value
        "mfspr  %r4, 1023", // r4 = PIR
        "sldi   %r4, %r4, 32 + 12",
        "oris   %r4, %r4, EXCEPTION_SAVE_AREA@highest",
        "ori    %r4, %r4, EXCEPTION_SAVE_AREA@higher",
        "rldicr %r4, %r4, 32, 63",
        "oris   %r4, %r4, EXCEPTION_SAVE_AREA@high",
        "ori    %r4, %r4, EXCEPTION_SAVE_AREA@l",
        // Now save registers.
        "std    %r0, 0(%r4)",
        "std    %r1, 8(%r4)",
        "subi   %r1, %r1, 0x100", // HACK: Subtract 0x100 bytes from r1 to reuse the stack.
        "std    %r2, 16(%r4)",
        "mfspr  %r0, 304", // Reload R3, which was saved in HPSRG0.
        "std    %r0, 24(%r4)",
        "mfspr  %r0, 305", // Reload R4, which was saved in HSPRG1.
        "std    %r0, 32(%r4)",
        "std    %r5, 40(%r4)",
        "std    %r6, 48(%r4)",
        "std    %r7, 56(%r4)",
        "std    %r8, 64(%r4)",
        "std    %r9, 72(%r4)",
        "std    %r10, 80(%r4)",
        "std    %r11, 88(%r4)",
        "std    %r12, 96(%r4)",
        "std    %r13, 104(%r4)",
        "std    %r14, 112(%r4)",
        "std    %r15, 120(%r4)",
        "std    %r16, 128(%r4)",
        "std    %r17, 136(%r4)",
        "std    %r18, 144(%r4)",
        "std    %r19, 152(%r4)",
        "std    %r20, 160(%r4)",
        "std    %r21, 168(%r4)",
        "std    %r22, 176(%r4)",
        "std    %r23, 184(%r4)",
        "std    %r24, 192(%r4)",
        "std    %r25, 200(%r4)",
        "std    %r26, 208(%r4)",
        "std    %r27, 212(%r4)",
        "std    %r28, 220(%r4)",
        "std    %r29, 228(%r4)",
        "std    %r30, 236(%r4)",
        "std    %r31, 244(%r4)",
        "mflr   %r0",
        "std    %r0, 252(%r4)",
        "mfctr  %r0",
        "std    %r0, 260(%r4)",
        "mfsrr0 %r0",
        "std    %r0, 268(%r4)",
        "mfsrr1 %r0",
        "std    %r0, 276(%r4)",
        "bl     except",
        options(noreturn)
    );
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
        (0x38600000 | (id as u32)),                      // li      %r3, id
        0x7C6903A6,                                      // mtctr   %r3
        0x4E800420,                                      // bctr
    ]
}

pub unsafe fn init_except() {
    let buf = make_longjmp_exc(5, except_thunk as usize);
    core::ptr::copy_nonoverlapping(buf.as_ptr(), 0x00000500 as *mut u32, buf.len());
}
