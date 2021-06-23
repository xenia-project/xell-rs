//! Integrated Interrupt Controller (IIC)

const IIC_BASE: u64 = 0x80000200_00050000;

#[repr(usize)]
#[allow(dead_code)]
enum Register {
    WhoAmI = 0x00,
    CurrentTaskPriority = 0x08,
    IpiDispatch = 0x10,
    ActiveIrqs = 0x20,
    AssertedIrqs = 0x28,
    Ack = 0x50,
    AckPri = 0x58,
    Eoi = 0x60,
    EoiPri = 0x68,
    SpuriousVector = 0x70,
}

#[repr(u8)]
#[allow(dead_code)]
#[derive(ufmt::derive::uDebug)]
pub enum Interrupt {
    Ipi4 = 2,
    Ipi3 = 4,
    Smm = 5,
    Sfcx = 6,
    SataHdd = 8,
    SataCdrom = 9,
    Ohci0 = 11,
    Ehci0 = 12,
    Ohci1 = 13,
    Ehci1 = 14,
    Xma = 16,
    Audio = 17,
    Enet = 19,
    Xps = 21,
    Graphics = 22,
    Profiler = 24,
    Biu = 25,
    Ioc = 26,
    Fsb = 27,
    Ipi2 = 28,
    Clock = 29,
    Ipi1 = 30,
    None = 31,
}

pub struct Iic {
    mmio: &'static mut [u8],
}

#[allow(dead_code)]
impl Iic {
    pub fn local() -> Self {
        let id = xenon_cpu::intrin::pir();
        let base = IIC_BASE + (0x1000 * id);

        // SAFETY: It should always be safe to get a pointer to the current CPU's
        // interrupt controller.
        Self {
            mmio: unsafe { core::slice::from_raw_parts_mut(base as *mut _, 0x1000) },
        }
    }

    fn write<T>(&self, reg: Register, val: T) {
        unsafe {
            core::ptr::write_volatile(&self.mmio[reg as usize] as *const _ as *mut T, val);
        }
    }

    fn read<T>(&self, reg: Register) -> T {
        unsafe { core::ptr::read_volatile(&self.mmio[reg as usize] as *const _ as *mut T) }
    }

    /// Acknowledge and get an interrupt (if one is pending).
    pub fn acknowledge(&self) -> Option<Interrupt> {
        let raw_int = self.read::<u64>(Register::Ack) >> 2;
        let spv = self.read::<u64>(Register::SpuriousVector) >> 2;

        if raw_int == spv {
            None
        } else {
            unsafe { core::mem::transmute((raw_int & 0x1F) as u8) }
        }
    }

    /// Signal the end of an IIC interrupt.
    pub fn eoi(&self, int: Interrupt) {
        let raw_int = int as u32;
        self.write::<u64>(Register::Eoi, (raw_int as u64) << 2);
    }

    pub fn set_priority(&self, prio: Interrupt) {
        self.write(Register::CurrentTaskPriority, (prio as u64) << 2);
        self.read::<u64>(Register::CurrentTaskPriority);
    }
}
