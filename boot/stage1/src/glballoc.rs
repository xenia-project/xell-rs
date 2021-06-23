use buddyalloc::Heap;
use core::alloc::{GlobalAlloc, Layout};
use sync::mutex::SpinMutex;

const HEAP_START: *mut u8 = 0x8000_0000_0800_0000 as *mut u8;
const HEAP_SIZE: usize = 0x0100_0000;

/// Declare a simple heap locked behind a Mutex.
struct LockedHeap<const N: usize>(SpinMutex<Heap<N>>);

/// Implement Rust's [GlobalAlloc] trait for the locked heap.
unsafe impl<const N: usize> GlobalAlloc for LockedHeap<N> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0
            .lock(|heap| heap.allocate(layout).unwrap_or(core::ptr::null_mut()))
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.0.lock(|heap| {
            heap.deallocate(ptr, layout);
        });
    }
}

#[alloc_error_handler]
fn alloc_error(_layout: Layout) -> ! {
    panic!("Allocation failed.");
}

#[global_allocator]
static mut ALLOCATOR: LockedHeap<20> =
    unsafe { LockedHeap(SpinMutex::new(Heap::new_unchecked(HEAP_START, HEAP_SIZE))) };
