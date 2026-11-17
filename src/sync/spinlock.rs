use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

pub struct Spinlock<T: ?Sized> {
    lock: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Sync for Spinlock<T> {}
unsafe impl<T: ?Sized + Send> Send for Spinlock<T> {}

pub struct SpinlockGuard<'a, T: ?Sized + 'a> {
    lock: &'a AtomicBool,
    data: &'a mut T,
    saved_flags: Option<u64>,
}

impl<T> Spinlock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            lock: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    /// Acquires the lock with interrupts safely disabled to prevent IRQ deadlocks.
    pub fn lock(&self) -> SpinlockGuard<'_, T> {
        let flags = crate::arch::x86_64::cpu::read_and_disable_interrupts();
        while self.lock.swap(true, Ordering::Acquire) {
            while self.lock.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
        }
        SpinlockGuard {
            lock: &self.lock,
            data: unsafe { &mut *self.data.get() },
            saved_flags: Some(flags),
        }
    }

    pub fn lock_irqsave(&self) -> SpinlockGuard<'_, T> {
        self.lock()
    }

    pub fn try_lock(&self) -> Option<SpinlockGuard<'_, T>> {
        let flags = crate::arch::x86_64::cpu::read_and_disable_interrupts();
        if !self.lock.swap(true, Ordering::Acquire) {
            Some(SpinlockGuard {
                lock: &self.lock,
                data: unsafe { &mut *self.data.get() },
                saved_flags: Some(flags),
            })
        } else {
            crate::arch::x86_64::cpu::restore_interrupts(flags);
            None
        }
    }

    pub fn raw_lock_ptr(&self) -> *const AtomicBool {
        &self.lock
    }
}

impl<'a, T: ?Sized> Deref for SpinlockGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.data
    }
}

impl<'a, T: ?Sized> DerefMut for SpinlockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.data
    }
}

impl<'a, T: ?Sized> Drop for SpinlockGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.store(false, Ordering::Release);
        if let Some(flags) = self.saved_flags {
            crate::arch::x86_64::cpu::restore_interrupts(flags);
        }
    }
}
