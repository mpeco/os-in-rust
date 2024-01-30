use core::{alloc::Layout, mem, ptr, sync::atomic::{AtomicU64, Ordering}};
use alloc::alloc::{alloc, dealloc};

use crate::{memory::address::VirtAddr, x86_64::interrupts::handler::SavedState as InterruptSavedState};


const IDLE_TASK_ID: TaskId = TaskId { 0: 0 };
const IDLE_TASK_STACK_LEN: usize = 128;


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TaskId(u64);
impl TaskId {
    pub fn new() -> TaskId {
        static NEXT_ID: AtomicU64 = AtomicU64::new(IDLE_TASK_ID.0 + 1);
        TaskId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}
// Use the same setup saved during interrupts since it contains all the registers
pub struct SavedState(pub InterruptSavedState);
impl SavedState {
    pub fn new() -> SavedState {
        SavedState { 0: InterruptSavedState { ..Default::default() } }
    }
}
pub struct Task {
    pub id: TaskId,
    _stack: Stack,
    pub saved_state: SavedState,
    pub is_blocked: bool
}
impl Task {
    pub fn new<T>(stack_len: usize, init_task_fn: fn(*const T), args: Option<*const T>) -> Task {
        use crate::x86_64::cpu::registers;

        let stack = Stack::new(stack_len);

        let mut saved_state = SavedState::new();
        let state = &mut saved_state.0;

        state.stack_frame.cs  = registers::cs::read() as u64;
        state.stack_frame.rip = init_task_fn_wrapper as u64;
        state.stack_frame.ss  = registers::ss::read() as u64;
        state.stack_frame.rsp = stack.get_top_addr().as_usize() as u64;
        state.stack_frame.rflags = registers::rflags::read();

        state.rdi = init_task_fn as u64; // 1st param
        if let Some(args) = args {
            state.rsi = args as u64; // 2nd param
        }

        Task { id: TaskId::new(), _stack: stack, saved_state, is_blocked: false }
    }

    pub fn idle_task() -> Task {
        let mut idle_task = Self::new(IDLE_TASK_STACK_LEN, idle_task_fn, None);
        idle_task.id = IDLE_TASK_ID;
        idle_task
    }
}
#[allow(improper_ctypes_definitions)]
extern "sysv64" fn init_task_fn_wrapper(init_task_fn: fn(*const ()), args: *const ()) {
    init_task_fn(args);
}
fn idle_task_fn(_args: *const ()) {
    use crate::x86_64::cpu;

    loop {
        cpu::instructions::sti();
        cpu::instructions::hlt();
    }
}

pub struct Stack {
    pub buffer: *mut u8,
    pub length: usize
}
impl Stack {
    pub fn new(length: usize) -> Stack {
        // allocate the buffer
        let layout = Layout::from_size_align(
            mem::size_of::<u8>()*length, mem::align_of::<u8>()
        ).unwrap();
        let buffer = unsafe { alloc(layout) as *mut u8 };
        assert_ne!(buffer, ptr::null_mut(), "Unsufficient memory to allocate stack");
        Stack { buffer, length }
    }

    pub fn get_top_addr(&self) -> VirtAddr {
        VirtAddr::new(self.buffer as usize + self.length)
    }
}
impl Drop for Stack {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(
            mem::size_of::<u8>()*self.length, mem::align_of::<u8>()
        ).unwrap();
        unsafe { dealloc(self.buffer, layout); }
    }
}
