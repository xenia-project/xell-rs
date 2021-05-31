#[inline]
pub fn mftb() -> u128 {
    let mut tbu: u64;
    let mut tbl: u64;
    let mut tbu2: u64;

    loop {
        unsafe {
            asm!(
                "mftbu {0}",
                "mftb {1}",
                "mftbu {2}",

                out(reg) tbu,
                out(reg) tbl,
                out(reg) tbu2,
            );
        }

        // Finished loading if the upper timebase did not change.
        if tbu == tbu2 {
            break;
        }
    }

    (tbu as u128) << 64 | tbl as u128
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

#[inline]
pub unsafe fn mtmsr(msr: u64) {
    asm!("mtmsrd {}, 0", in(reg) msr);
}

#[inline]
pub unsafe fn mtmsrl(msr: u64) {
    asm!("mtmsrd {}, 1", in(reg) msr);
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
