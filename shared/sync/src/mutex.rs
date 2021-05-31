use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicU32, Ordering},
};

/// This struct implements a naive spinlock that guards data contained within.
#[repr(align(16))]
pub struct SpinMutex<T> {
    lock_count: AtomicU32,
    inner: UnsafeCell<T>,
}

// Implement Send + Sync for a SpinMutex containing an object that implements Send.
// The object does not have to implement Sync since it will only be accessed from a single thread.
unsafe impl<T: Send> Send for SpinMutex<T> {}
unsafe impl<T: Send> Sync for SpinMutex<T> {}

#[allow(dead_code)]
impl<T> SpinMutex<T> {
    pub const fn new(inner: T) -> Self {
        Self {
            lock_count: AtomicU32::new(0),
            inner: UnsafeCell::new(inner),
        }
    }

    /// Retrieves the inner value without attempting to lock the spinlock,
    /// or seeing if the spinlock is already locked. Unsafe for obvious reasons.
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn get_mut_unchecked(&self) -> &mut T {
        &mut *self.inner.get()
    }

    /// This function will attempt to lock the mutex and call the passed-in closure.
    pub fn try_lock<R>(&self, f: impl FnOnce(&mut T) -> R) -> Result<R, ()> {
        // Attempt to acquire the lock.
        match self
            .lock_count
            .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
        {
            Ok(_) => {}
            Err(_) => return Err(()),
        }

        // We now have exclusive access to the data within.
        let r = f(unsafe { &mut *self.inner.get() });

        // Release the lock.
        self.lock_count.fetch_sub(1, Ordering::Release);

        Ok(r)
    }

    /// This function will call the passed-in closure when the mutex is locked.
    pub fn lock<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        // Acquire the lock.
        loop {
            match self
                .lock_count
                .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
            {
                Ok(_) => break,
                Err(_) => continue,
            }
        }

        // We have exclusive access to the data within.
        let r = f(unsafe { &mut *self.inner.get() });

        // Release the lock.
        // We have to do this without lwarx/stwcx due to a processor race condition.
        // This is probably safe(?)
        self.lock_count.store(0, Ordering::Relaxed);

        r
    }
}
