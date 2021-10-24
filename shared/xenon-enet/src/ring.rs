//! This module contains code for ethernet ring management.

use super::{EthernetBuffer, HwDescriptor};
use core::{marker::PhantomData, sync::atomic::Ordering};

extern crate alloc;
use alloc::boxed::Box;

/// An individual "logical" descriptor, used to track extra information
/// associated with hardware descriptors.
#[derive(Default, Clone)]
struct LogicalDescriptor {
    /// The managed heap buffer assigned to this descriptor, if any.
    buf: Option<Box<EthernetBuffer>>,
}

impl core::fmt::Debug for LogicalDescriptor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LogicalDescriptor")
            .field("buf", &self.buf.is_some())
            .finish()
    }
}

pub trait RingType {}

/// Receive ring marker
pub struct RxRing;
/// Transfer ring marker
pub struct TxRing;

impl RingType for RxRing {}
impl RingType for TxRing {}

/// This structure represents a ring of DMA buffer descriptors for a Xenon MAC.
///
/// # Hardware interaction
/// Hardware and software may both access descriptors in the ring at the same time.
///
/// When a descriptor is ready for hardware processing, the ownership bit is flipped
/// such that the hardware now "owns" the descriptor.
/// Processing means sending a packet for TX, or the reception of a packet for RX.
///
/// When the hardware owns the descriptor, we may not touch it whatsoever.
/// As such, this interface offers no way to retrieve hardware-owned descriptors.
///
/// When a descriptor is finished processing, hardware will turn off the ownership
/// bit, handing ownership back to us. At this point, we can take the buffer out of
/// the descriptor and process or free it.
pub struct Ring<S: RingType, const N: usize> {
    _ring_type: PhantomData<S>,

    /// A contiguous array of hardware descriptors. The hardware will receive a pointer to this.
    /// This _assumes_ that the MMU is disabled, and `va == pa`.
    hw_descriptors: Box<[HwDescriptor; N]>,

    /// Associated logical descriptors, tracking extra information that can't live inside
    /// of the hardware descriptors.
    descriptors: [LogicalDescriptor; N],

    /// The next busy descriptor, without wraparound.
    next_busy: usize,
    /// The next free descriptor, without wraparound. If `next_free` == `next_busy`, all descriptors are free.
    next_free: usize,
}

impl<S: RingType, const N: usize> Ring<S, N> {
    /// Construct a new ethernet ring, with an allocation backed by the global allocator.
    pub fn new() -> Self {
        let mut hw_descriptors = Box::new([HwDescriptor::new(); N]);
        hw_descriptors.last_mut().unwrap().capacity = super::HWDESC_CAP_LAST_ENTRY;

        const LOGDESC_INIT: LogicalDescriptor = LogicalDescriptor { buf: None };

        Self {
            _ring_type: PhantomData,
            hw_descriptors,
            descriptors: [LOGDESC_INIT; N],

            next_busy: 0,
            next_free: 0,
        }
    }

    /// Retrieve the next unused descriptor, if any.
    pub fn get_next_free<'ring>(&'ring mut self) -> Option<FreeDescriptor<'ring, S, N>> {
        // If `next_free` is >= `N` slots away from `next_busy`,
        // the entire ring has been consumed.
        if self.next_free - self.next_busy >= N {
            None
        } else {
            // N.B: Do not increment `next_free` here. The descriptor must do that when submitted.
            // Because of the mutable borrow against `self`, callers cannot fetch more than
            // one descriptor at a time.
            let idx = self.next_free % N;

            Some(FreeDescriptor { ring: self, idx })
        }
    }

    /// Retrieve the next completed descriptor, if any.
    pub fn get_next_complete(&mut self) -> Option<CompleteDescriptor<'_, S, N>> {
        if self.next_busy == self.next_free {
            None
        } else {
            let idx = self.next_busy % N;

            // Now, we need to check and see if the HW ownership bit if set.
            // If so, do not return a reference.
            if self.hw_descriptors[idx].is_busy() {
                None
            } else {
                Some(CompleteDescriptor { ring: self, idx })
            }
        }
    }
}

unsafe fn read_mod_write_volatile<T>(addr: *mut T, func: impl FnOnce(T) -> T) {
    let oval = core::ptr::read_volatile(addr);
    let nval = func(oval);
    core::ptr::write_volatile(addr, nval);
}

/// Represents a safe interface for a particular free hardware descriptor.
pub struct FreeDescriptor<'a, S: RingType, const N: usize> {
    /// The ring that owns this descriptor.
    ring: &'a mut Ring<S, N>,
    /// The wrapped-around descriptor index.
    idx: usize,
}

// Actions corresponding to a free descriptor on the RX ring.
impl<'a, const N: usize> FreeDescriptor<'a, RxRing, N> {
    /// Submit this descriptor to hardware.
    pub fn submit(self, buf: Box<EthernetBuffer>) {
        // Update the hardware descriptor.
        let hw_desc = &mut self.ring.hw_descriptors[self.idx];
        unsafe {
            core::ptr::write_volatile(&mut hw_desc.len, 0); // RX: 0 bytes initial length
            core::ptr::write_volatile(&mut hw_desc.addr, buf.0.as_ptr() as u32);

            read_mod_write_volatile(&mut hw_desc.capacity, |v| {
                // N.B: Avoid overwriting HWDESC_CAP_LAST_ENTRY.
                (v & super::HWDESC_CAP_LAST_ENTRY) | (buf.0.len() as u32 & 0x7FFF_FFFF)
            });

            // Prevent reordering of the above writes and the below ownership flag modification.
            core::sync::atomic::fence(Ordering::SeqCst);

            // TODO: Figure out what magic bit 0x4000_0000 is.
            core::ptr::write_volatile(
                &mut hw_desc.flags,
                super::HWDESC_FLAG_HW_OWNED | 0x4000_0000,
            );
        }

        self.ring.next_free += 1;
    }
}

// Actions corresponding to a free descriptor on the TX ring.
impl<'a, const N: usize> FreeDescriptor<'a, TxRing, N> {
    /// Submit this descriptor to hardware.
    pub fn submit(self, buf: Box<EthernetBuffer>, len: usize) {
        // Update the hardware descriptor.
        let hw_desc = &mut self.ring.hw_descriptors[self.idx];
        unsafe {
            core::ptr::write_volatile(&mut hw_desc.len, len as u32);
            core::ptr::write_volatile(&mut hw_desc.addr, buf.0.as_ptr() as u32);

            read_mod_write_volatile(&mut hw_desc.capacity, |v| {
                // N.B: Avoid overwriting HWDESC_CAP_LAST_ENTRY.
                (v & super::HWDESC_CAP_LAST_ENTRY) | (buf.0.len() as u32 & 0x7FFF_FFFF)
            });

            // Prevent reordering of the above writes and the below ownership flag modification.
            core::sync::atomic::fence(Ordering::SeqCst);

            // TODO: Figure out the magic bits 0x4023_0000.
            core::ptr::write_volatile(
                &mut hw_desc.flags,
                super::HWDESC_FLAG_HW_OWNED | 0x4023_0000,
            );
        }

        // Update the logical descriptor.
        self.ring.descriptors[self.idx].buf.replace(buf);
        self.ring.next_free += 1;
    }
}

/// Represents a safe interface for a particular completed hardware descriptor.
pub struct CompleteDescriptor<'a, S: RingType, const N: usize> {
    ring: &'a mut Ring<S, N>,
    idx: usize,
}

impl<'a, S: RingType, const N: usize> CompleteDescriptor<'a, S, N> {
    // take() function for contained buffer
    //   -> tx, caller takes buffer and frees or reuses
    //   -> rx, caller submits packet to netstack and (typically) reuses buffer
    // submit() to transfer this descriptor to HW for processing

    /// Mark a previously finished descriptor as free, taking the buffer out of it.
    /// This returns a tuple of the buffer and the length used by hardware.
    pub fn free(self) -> (Box<EthernetBuffer>, usize) {
        // Clear out the descriptor.
        let hw_desc = &mut self.ring.hw_descriptors[self.idx];
        let len = unsafe {
            core::ptr::write_volatile(&mut hw_desc.addr, 0x0BADF00D);
            core::ptr::read_volatile(&hw_desc.len)
        };

        // Take the buffer from the logical descriptor.
        let buf = self.ring.descriptors[self.idx]
            .buf
            .take()
            .expect("no buffer in completed descriptor");

        self.ring.next_busy += 1;
        (buf, len as usize)
    }
}

/// This implements methods specific to an RX descriptor ring.
impl<const N: usize> Ring<RxRing, N> {
    // ref empty descriptors
    //   -> fill with memory buffers
    // ref completed descriptors
    //   -> take and maybe replace packet buffer
    // no ref of hardware-owned descriptors
}

/// This implements methods specific to a TX descriptor ring.
impl<const N: usize> Ring<TxRing, N> {
    // ref empty descriptors
    //   -> put buffer into ring for future tx
    // ref completed descriptors
    //   -> take buffer and free to heap (or reuse)
    // no ref of hardware-owned descriptors
}
