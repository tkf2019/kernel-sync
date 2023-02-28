//! A naive seqlock implementation.
//!
//! Seqlocks are similar to read/write spin locks, except they give a much higher
//! priority to writers: in fact a writer is allowed to proceed even when readers
//! are active.

use core::{
    cell::SyncUnsafeCell,
    ops::{Deref, DerefMut},
};

use alloc::fmt;

use crate::{
    arch::{smp_rmb, smp_wmb},
    SpinLock, SpinLockGuard,
};

/// A seqlock (short for sequence lock) is a special locking mechanism used in Linux
/// for supporting fast writes of shared variables between two parallel operating
/// system routines.
pub struct SeqLock<T: ?Sized> {
    seq: SyncUnsafeCell<usize>,
    lock: SpinLock<T>,
}

/// A guard that provides mutable data access.
///
/// When the guard falls out of scope it will release the lock.
pub struct SeqLockGuard<'a, T: ?Sized + 'a> {
    seq: &'a mut usize,
    lock: SpinLockGuard<'a, T>,
}

unsafe impl<T: ?Sized + Send> Sync for SeqLock<T> {}
unsafe impl<T: ?Sized + Send> Send for SeqLock<T> {}

impl<T> SeqLock<T> {
    /// Creates a new [`SeqLock`] wrapping the supplied data.
    #[inline(always)]
    pub const fn new(data: T) -> Self {
        Self {
            seq: SyncUnsafeCell::new(0),
            lock: SpinLock::new(data),
        }
    }

    /// Consumes this [`SeqLock`] and unwraps the underlying data.
    #[inline(always)]
    pub fn into_inner(self) -> T {
        // We know statically that there are no outstanding references to
        // `self` so there's no need to lock.
        let SeqLock { lock, .. } = self;
        lock.into_inner()
    }
}

impl<T: ?Sized> SeqLock<T> {
    /// Reads the data with its immutable reference. Critical sections can be executed several times.
    /// There is no need to disable interrupt in this function.
    ///
    /// # Safety
    ///
    /// The technique will not work for data that contains **raw pointers**, because any writer could
    /// invalidate a pointer that a reader has already followed. Updating the memory block being
    /// pointed-to is fine using seqlocks, but updating the pointer itself is not allowed. In a case
    /// where the pointers themselves must be updated or changed, using read-copy-update synchronization
    /// is preferred.
    ///
    /// Thus reference counter wrappers like `Arc` and `Weak` are suggested to prevent the data from being
    /// reclaimed.
    #[inline(always)]
    pub fn read<F, I>(&self, mut f: F) -> I
    where
        F: FnMut(&T) -> I,
    {
        loop {
            let seq = unsafe { &*self.seq.get() };
            // Check the sequence number if a writer has already been in the critical section
            let mut start = *seq;
            while start & 1 == 1 {
                start = *seq;
                core::hint::spin_loop();
            }
            smp_rmb();

            // Critical section
            let ret = f(unsafe { &*self.lock.as_mut_ptr() });

            // Retry if a writer broke the critical section.
            smp_rmb();
            if start == *seq {
                return ret;
            }
        }
    }

    /// Locks the [`SeqLock`] and returns a guard that permits mutable access to inner data.
    pub fn write(&self) -> SeqLockGuard<T> {
        let lock = self.lock.lock();
        let seq = unsafe { &mut *self.seq.get() };

        // Increase sequence number
        *seq += 1;
        smp_wmb();

        SeqLockGuard { seq, lock }
    }

    /// Tries to read the data with its immutable reference. Critical sections can be executed only once.
    /// Returns if a writer broke the critical section.
    ///
    /// # Safety
    ///
    /// The technique will not work for data that contains **raw pointers**, because any writer could
    /// invalidate a pointer that a reader has already followed. Updating the memory block being
    /// pointed-to is fine using seqlocks, but updating the pointer itself is not allowed. In a case
    /// where the pointers themselves must be updated or changed, using read-copy-update synchronization
    /// is preferred.
    ///
    /// Thus reference counter wrappers like `Arc` and `Weak` are suggested to prevent the data from being
    /// reclaimed.
    #[inline(always)]
    pub fn try_read<F, I>(&self, mut f: F) -> Option<I>
    where
        F: FnMut(&T) -> I,
    {
        let seq = unsafe { &*self.seq.get() };
        // Check the sequence number if a writer has already been in the critical section
        let mut start = *seq;
        while start & 1 == 1 {
            start = *seq;
            core::hint::spin_loop();
        }
        smp_rmb();

        // Critical section
        let ret = f(unsafe { &*self.lock.as_mut_ptr() });

        smp_rmb();
        if start == *seq {
            Some(ret)
        } else {
            None
        }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for SeqLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let result = self.try_read(|data| {
            write!(f, "SeqLock {{ data: ")
                .and_then(|()| data.fmt(f))
                .and_then(|()| write!(f, "}}"));
        });
        write!(
            f,
            "{} result",
            if result.is_some() {
                "Real"
            } else {
                "Uncertain"
            }
        )
    }
}

impl<T: ?Sized + Default> Default for SeqLock<T> {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<T> From<T> for SeqLock<T> {
    fn from(data: T) -> Self {
        Self::new(data)
    }
}

impl<'a, T: ?Sized + fmt::Debug> fmt::Debug for SeqLockGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized + fmt::Display> fmt::Display for SeqLockGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized> Deref for SeqLockGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.lock
    }
}

impl<'a, T: ?Sized> DerefMut for SeqLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.lock
    }
}

impl<'a, T: ?Sized> Drop for SeqLockGuard<'a, T> {
    /// The dropping of the MutexGuard will release the lock it was created from and increase the sequence again to
    /// keep if even.
    fn drop(&mut self) {
        smp_wmb();
        *self.seq += 1;
    }
}
