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

use atomic::Atomic;
use core::{
    fmt::Write,
    sync::atomic::{AtomicU32, Ordering},
};
use xenon_cpu::{
    intrin::{mfmsr, mtmsrl},
    mfspr,
};
use xenon_soc::{smc, uart};

extern crate core_reqs;

// mod gdb;
mod except;
mod panic;

use except::ExceptionType;

global_asm!(include_str!("startup.s"));

static PROCESSORS: AtomicU32 = AtomicU32::new(0);

macro_rules! println {
    ($($tts:tt)*) => {
        uart::UART.lock(|mut uart| {
            core::writeln!(&mut uart, $($tts)*).unwrap();
        });
    };
}

macro_rules! print {
    ($($tts:tt)*) => {
        uart::UART.lock(|mut uart| {
            core::write!(&mut uart, $($tts)*).unwrap();
        });
    };
}

#[allow(dead_code)]
const fn bit(b: u64) -> u64 {
    0x8000_0000_0000_0000 >> b
}

#[allow(dead_code)]
const fn make_longjmp(target: usize, p1: u64) -> [u32; 17] {
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

#[allow(dead_code)]
const fn abs_diff(a: usize, b: usize) -> usize {
    if a > b {
        a - b
    } else {
        b - a
    }
}

#[allow(dead_code)]
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
        print!("\n> ");

        let n = uart::UART.lock(|mut uart| read_line(&mut uart, &mut buf));

        let line = match core::str::from_utf8(&buf[..n]) {
            Ok(l) => l,
            Err(_) => continue,
        };

        let mut args = line.split(' ');
        match args.next() {
            Some("r64") => {
                let addr = {
                    let addr_str = match args.next() {
                        Some(a) => a,
                        None => {
                            println!("r64 <address>");
                            continue;
                        }
                    };

                    match u64::from_str_radix(addr_str, 16) {
                        Ok(n) => n,
                        Err(_) => {
                            println!("invalid address");
                            continue;
                        }
                    }
                };

                let val = unsafe { core::ptr::read_volatile(addr as *const u64) };
                println!("{:016X}", val);
            }

            Some("w64") => {
                let addr = {
                    let addr_str = match args.next() {
                        Some(a) => a,
                        None => {
                            println!("w64 <address> <val>");
                            continue;
                        }
                    };

                    match u64::from_str_radix(addr_str, 16) {
                        Ok(n) => n,
                        Err(_) => {
                            println!("invalid address");
                            continue;
                        }
                    }
                };

                let val = {
                    let val_str = match args.next() {
                        Some(a) => a,
                        None => {
                            println!("w64 <address> <val>");
                            continue;
                        }
                    };

                    match u64::from_str_radix(val_str, 16) {
                        Ok(n) => n,
                        Err(_) => {
                            println!("invalid value");
                            continue;
                        }
                    }
                };

                unsafe {
                    core::ptr::write_volatile(addr as *mut u64, val);
                }
            }

            Some("reboot") => {
                println!("Rebooting system...");
                smc::SMC.lock(|smc| {
                    smc.restart_system();
                });
            }

            Some("except") => {
                println!("If you say so...");
                unsafe {
                    except::cause_exception();
                }
            }

            Some("ping") => {
                println!("pong");
            }

            Some("🍆") => {
                println!(";)");
            }

            Some("exit") => {
                println!("Goodbye!");
                return;
            }

            Some("") => {}

            Some(cmd) => {
                println!("Unknown command \"{}\"!", cmd);
            }

            None => {}
        }
    }
}

fn startup_exception_handler(ex: ExceptionType, ctx: &mut except::CpuContext) -> Result<(), ()> {
    let pir = xenon_cpu::intrin::pir();
    uart::UART.lock(|uart| {
        let sp = unsafe {
            let sp: u64;
            asm!(
                "mr {}, %r1",
                out(reg) sp
            );

            sp
        };

        let toc = unsafe {
            let toc: u64;
            asm!(
                "mr {}, %r2",
                out(reg) toc
            );

            toc
        };

        core::writeln!(uart, "Exception on processor {:#?}!", pir).unwrap();
        core::writeln!(uart, "EXC:   {:?}", ex).unwrap();
        core::writeln!(uart, "MSR:   {:016X}", xenon_cpu::intrin::mfmsr()).unwrap();
        core::writeln!(uart, "LPCR:  {:016X}", unsafe { mfspr!(318) }).unwrap();
        core::writeln!(uart, "LPIDR: {:016X}", unsafe { mfspr!(319) }).unwrap();
        core::writeln!(uart, "HDEC:  {:016X}", unsafe { mfspr!(310) }).unwrap();
        core::writeln!(uart, "DEC:   {:016X}", unsafe { mfspr!(22) }).unwrap();
        core::writeln!(uart, "TOC:   {:016X}", toc).unwrap();
        core::writeln!(uart, "SP:    {:016X}", sp).unwrap();
        core::writeln!(uart, "CTX:\n{:>3?}", ctx).unwrap();
    });

    // Branch to thread entry.
    let context =
        except::CpuContext::with_hvcall(thread_entry as usize, 0x8000_0000_1E00_0000 - (pir << 16));
    unsafe {
        except::load_context(&context);
    }
}

fn normal_exception_handler(ex: ExceptionType, _ctx: &mut except::CpuContext) -> Result<(), ()> {
    match ex {
        ExceptionType::ExternalInterrupt => Ok(()),

        _ => Err(()),
    }
}

fn exception_handler(ex: ExceptionType, ctx: &mut except::CpuContext) -> Result<(), ()> {
    match EXCEPTION_HANDLER_MODE.load(Ordering::Relaxed) {
        ExceptionMode::Startup => startup_exception_handler(ex, ctx),
        ExceptionMode::Normal => normal_exception_handler(ex, ctx),
    }
}

#[derive(Clone, Copy)]
enum ExceptionMode {
    Startup,
    Normal,
}

static EXCEPTION_HANDLER_MODE: Atomic<ExceptionMode> = Atomic::new(ExceptionMode::Startup);

fn thread_entry() -> ! {
    let pir = xenon_cpu::intrin::pir();
    PROCESSORS.fetch_or(1 << pir, Ordering::Relaxed);

    // Loop until all processors check in.
    while PROCESSORS.load(Ordering::Relaxed) != 0x3F {}

    // Enable external interrupts.
    unsafe {
        mtmsrl(bit(48));
    }

    if pir == 0 {
        serial_terminal();

        smc::SMC.lock(|smc| {
            smc.restart_system();
        });
    }

    loop {}
}

#[no_mangle]
#[link_section = ".text.startup"]
pub extern "C" fn __start_rust(
    pir: u64,
    src: u32,
    _msr: u64,
    hrmor: u64,
    pvr: u64,
    lpcr: u64,
) -> ! {
    unsafe {
        // Clear out the relocation routine written by startup.s
        core_reqs::memset(0x8000_0000_0000_0000 as *mut u8, 0x00, 0x100);

        // Disable all checkstops. Enable machine check exceptions.
        // Default: 0x07FFA7FE00000000
        core::ptr::write_volatile(0x8000_0200_0006_1060 as *mut u64, 0x0000_07FF_0000_0000);
    }

    uart::UART.lock(|uart| {
        if pir == 0 {
            uart.reset(uart::Speed::S115200);
        }

        let sp = unsafe {
            let sp: u64;
            asm!(
                "mr {}, %r1",
                out(reg) sp
            );

            sp
        };

        let toc = unsafe {
            let toc: u64;
            asm!(
                "mr {}, %r2",
                out(reg) toc
            );

            toc
        };

        core::writeln!(uart, "Hello from processor {:#?}!", pir).unwrap();
        core::writeln!(uart, "MSR:   {:016X}", mfmsr()).unwrap();
        core::writeln!(uart, "HRMOR: {:016X}", hrmor).unwrap();
        core::writeln!(uart, "RMOR:  {:016X}", unsafe { mfspr!(312) }).unwrap();
        core::writeln!(uart, "LPCR:  {:016X}", lpcr).unwrap();
        core::writeln!(uart, "LPIDR: {:016X}", unsafe { mfspr!(319) }).unwrap();
        core::writeln!(uart, "PVR:   {:016X}", pvr).unwrap();
        core::writeln!(uart, "HDEC:  {:016X}", unsafe { mfspr!(310) }).unwrap();
        core::writeln!(uart, "DEC:   {:016X}", unsafe { mfspr!(22) }).unwrap();
        core::writeln!(uart, "TOC:   {:016X}", toc).unwrap();
        core::writeln!(uart, "SP:    {:016X}", sp).unwrap();
        core::writeln!(uart, "SRC:   {:016X}", src).unwrap();
    });

    unsafe {
        except::init_except(Some(exception_handler));
    }

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
            println!("Startup from OS.");

            // We'll need to catch all other cores that may still be running the OS.
            // Set a branch on the external interrupt vector, and trigger an IPI.
            println!("Triggering IPI on all other cores.");

            // Loop...
            while PROCESSORS.load(Ordering::Relaxed) != 0x3E {
                print!(
                    "Waiting for other processors... {:02X}  \r",
                    PROCESSORS.load(Ordering::Relaxed)
                );

                smc::SMC.lock(|smc| {
                    smc.set_led(true, PROCESSORS.load(Ordering::Relaxed) as u8);
                });

                unsafe {
                    // Set the IRQL on all other processors to 0 (to unmask all interrupts).
                    // The hypervisor isn't going to like this, but we set a detour on the interrupt vector earlier.
                    for i in 1usize..6usize {
                        let ptr = (0x8000_0200_0005_0000 + (i * 0x1000)) as *mut u64;
                        ptr.offset(1).write_volatile(0);
                    }

                    // Trigger an IPI on all other processors, with vector 30.
                    core::ptr::write_volatile(0x8000_0200_0005_0010 as *mut u64, 0x003F_0078);
                }

                xenon_cpu::time::delay(core::time::Duration::from_secs(1));
            }

            println!("Processors captured.");
        }

        // Shouldn't hit this case.
        _ => loop {},
    }

    smc::SMC.lock(|smc| {
        // Flash all green LEDs.
        smc.set_led(true, 0xF0);
    });

    EXCEPTION_HANDLER_MODE.store(ExceptionMode::Normal, Ordering::Relaxed);
    println!("System captured.");

    PROCESSORS.fetch_or(1 << pir, Ordering::Relaxed);

    // Branch to thread entry.
    let context =
        except::CpuContext::with_hvcall(thread_entry as usize, 0x8000_0000_1E00_0000 - (pir << 16));
    unsafe {
        except::load_context(&context);
    }
}
