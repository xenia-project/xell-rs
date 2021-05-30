#![no_std]

use core::{marker::PhantomData, pin::Pin, ptr::NonNull, time::Duration};
use smoltcp::phy::{self, Device};

#[allow(dead_code)]
#[repr(u32)]
enum Register {
    TxConfig = 0x00,
    TxDescriptorBase = 0x04,
    TxDescriptorStatus = 0x0C,
    RxConfig = 0x10,
    RxDescriptorBase = 0x14,
    InterruptStatus = 0x20,
    InterruptMask = 0x24,
    Config0 = 0x28,
    Power = 0x30,
    PhyConfig = 0x40,
    PhyControl = 0x44,
    Config1 = 0x50,
    RetryCount = 0x54,
    MulticastFilterControl = 0x60,
    Address0 = 0x62,
    MulticastHigh = 0x68,
    MaxPacketSize = 0x78,
    Address1 = 0x7A,
}

/// The type of a transmission ring.
pub enum RingType {
    Rx,
    Tx,
}

// Flag bit guesses:
// 0x8000_0000: Hardware ownership bit
// 0x4000_0000: ??
// 0x0020_0000: (TX) last buffer?
// 0x0002_0000: (TX) interrupt related?
// 0x0001_0000: (TX) interrupt related?

const HWDESC_FLAG_HW_OWNED: u32 = 0x80000000;
const HWDESC_LAST_ENTRY: u32 = 0x80000000; // N.B: This is set in the `capacity` field.

/// Transfer descriptor, as defined by hardware.
#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct HwDescriptor {
    len: u32,
    flags: u32,
    addr: u32,
    capacity: u32,
}

impl HwDescriptor {
    pub fn new() -> Self {
        Self {
            len: 0,
            flags: 0,
            addr: 0,
            capacity: 0,
        }
    }

    pub fn is_free(&self) -> bool {
        (self.flags & HWDESC_FLAG_HW_OWNED) == 0
    }
}

pub struct EthernetPendingDesc<'a>(&'a mut HwDescriptor, &'a mut [u8]);

pub struct EthernetDescBuilder<'a>(&'a mut HwDescriptor, &'a mut [u8]);

impl<'a> EthernetDescBuilder<'a> {
    fn new(desc: &'a mut HwDescriptor, buf: &'static mut [u8]) -> Self {
        desc.capacity = (desc.capacity & 0x80000000) | ((buf.len() as u32) & 0x7FFFFFFF);
        desc.addr = buf.as_mut_ptr() as u32;

        Self(desc, buf)
    }

    pub fn set_flags(self, flags: u32) -> Self {
        self.0.flags = flags;

        self
    }

    pub fn commit(self) -> EthernetPendingDesc<'a> {
        self.0.flags |= HWDESC_FLAG_HW_OWNED;
        EthernetPendingDesc(self.0, self.1)
    }
}

/// Represents an ethernet descriptor that has pending data.
pub struct EthernetCompleteDesc<'a>(&'a mut HwDescriptor);

/// This structure represents a ring of DMA buffer descriptors for a Xenon MAC.
pub struct EthernetRing<T: RingType, const N: usize> {
    ring_type: PhantomData<T>,

    descriptors: [HwDescriptor; N],

    /// Index of first busy buffer (or represents no busy buffers if equivalent to `avail`)
    busy: usize,
    /// Index of first free buffer
    avail: usize,
}

impl<const T: RingType, const N: usize> EthernetRing<T, N> {
    pub fn new() -> Self {
        let mut descriptors = [HwDescriptor::new(); N];
        descriptors.last_mut().unwrap().capacity = HWDESC_LAST_ENTRY;

        Self {
            descriptors: [HwDescriptor::new(); N],

            busy: 0,
            avail: 0,
        }
    }

    pub fn next_avail<'a>(&'a mut self, buf: &'static mut [u8]) -> Option<EthernetDescBuilder<'a>> {
        let desc = &mut self.descriptors[self.avail];

        if desc.is_free() {
            Some(EthernetDescBuilder::new(desc, buf))
        } else {
            None
        }
    }

    pub fn next_complete<'a>(&'a mut self) -> Option<EthernetCompleteDesc<'a, N>> {
        None
    }
}

#[repr(align(16))]
pub struct EthernetDevice<const N: usize, const M: usize> {
    mmio: core::ptr::NonNull<u8>,

    rx_ring: &'static mut EthernetRing<RingType::Rx, N>,
    tx_ring: &'static mut EthernetRing<RingType::Tx, M>,
}

impl<const N: usize, const M: usize> EthernetDevice<N, M> {
    /// Constructs a new [EthernetDevice].
    ///
    /// SAFETY: The caller _MUST_ ensure that there is only one instance
    /// of this object at a time. Multiple instances will cause undefined behavior.
    pub unsafe fn new(
        rx_ring: &'static mut EthernetRing<RingType::Rx, N>,
        tx_ring: &'static mut EthernetRing<RingType::Tx, M>,
    ) -> Self {
        let mut obj = Self {
            mmio: NonNull::new_unchecked(0x8000_0200_EA00_1400 as *mut u8),

            rx_ring,
            tx_ring,
        };

        obj.reset();
        obj
    }

    fn write<T>(&mut self, reg: Register, val: T) {
        // SAFETY: The access is bounded by Register, and cannot arbitrarily overflow.
        unsafe {
            core::ptr::write_volatile(self.mmio.as_ptr().offset(reg as isize) as *mut T, val);
        }
    }

    fn read<T>(&mut self, reg: Register) -> T {
        // SAFETY: The access is bounded by Register, and cannot arbitrarily overflow.
        unsafe { core::ptr::read_volatile(self.mmio.as_ptr().offset(reg as isize) as *mut T) }
    }

    pub fn reset(&mut self) {
        // Zero out the interrupt mask.
        self.write(Register::InterruptMask, 0x00000000);

        self.write(Register::Config0, 0x08558001);
        xenon_cpu::time::delay(Duration::from_micros(100));
        self.write(Register::Config0, 0x08550001);

        self.write(Register::PhyControl, 0x00000004);
        xenon_cpu::time::delay(Duration::from_micros(100));
        self.write(Register::PhyControl, 0x00000004);

        self.write(Register::MaxPacketSize, 1522);

        self.write(Register::Config1, 0x2360);

        self.write(Register::MulticastFilterControl, 0x0E38);

        // TODO: MAC address
    }
}

/*
impl<'a> Device<'a> for EthernetDevice {
    type RxToken = EthernetRxToken<'a>;
    type TxToken = EthernetTxToken<'a>;

    fn receive(&'a mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        None
    }

    fn transmit(&'a mut self) -> Option<Self::TxToken> {
        None
    }

    fn capabilities(&self) -> smoltcp::phy::DeviceCapabilities {
        let mut caps = smoltcp::phy::DeviceCapabilities::default();

        caps.max_transmission_unit = 1522;
        caps.max_burst_size = None;
        caps.checksum = smoltcp::phy::ChecksumCapabilities::ignored();
        caps
    }
}

impl<'a> phy::RxToken for EthernetRxToken<'a> {
    fn consume<R, F>(self, timestamp: smoltcp::time::Instant, f: F) -> smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> smoltcp::Result<R>,
    {
        todo!()
    }
}

impl<'a> phy::TxToken for EthernetTxToken<'a> {
    fn consume<R, F>(
        self,
        timestamp: smoltcp::time::Instant,
        len: usize,
        f: F,
    ) -> smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> smoltcp::Result<R>,
    {
        todo!()
    }
}
*/
