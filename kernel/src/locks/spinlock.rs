use core::{
    sync::atomic::{AtomicBool, Ordering},
    cell::UnsafeCell,
    hint::spin_loop,
    ops::{Deref, DerefMut}
};


pub struct Spinlock<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>
}
impl<T> Spinlock<T> {
    pub const fn new(value: T) -> Spinlock<T> {
        Spinlock { locked: AtomicBool::new(false), value: UnsafeCell::new(value) }
    }

    pub fn lock(&self) -> SpinlockGuard<T> {
        while self.locked.swap(true, Ordering::Acquire) {
            spin_loop()
        }
        SpinlockGuard::new(self)
    }

    // halts while waiting
    pub fn lock_hlt(&self) -> SpinlockGuard<T> {
        crate::x86_64::interrupts::hlt_wait(
            || { self.locked.swap(true, Ordering::Acquire) == false }
        );
        SpinlockGuard::new(self)
    }
}
// The spinlock will guarantee only one thread can access the value at a time
unsafe impl<T> Sync for Spinlock<T> where T: Send {}

pub struct SpinlockGuard<'a, T> {
    spinlock: &'a Spinlock<T>,
}
impl<T> SpinlockGuard<'_, T> {
    fn new(spinlock: &Spinlock<T>) -> SpinlockGuard<'_, T> {
        SpinlockGuard { spinlock }
    }

    pub fn unlock(self) {
        drop(self);
    }
}
// Only one instance of SpinlockGuard can exist at a time, making these references safe
impl<T> Deref for SpinlockGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.spinlock.value.get() }
    }
}
impl<T> DerefMut for SpinlockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.spinlock.value.get() }
    }
}
impl<T> Drop for SpinlockGuard<'_, T> {
    fn drop(&mut self) {
        self.spinlock.locked.store(false, Ordering::Release);
    }
}
