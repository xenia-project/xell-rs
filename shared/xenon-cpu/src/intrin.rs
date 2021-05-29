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

#[inline]
pub fn mfmsr() -> u64 {
    let msr;
    unsafe {
        asm!("mfmsr {}", out(reg) msr);
    }

    msr
}

#[macro_export]
macro_rules! mtspr {
    ($spr:literal, $val:expr) => {
        asm!(
            "mtspr {spr}, {0}",
            in(reg_nonzero) $val,
            spr = const $spr,
        )
    };
}

#[macro_export]
macro_rules! mfspr {
    ($spr:literal) => {
        {
            let mut val = 0u64;
            asm!(
                "mfspr {0}, {spr}",
                out(reg_nonzero) val,
                spr = const $spr,
            );

            val
        }
    };
}
