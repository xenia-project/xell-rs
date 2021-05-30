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
use xenon_cpu::{mfspr, mtspr};

global_asm!(include_str!("startup.s"));

extern crate core_reqs;

// mod gdb;
mod except;
mod panic;

use xenon_soc::{smc, uart};

static PROCESSORS: AtomicU32 = AtomicU32::new(0);
static SECONDARY_BRANCH_TARGET: AtomicUsize = AtomicUsize::new(0);

extern "C" {
    // fn start_from_rom() -> !;
    fn start_from_libxenon() -> !;
}

macro_rules! writeln {
    ($($tts:tt)*) => {
        uart::UART.lock(|mut uart| {
            ufmt::uwriteln!(&mut uart, $($tts)*).unwrap();
        });
    };
}

macro_rules! write {
    ($($tts:tt)*) => {
        uart::UART.lock(|mut uart| {
            ufmt::uwrite!(&mut uart, $($tts)*).unwrap();
        });
    };
}

const fn make_longjmp(target: usize) -> [u32; 12] {
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

fn read_line(uart: &mut uart::UART, line: &mut [u8]) -> usize {
    let mut n = 0usize;

    while n < line.len() {
        match uart.read_byte() {
            b'\r' => {
                uart.write(b"\r\n");
                break;
            }

            // Backspace.
            0x08 => {
                if n != 0 {
                    // Clear the character from the screen.
                    uart.write(b"\x08 \x08");

                    line[n] = b'\0';
                    n -= 1;
                }
            }

            byte => {
                uart.write_byte(byte);

                line[n] = byte;
                n += 1;
            }
        }
    }

    n
}

fn serial_terminal() {
    let mut buf = [0u8; 1024];
    loop {
        write!("\n> ");

        let n = uart::UART.lock(|mut uart| read_line(&mut uart, &mut buf));

        let line = match core::str::from_utf8(&buf[..n]) {
            Ok(l) => l,
            Err(_) => continue,
        };

        let mut args = line.split(" ");
        match args.next() {
            Some("r64") => {
                let addr = {
                    let addr_str = match args.next() {
                        Some(a) => a,
                        None => {
                            writeln!("r64 <address>");
                            continue;
                        }
                    };

                    match u64::from_str_radix(addr_str, 16) {
                        Ok(n) => n,
                        Err(_) => {
                            writeln!("invalid address");
                            continue;
                        }
                    }
                };

                let val = unsafe { core::ptr::read_volatile(addr as *const u64) };
                writeln!("{:#?}", val);
            }

            Some("w64") => {
                let addr = {
                    let addr_str = match args.next() {
                        Some(a) => a,
                        None => {
                            writeln!("w64 <address> <val>");
                            continue;
                        }
                    };

                    match u64::from_str_radix(addr_str, 16) {
                        Ok(n) => n,
                        Err(_) => {
                            writeln!("invalid address");
                            continue;
                        }
                    }
                };

                let val = {
                    let val_str = match args.next() {
                        Some(a) => a,
                        None => {
                            writeln!("w64 <address> <val>");
                            continue;
                        }
                    };

                    match u64::from_str_radix(val_str, 16) {
                        Ok(n) => n,
                        Err(_) => {
                            writeln!("invalid value");
                            continue;
                        }
                    }
                };

                unsafe {
                    core::ptr::write_volatile(addr as *mut u64, val);
                }
            }

            Some("mthrmor") => {
                let val = {
                    let val_str = match args.next() {
                        Some(s) => s,
                        None => {
                            writeln!("mthrmor <val>");
                            continue;
                        }
                    };

                    match u64::from_str_radix(val_str, 16) {
                        Ok(n) => n,
                        Err(_) => {
                            writeln!("invalid address");
                            continue;
                        }
                    }
                };

                unsafe { mtspr!(313, val) };
            }

            Some("reboot") => {
                writeln!("Rebooting system...");
                smc::SMC.lock(|smc| {
                    smc.send_message(&[
                        0x82043000u32,
                        0x00000000u32,
                        0x00000000u32,
                        0x00000000u32,
                    ]);
                });
            }

            Some("except") => {
                writeln!("If you say so...");
                unsafe {
                    except::test_take_exception();
                }
            }

            Some("ping") => {
                writeln!("pong");
            }

            Some("boot") => {
                writeln!("Booting...");
                return;
            }

            Some("") => {}

            Some(&_) => {
                writeln!("Unknown command!");
            }

            None => {}
        }
    }
}

#[no_mangle]
#[link_section = ".text.startup"]
pub extern "C" fn __start_rust(pir: u64, src: u32, msr: u64, hrmor: u64, pvr: u64, lpcr: u64) -> ! {
    uart::UART.lock(|uart| {
        if pir == 0 {
            uart.reset(uart::Speed::S115200);
        }

        let toc = unsafe {
            let toc: u64;
            asm!(
                "mr {}, %r2",
                out(reg) toc
            );

            toc
        };

        ufmt::uwriteln!(uart, "Hello from processor {:#?}!", pir).unwrap();
        ufmt::uwriteln!(uart, "PIR:   {:#?}", pir).unwrap();
        ufmt::uwriteln!(uart, "MSR:   {:#?}", msr).unwrap();
        ufmt::uwriteln!(uart, "HRMOR: {:#?}", hrmor).unwrap();
        ufmt::uwriteln!(uart, "RMOR:  {:#?}", unsafe { mfspr!(312) }).unwrap();
        ufmt::uwriteln!(uart, "LPCR:  {:#?}", lpcr).unwrap();
        ufmt::uwriteln!(uart, "LPIDR: {:#?}", unsafe { mfspr!(319) }).unwrap();
        ufmt::uwriteln!(uart, "PVR:   {:#?}", pvr).unwrap();
        ufmt::uwriteln!(uart, "HDEC:  {:#?}", unsafe { mfspr!(310) }).unwrap();
        ufmt::uwriteln!(uart, "DEC:   {:#?}", unsafe { mfspr!(22) }).unwrap();
        ufmt::uwriteln!(uart, "SRC:   {:#?}", src).unwrap();
        ufmt::uwriteln!(uart, "TOC:   {:#?}", toc).unwrap();
    });

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

    unsafe {
        // HRMOR = 0
        // N.B: We can only modify HRMOR when other threads are captured!
        // mtspr!(313, 0x00000000);
        except::init_except();
    }

    serial_terminal();

    match src {
        // Startup from ROM
        /*
        0 => {
            write!("Startup from ROM.\n");

            // Setup a jump vector to branch to our startup code.
            let jmpbuf = make_longjmp(start_from_rom as usize);

            unsafe {
                core::ptr::copy_nonoverlapping(
                    jmpbuf.as_ptr(),
                    0x80000000_00000100usize as *mut u32,
                    jmpbuf.len(),
                );
            }

            // Startup the secondary thread.
            unsafe {
                // CTRL.TE{0,1} = 0b11
                mtspr!(152, 0x00C0_0000);
            }
        }
        */
        // Startup from OS (1)
        // HACK: Also going to apply this path for startup from ROM for development.
        0 | 1 => {
            writeln!("Startup from OS.");

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
                core::ptr::write_volatile(
                    0x00000500 as *mut u32,
                    make_reljump(0x00000500usize, 0x000000A0 as usize),
                );

                // Ditto for the decrementer.
                core::ptr::write_volatile(
                    0x00000900 as *mut u32,
                    make_reljump(0x00000900usize, 0x000000A0 as usize),
                );

                // Ensure the compiler does not reorder instructions.
                core::sync::atomic::compiler_fence(Ordering::SeqCst);

                writeln!(
                    "External interrupt vector overwritten. Triggering IPI on all other cores."
                );

                // Set the IRQL on all other processors to 0 (to unmask all interrupts).
                // The hypervisor isn't going to like this, but we set a detour on the interrupt vector earlier.
                for i in 1usize..6usize {
                    let ptr = (0x8000_0200_0005_0000 + (i * 0x1000)) as *mut u64;
                    ptr.offset(1).write_volatile(0);
                }

                // Trigger an IPI on all other processors, with vector 0x78.
                core::ptr::write_volatile(0x8000_0200_0005_0010 as *mut u64, 0x003E_0078);
            };

            writeln!("IPI in progress. Waiting...");

            // Loop...
            while PROCESSORS.load(Ordering::Relaxed) != 0x3F {
                write!(
                    "Waiting for other processors... {:#?}  \r",
                    PROCESSORS.load(Ordering::Relaxed)
                );

                smc::SMC.lock(|smc| {
                    smc.set_led(true, PROCESSORS.load(Ordering::Relaxed) as u8);
                });

                xenon_cpu::time::delay(core::time::Duration::from_millis(20));
            }

            writeln!("Processors captured.");
        }

        // Shouldn't hit this case.
        _ => loop {},
    }

    smc::SMC.lock(|smc| {
        // Flash all green LEDs.
        smc.set_led(true, 0xF0);
    });

    writeln!("System captured.");

    loop {}
}
