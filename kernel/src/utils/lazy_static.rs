use core::{cell::UnsafeCell, mem::MaybeUninit, ops::{Deref, DerefMut}};

use super::init_once::InitOnce;


// Wrapper for UnsafeCell that implements Sync as long as T implements it
pub struct SyncUnsafeCell<T>
    where T: Sync
{
    value: UnsafeCell<T>
}
impl<T> Deref for SyncUnsafeCell<T>
    where T: Sync
{
    type Target = UnsafeCell<T>;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
unsafe impl<T> Sync for SyncUnsafeCell<T> where T: Sync {}


// A static variable that has to be initialized
pub struct LazyStatic<T>
    where T: Sync
{
    value: SyncUnsafeCell<MaybeUninit<T>>,
    is_init: InitOnce
}
impl<T> LazyStatic<T>
    where T: Sync
{
    pub const fn new() -> LazyStatic<T> {
        LazyStatic {
            value: SyncUnsafeCell { value: UnsafeCell::new(MaybeUninit::uninit()) },
            is_init: InitOnce::new()
        }
    }

    pub fn init(&self, value: T) {
        self.is_init.init().expect("Attempted to initialize LazyStatic more than once");
        unsafe { (&mut *self.value.get()).write(value); }
    }

    pub fn is_init(&self) -> bool {
        self.is_init.is_init()
    }
}
impl<T> Deref for LazyStatic<T>
    where T: Sync
{
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { (&mut *self.value.get()).assume_init_ref() }
    }
}
impl<T> DerefMut for LazyStatic<T>
    where T: Sync
{
    fn deref_mut(&mut self) -> &mut T {
        unsafe { (&mut *self.value.get()).assume_init_mut() }
    }
}
