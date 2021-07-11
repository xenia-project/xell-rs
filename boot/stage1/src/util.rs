//! This file defines utility functionality.

#[allow(dead_code)]
pub const fn bit(b: u64) -> u64 {
    0x8000_0000_0000_0000 >> b
}

#[allow(dead_code)]
pub const fn make_longjmp(target: usize, p1: u64) -> [u32; 17] {
    [
        (0x3C600000 | ((target >> 48) & 0xFFFF)) as u32, // lis     %r3, target[64:48]
        (0x60630000 | ((target >> 32) & 0xFFFF)) as u32, // ori     %r3, %r3, target[48:32]
        0x786307C6,                                      // rldicr  %r3, %r3, 32, 31
        (0x64630000 | ((target >> 16) & 0xFFFF)) as u32, // oris    %r3, %r3, target[32:16]
        (0x60630000 | ((target >> 00) & 0xFFFF)) as u32, // ori     %r3, %r3, target[16:0]
        0x7C7A03A6,                                      // mtsrr0  %r3
        // Clear MSR[EE/IR/DR]
        0x3c800000, // lis      %r4, 0x0000
        0x60848030, // ori      %r4, %r4, 0x8030
        0x7C6000A6, // mfmsr    %r3
        0x7C632078, // andc     %r3, %r3, %r4
        0x7C7B03A6, // mtsrr1   %r3
        // Load the parameter.
        (0x3C600000 | ((p1 >> 48) & 0xFFFF)) as u32, // lis     %r3, p1[64:48]
        (0x60630000 | ((p1 >> 32) & 0xFFFF)) as u32, // ori     %r3, %r3, p1[48:32]
        0x786307C6,                                  // rldicr  %r3, %r3, 32, 31
        (0x64630000 | ((p1 >> 16) & 0xFFFF)) as u32, // oris    %r3, %r3, p1[32:16]
        (0x60630000 | ((p1 >> 00) & 0xFFFF)) as u32, // ori     %r3, %r3, p1[16:0]
        // Branch to target.
        0x4C000024, // rfid
    ]
}

/// Calculate |a - b|.
pub const fn abs_diff(a: usize, b: usize) -> usize {
    if a > b {
        a - b
    } else {
        b - a
    }
}

/// Make an appropriate branch opcode located at `address` that
/// jumps to `target`.
///
/// This routine will panic if the delta is too large to be represented
/// with a single branch instruction.
pub const fn make_reljump(address: usize, target: usize) -> u32 {
    let diff = abs_diff(target, address);
    let offset = target.wrapping_sub(address);

    // If the offset can fit within a single branch instruction, use it.
    if diff < 0x7F_FFFF {
        (0x4800_0000 | (offset & 0x00FF_FFFC)) as u32
    } else {
        panic!("Offset too large for relative jump!");
    }
}

/// Create an address suitable for loading using signed arithmetic,
/// i.e:
///
/// ```asm
/// lis %rX, <addr>@ha
/// addi %rX, <addr>@l
/// ```
pub const fn make_arithaddr(addr: u32) -> (u16, u16) {
    let lo = (addr & 0xFFFF) as u16;
    let hi = { ((addr >> 16) as u16) + if (lo & 0x8000) != 0 { 1 } else { 0 } };

    (hi, lo)
}
