#[inline]
pub fn mftb() -> u64 {
    let tb;
    unsafe {
        asm!("mftb {}", out(reg) tb);
    }

    tb
}

#[inline]
pub fn pir() -> u64 {
    let pir;
    unsafe {
        asm!("mfspr {}, 1023", out(reg) pir);
    }

    pir
}
