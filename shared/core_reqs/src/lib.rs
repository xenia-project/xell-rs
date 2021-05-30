//! Requirements for Rust libcore. These are just basic libc `mem*()` routines
//! as well as some intrinsics to get access to 64-bit integers in 32-bit land
//!
//! This code is from [chocolate milk](https://github.com/gamozolabs/chocolate_milk/blob/643f47b901ceda1f688d3c20ff92b0f41af80251/shared/core_reqs/src/lib.rs).

#![feature(global_asm, llvm_asm)]
#![no_std]

/// libc `memcpy` implementation in Rust
///
/// This implementation of `memcpy` is overlap safe, making it technically
/// `memmove`.
///
/// # Parameters
///
/// * `dest` - Pointer to memory to copy to
/// * `src`  - Pointer to memory to copy from
/// * `n`    - Number of bytes to copy
///
/// # Safety
/// This function copies raw memory! You must make sure the two areas point to valid memory.
/// In addition, you MUST make sure `src` and `dst` do not overlap!
#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    memmove(dest, src, n)
}

/// libc `memmove` implementation in Rust
///
/// # Parameters
///
/// * `dest` - Pointer to memory to copy to
/// * `src`  - Pointer to memory to copy from
/// * `n`    - Number of bytes to copy
///
/// # Safety
/// This function copies raw memory! You must make sure the two areas point to valid memory.
#[no_mangle]
pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if src < dest as *const u8 {
        // copy backwards
        let mut ii = n;
        while ii != 0 {
            ii -= 1;
            *dest.add(ii) = *src.add(ii);
        }
    } else {
        // copy forwards
        let mut ii = 0;
        while ii < n {
            *dest.add(ii) = *src.add(ii);
            ii += 1;
        }
    }

    dest
}

/// libc `memset` implementation in Rust
///
/// # Parameters
///
/// * `s` - Pointer to memory to set
/// * `c` - Character to set `n` bytes in `s` to
/// * `n` - Number of bytes to set
///
/// # Safety
/// This function modifies raw memory! You must make sure the `s` points to valid memory.
#[no_mangle]
#[cfg(target_arch = "powerpc64")]
pub unsafe extern "C" fn memset(s: *mut u8, c: i32, n: usize) -> *mut u8 {
    if n == 0 {
        return s;
    }

    let mut ii = n;
    while ii != 0 {
        ii -= 1;
        *s.add(ii) = c as u8;
    }

    s
}

/// libc `memset` implementation in Rust
///
/// # Parameters
///
/// * `s` - Pointer to memory to set
/// * `c` - Character to set `n` bytes in `s` to
/// * `n` - Number of bytes to set
///
#[no_mangle]
#[cfg(target_arch = "x86")]
pub unsafe extern "C" fn memset(s: *mut u8, c: i32, n: usize) -> *mut u8 {
    if n == 0 {
        return s;
    }

    llvm_asm!(r#"
        rep stosb
    "# :: "{edi}"(s), "{ecx}"(n), "{eax}"(c) : "memory", "edi", "ecx", "eax" :
    "volatile", "intel");

    s
}

/// libc `memset` implementation in Rust
///
/// # Parameters
///
/// * `s` - Pointer to memory to set
/// * `c` - Character to set `n` bytes in `s` to
/// * `n` - Number of bytes to set
///
#[no_mangle]
#[cfg(target_arch = "x86_64")]
pub unsafe extern "C" fn memset(s: *mut u8, c: i32, n: usize) -> *mut u8 {
    if n == 0 {
        return s;
    }

    llvm_asm!(r#"
        rep stosb
    "# :: "{rdi}"(s), "{rcx}"(n), "{eax}"(c) : "memory", "rdi", "rcx", "eax" :
    "volatile", "intel");

    s
}

/// libc `memcmp` implementation in Rust
///
/// # Parameters
///
/// * `s1` - Pointer to memory to compare with s2
/// * `s2` - Pointer to memory to compare with s1
/// * `n`  - Number of bytes to set
///
/// # Safety
/// This function is generally safe to use as long as the raw memory
/// is accessible.
#[no_mangle]
pub unsafe extern "C" fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    let mut ii = 0;
    while ii < n {
        let a = *s1.add(ii);
        let b = *s2.add(ii);
        if a != b {
            return a as i32 - b as i32;
        }
        ii += 1;
    }

    0
}

/// libc `bcmp` implementation in Rust
///
/// # Parameters
///
/// * `s1` - Pointer to memory to compare with s2
/// * `s2` - Pointer to memory to compare with s1
/// * `n`  - Number of bytes to compare
///
/// # Safety
/// This function is generally safe to use as long as the raw memory
/// is accessible.
#[no_mangle]
pub unsafe extern "C" fn bcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    let mut ii = 0;
    while ii < n {
        let a = *s1.add(ii);
        let b = *s2.add(ii);
        if a != b {
            return 1;
        }
        ii += 1;
    }

    0
}

// Making a fake __CxxFrameHandler3 in Rust causes a panic, this is hacky
// workaround where we declare it as a function that will just crash if it.
// We should never hit this so it doesn't matter.
#[cfg(target_arch = "x86_64")]
global_asm!(
    r#"
    .global __CxxFrameHandler3
    __CxxFrameHandler3:
        ud2
"#
);

/// Whether or not floats are used. This is used by the MSVC calling convention
/// and it just has to exist.
#[export_name = "_fltused"]
pub static FLTUSED: usize = 0;
