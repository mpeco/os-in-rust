use core::{sync::atomic::{AtomicUsize, Ordering}, mem, ptr};
use alloc::alloc::{alloc, dealloc, Layout};


// Lock-free atomic FIFO queue with fixed size
pub struct ArrayQueue<T> {
    buffer_ptr: *mut Option<T>,
    size: usize,
    head: AtomicUsize,
    tail: AtomicUsize
}
impl<T> ArrayQueue<T> {
    pub fn new(size: usize) -> Option<ArrayQueue<T>> {
        // allocate the buffer
        let layout = Layout::from_size_align(
            mem::size_of::<Option<T>>()*size, mem::align_of::<Option<T>>()
        ).unwrap();
        let buffer_ptr = unsafe { alloc(layout) as *mut Option<T> };

        if buffer_ptr == ptr::null_mut() {
            return None;
        }

        // set everything to none
        let buffer = unsafe { core::slice::from_raw_parts_mut(buffer_ptr, layout.size()) };
        for i in 0..layout.size()/mem::size_of::<Option<T>>() { buffer[i] = None; }

        Some(ArrayQueue{ buffer_ptr, size, head: AtomicUsize::new(0), tail: AtomicUsize::new(0) })
    }

    pub fn push(&self, value: T) -> Result<(), ()> {
        let mut old_tail = self.tail.load(Ordering::Acquire);

        if self.is_full() {
            return Err(());
        }

        while let Err(cur_tail) = self.tail.compare_exchange_weak(
            old_tail, (old_tail+1)%self.size, Ordering::AcqRel, Ordering::Acquire
        )
        {
            old_tail = cur_tail;
            if self.is_full() {
                return Err(());
            }
        }

        self.write(old_tail, Some(value));

        Ok(())
    }

    pub fn pop(&self) -> Option<T> {
        let mut old_head = self.head.load(Ordering::Acquire);

        if self.is_empty() { return None; }

        while let Err(cur_head) = self.head.compare_exchange_weak(
            old_head, (old_head+1)%self.size, Ordering::AcqRel, Ordering::Acquire
        )
        {
            old_head = cur_head;
            if self.is_empty() { return None; }
        }

        let value = self.read(old_head).take();
        self.write(old_head, None);

        value
    }

    pub fn is_empty(&self) -> bool {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        head == tail && self.read(head).is_none()
    }

    pub fn is_full(&self) -> bool {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        head == tail && !self.read(head).is_none()
    }

    fn write(&self, index: usize, value: Option<T>) {
        unsafe { core::ptr::write_volatile(self.buffer_ptr.add(index), value) }
    }
    fn read(&self, index: usize) -> Option<T> {
        unsafe { core::ptr::read_volatile(self.buffer_ptr.add(index)) }
    }
}
impl<T> Drop for ArrayQueue<T> {
    fn drop(&mut self) {
        let ptr = self.buffer_ptr as *mut u8;
        let layout = Layout::from_size_align(
            mem::size_of::<Option<T>>()*self.size, mem::align_of::<Option<T>>()
        ).unwrap();
        unsafe { dealloc(ptr, layout); }
    }
}
unsafe impl<T> Sync for ArrayQueue<T> {}
unsafe impl<T> Send for ArrayQueue<T> {}
