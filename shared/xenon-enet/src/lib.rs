use smoltcp::phy::{self, Device};

#[allow(dead_code)]
#[repr(u32)]
enum Registers {
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

#[repr(C, align(16))]
struct Descriptor {
    len: u32,
    flags: u32,
    addr: u32,
    capacity: u32,
}

#[repr(align(16))]
pub struct EthernetDevice {
    mmio: &'static mut [u8],

    rx_buffer: [u8; 1536],
    tx_buffer: [u8; 1536],
}

impl EthernetDevice {
    pub fn new() -> Self {
        Self {
            mmio: unsafe { core::slice::from_raw_parts_mut(0x8000_0200_EA00_1400 as *mut u8, 0x80) },

            rx_buffer: [0; 1536],
            tx_buffer: [0; 1536],
        }
    }
}

pub struct EthernetRxToken<'a>(&'a mut [u8]);
pub struct EthernetTxToken<'a>(&'a mut [u8]);

impl<'a> Device<'a> for EthernetDevice {
    type RxToken = EthernetRxToken<'a>;
    type TxToken = EthernetTxToken<'a>;

    fn receive(&'a mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        todo!()
    }

    fn transmit(&'a mut self) -> Option<Self::TxToken> {
        todo!()
    }

    fn capabilities(&self) -> smoltcp::phy::DeviceCapabilities {
        todo!()
    }
}

impl<'a> phy::RxToken for EthernetRxToken<'a> {
    fn consume<R, F>(self, timestamp: smoltcp::time::Instant, f: F) -> smoltcp::Result<R>
        where F: FnOnce(&mut [u8]) -> smoltcp::Result<R> {
        todo!()
    }
}

impl<'a> phy::TxToken for EthernetTxToken<'a> {
    fn consume<R, F>(self, timestamp: smoltcp::time::Instant, len: usize, f: F) -> smoltcp::Result<R>
        where F: FnOnce(&mut [u8]) -> smoltcp::Result<R> {
        todo!()
    }
}
