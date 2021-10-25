#![no_std]
#![feature(iter_zip)]

mod ring;

extern crate alloc;

use ring::{Ring, RxRing, TxRing};

use alloc::boxed::Box;
use core::{ptr::NonNull, time::Duration};
use smoltcp::phy::{self, Device};

#[allow(dead_code)]
#[repr(u32)]
enum Register {
    TxConfig = 0x00,
    TxDescriptorBase = 0x04,
    TxDescriptorStatus = 0x0C,
    RxConfig = 0x10,
    RxDescriptorBase = 0x14,
    // RxDescriptorStatus(?) = 0x18,
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

// Flag bit guesses:
// 0x8000_0000: Hardware ownership bit
// 0x4000_0000: ??
// 0x0020_0000: (TX) last buffer? e.g. packet not split
// 0x0002_0000: (TX) interrupt related?
// 0x0001_0000: (TX) interrupt related?

const HWDESC_FLAG_HW_OWNED: u32 = 0x80000000;
const HWDESC_CAP_LAST_ENTRY: u32 = 0x80000000; // N.B: This is set in the `capacity` field.

#[repr(C, align(2048))]
#[derive(Clone)]
pub struct EthernetBuffer([u8; 2048]);

impl Default for EthernetBuffer {
    fn default() -> Self {
        Self([0u8; 2048])
    }
}

#[repr(C, packed)]
struct MacAddress([u8; 6]);

impl From<u64> for MacAddress {
    fn from(n: u64) -> Self {
        let bytes = n.to_be_bytes();
        Self(bytes[2..].try_into().unwrap())
    }
}

/// Transfer descriptor, as defined by hardware.
///
/// Descriptors can follow the following state machine:
/// * RX
///  * Free:
///   * `len != 0`: Network packet contained within buffer.
///   * `len == 0`: No receive buffer set. (implies `capacity` == 0)
///  * Busy: Owned by hardware; pending packet RX
/// * TX
///  * Free: Descriptor is free for queueing a network TX.
///   * Transmitted packet contained within; can free buffer.
///   * No transmit buffer set.
///  * Busy: Owned by hardware; pending packet TX
#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct HwDescriptor {
    /// Length of the packet contained within `addr`, if any.
    len: u32,
    /// Flags interpreted by the hardware, such as an ownership bit or interrupt
    /// routing bits.
    flags: u32,
    /// Physical address of an in-memory buffer used to contain the packet.
    addr: u32,
    /// Capacity of the in-memory buffer, with the high bit aliased as an "end-of-ring" bit.
    capacity: u32,
}

impl HwDescriptor {
    fn new() -> Self {
        Self {
            len: 0,
            flags: 0,
            addr: 0,
            capacity: 0,
        }
    }

    /// Query to see if this descriptor is currently busy (owned by hardware) at this point in time.
    fn is_busy(&self) -> bool {
        (unsafe { core::ptr::read_volatile(&self.flags) } & HWDESC_FLAG_HW_OWNED) != 0
    }
}

impl Default for HwDescriptor {
    fn default() -> Self {
        Self {
            len: 0x0000_0000,
            flags: 0x0000_0000,
            addr: 0x0BAD_F00D,
            capacity: 0x0000_0000,
        }
    }
}

#[repr(align(16))]
pub struct EthernetDevice<const N: usize, const M: usize> {
    mmio: core::ptr::NonNull<u8>,

    rx_ring: Ring<RxRing, N>,
    tx_ring: Ring<TxRing, M>,
}

impl<const N: usize, const M: usize> EthernetDevice<N, M> {
    /// Constructs a new [EthernetDevice].
    ///
    /// SAFETY: The caller _MUST_ ensure that there is only one instance
    /// of this object at a time. Multiple instances will cause undefined behavior.
    pub unsafe fn new() -> Self {
        let mut obj = Self {
            mmio: NonNull::new_unchecked(0x8000_0200_EA00_1400 as *mut u8),

            rx_ring: Ring::new(),
            tx_ring: Ring::new(),
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

    fn reset(&mut self) {
        // N.B: The magic numbers are from:
        // https://github.com/xenia-project/linux/blob/8b3cd8b6e99453ad854a5441092ed87b70385f37/drivers/net/ethernet/xenon/xenon_net.c#L370-L438

        // Zero out the interrupt mask.
        self.write(Register::InterruptMask, 0x00000000);

        // Reset the chip.
        self.write(Register::Config0, 0x0855_8001);
        xenon_cpu::time::delay(Duration::from_micros(100));
        self.write(Register::Config0, 0x0855_0001);

        self.write(Register::PhyControl, 0x00000004);
        xenon_cpu::time::delay(Duration::from_micros(100));
        self.write(Register::PhyControl, 0x00000000);

        self.write(Register::MaxPacketSize, 1522u32);

        self.write(Register::Config1, 0x2360);

        self.write(Register::MulticastFilterControl, 0x0E38);

        self.write(Register::Address0, MacAddress::from(0x69_42_00_00_00_00));
        self.write(Register::Address1, MacAddress::from(0x69_42_00_00_00_01));

        self.write(Register::TxConfig, 0x0000_1C00);
        self.write(Register::RxConfig, 0x0010_1C00);

        self.write(Register::PhyConfig, 0x0400_1901);

        // Write out the TX descriptor ring base 0.
        self.write(Register::TxConfig, 0x0000_1C00);
        self.write(Register::TxDescriptorBase, self.tx_ring.phys_base() as u32);

        // Write out the TX descriptor ring base 1.
        // FIXME: The originating implementation was hacked together. Why do they use the same ring twice?
        self.write(Register::TxConfig, 0x0001_1C00);
        self.write(Register::TxDescriptorBase, self.tx_ring.phys_base() as u32);
        self.write(Register::TxConfig, 0x0000_1C00);

        // Write out the RX descriptor ring base.
        self.write(Register::RxDescriptorBase, self.rx_ring.phys_base() as u32);

        // ???
        self.write(Register::PhyConfig, 0x0400_1001);
        self.write(Register::Config1, 0u32);
        self.write(Register::Config0, 0x0855_0001);

        // Enable RX/TX
        self.write(Register::TxConfig, 0x0000_1C01);
        self.write(Register::RxConfig, 0x0010_1C11);

        // Disable all interrupts.
        self.write(Register::InterruptMask, 0x0000_0000);
    }
}

/// Represents a token that, when consumed, yields a received packet.
pub struct EthernetRxToken<'ring, const N: usize>(ring::CompleteDescriptor<'ring, ring::RxRing, N>);

/// Represents a token that, when consumed, takes ownership of a buffer containing a packet to be sent.
pub struct EthernetTxToken<'ring, const M: usize>(ring::FreeDescriptor<'ring, ring::TxRing, M>);

// Implement the smoltcp interface to the Xenon ethernet device.
impl<'dev, const N: usize, const M: usize> Device<'dev> for EthernetDevice<N, M> {
    type RxToken = EthernetRxToken<'dev, N>;
    type TxToken = EthernetTxToken<'dev, M>;

    fn receive(&'dev mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        // Free up completed TX descriptors.
        while let Some(desc) = self.tx_ring.get_next_complete() {
            // Free the desc, drop the inner buffer. Maybe attempt to reuse it in the future.
            desc.free();
        }

        // Requeue free RX descriptors.
        while let Some(desc) = self.rx_ring.get_next_free() {
            let buf = Box::new(EthernetBuffer::default());

            // Submit the descriptor back to hardware.
            desc.submit(buf);
        }

        Some((
            EthernetRxToken(self.rx_ring.get_next_complete()?),
            EthernetTxToken(self.tx_ring.get_next_free()?),
        ))
    }

    fn transmit(&'dev mut self) -> Option<Self::TxToken> {
        // Free up completed TX descriptors.
        while let Some(desc) = self.tx_ring.get_next_complete() {
            // Free the desc, drop the inner buffer. Maybe attempt to reuse it in the future.
            desc.free();
        }

        // Now try to get the next free entry again. In most cases, it will point to an
        // entry we just freed.
        Some(EthernetTxToken(self.tx_ring.get_next_free()?))
    }

    fn capabilities(&self) -> smoltcp::phy::DeviceCapabilities {
        let mut caps = smoltcp::phy::DeviceCapabilities::default();

        caps.max_transmission_unit = 1522;
        caps.max_burst_size = None;
        caps.checksum = smoltcp::phy::ChecksumCapabilities::ignored();
        caps
    }
}

impl<'a, const N: usize> phy::RxToken for EthernetRxToken<'a, N> {
    fn consume<R, F>(self, _timestamp: smoltcp::time::Instant, f: F) -> smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> smoltcp::Result<R>,
    {
        let (mut buf, len) = self.0.free();
        f(&mut buf.0[..len])
    }
}

impl<'a, const M: usize> phy::TxToken for EthernetTxToken<'a, M> {
    fn consume<R, F>(
        self,
        _timestamp: smoltcp::time::Instant,
        len: usize,
        f: F,
    ) -> smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> smoltcp::Result<R>,
    {
        let mut buf = Box::new(EthernetBuffer::default());
        let res = f(&mut buf.0[..len])?;

        self.0.submit(buf, len);

        Ok(res)
    }
}
