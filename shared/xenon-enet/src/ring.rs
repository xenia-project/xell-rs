use super::{EthernetBuffer, HwDescriptor};
use core::marker::PhantomData;

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
        self.0.flags |= super::HWDESC_FLAG_HW_OWNED;
        EthernetPendingDesc(self.0, self.1)
    }
}

/// Represents an ethernet descriptor that has pending data.
pub struct EthernetCompleteDesc<'a>(&'a mut HwDescriptor);

/// Receive ring marker
pub struct RxRing;
/// Transfer ring marker
pub struct TxRing;

trait RingType {}
impl RingType for RxRing {}
impl RingType for TxRing {}

/// This structure represents a ring of DMA buffer descriptors for a Xenon MAC.
///
/// # Usage (RX)
/// ```rust,ignore
/// let mut ring: Ring<RxRing, 16>;
///
/// match ring.next_free() {
///   Some(desc) => {},
///   None => {}
/// }
/// ```
pub struct Ring<T: RingType, const N: usize> {
    ring_type: PhantomData<T>,

    /// A contiguous array of hardware descriptors. The hardware will receive a pointer to this.
    descriptors: [HwDescriptor; N],

    /// Index of first busy buffer (or represents no busy buffers if equivalent to `avail`)
    busy: usize,
    /// Index of first free buffer
    avail: usize,
}

impl<T: RingType, const N: usize> Ring<T, N> {
    fn new() -> Self {
        let mut descriptors = [HwDescriptor::new(); N];
        descriptors.last_mut().unwrap().capacity = super::HWDESC_LAST_ENTRY;

        Self {
            ring_type: PhantomData,
            descriptors: [HwDescriptor::new(); N],

            busy: 0,
            avail: 0,
        }
    }

    pub fn new_rx(buffers: [EthernetBuffer; N]) -> Self {
        let mut obj = Ring::new();

        for (mut desc, buf) in core::iter::zip(obj.descriptors, buffers) {
            desc.addr = buf.0.as_mut_ptr() as u32;
        }

        obj
    }
}

/// This implements methods specific to an RX descriptor ring.
impl<const N: usize> Ring<RxRing, N> {
    /// Attempt to consume a descriptor and swap its buffer with
    /// the input buffer.
    pub fn consume() -> EthernetBuffer {}
}

/// This implements methods specific to a TX descriptor ring.
impl<const N: usize> Ring<TxRing, N> {
    pub fn transmit_buffer(buffer: EthernetBuffer) -> Result<(), EthernetBuffer> {
        // TODO:
        // Attempt to find a free descriptor, or error out if none are free.
        Ok(())
    }
}
