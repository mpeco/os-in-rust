/* TODO: priority, how much time a task had the cpu for                       */

pub mod task;


use core::ptr;
use alloc::collections::{BTreeMap, VecDeque};

use crate::{
    ms, processor, time::{Time, timer::{self, stop_schedule_timer}},
    x86_64::interrupts::{interrupts_disabled, handler::SavedState as InterruptSavedState},
};
use self::task::{Task, TaskId};


const TASK_QUEUE_DEFAULT_CAPACITY: usize = 10;
const DEFAULT_PRREMPT_FREQUENCY: Time = ms!(100);


pub fn schedule() {
    processor::get().scheduler().schedule();
}

pub fn add_task(task: Task) {
    processor::get().scheduler().add_task(task);
}

pub fn yield_task() {
    processor::get().scheduler().yield_task();
}

// Yields the currently running task if condition closure returns true
pub fn yield_on_condition<F>(condition: F)
    where F: FnOnce() -> bool
{
    interrupts_disabled(|| {
        if condition() == true {
            processor::get().scheduler().yield_task();
        }
    });
}

pub fn wake_up_task(task_id: TaskId) {
    processor::get().scheduler().wake_up_task(task_id);
}

pub fn get_executing_task_id() -> TaskId {
    processor::get().scheduler().get_executing_task_id()
}

pub fn enable_preemption() {
    processor::get().scheduler().enable_preemption();
}
pub fn disable_preemption() {
    processor::get().scheduler().disable_preemption();
}


pub struct Scheduler {
    is_preemption_enabled: bool,
    is_idle: bool,
    idle_task: Task,
    curr_task: Option<Task>,
    task_queue: VecDeque<Task>,
    blocked_task_map: BTreeMap<TaskId, Task>
}
impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
            is_preemption_enabled: false, is_idle: false,
            idle_task: Task::idle_task(),
            curr_task: None,
            task_queue: VecDeque::with_capacity(TASK_QUEUE_DEFAULT_CAPACITY),
            blocked_task_map: BTreeMap::new()
        }
    }

    pub fn enable_preemption(&mut self) {
        self.is_preemption_enabled = true;
        timer::start_schedule_timer(DEFAULT_PRREMPT_FREQUENCY);
    }
    pub fn disable_preemption(&mut self) {
        self.is_preemption_enabled = false;
        stop_schedule_timer();
    }

    pub fn add_task(&mut self, task: Task) {
        self.task_queue.push_back(task);
    }

    pub fn schedule(&mut self) {
        interrupts_disabled(|| {
            if self.is_preemption_enabled {
                timer::start_schedule_timer(DEFAULT_PRREMPT_FREQUENCY);
            }

            if self.is_idle { return; }

            // in case current task was blocked push it to blocked task map
            let mut curr_task_ref = None;
            if let Some(curr_task) = self.curr_task.as_ref() {
                let curr_task_id = curr_task.id;

                if curr_task.is_blocked {
                    let curr_task = self.curr_task.take().unwrap();
                    self.blocked_task_map.insert(curr_task_id, curr_task);
                    curr_task_ref = Some(self.blocked_task_map.get_mut(&curr_task_id).unwrap());
                }
            }

            // retrieve next task to the queue and switch to it
            if let Some(next_task) = self.task_queue.pop_front() {
                if let Some(curr_task) = self.curr_task.take() {
                    self.task_queue.push_back(curr_task);
                    curr_task_ref = Some(self.task_queue.back_mut().unwrap());
                }

                self.curr_task = Some(next_task);
                let next_task_ref = self.curr_task.as_ref().unwrap();

                switch_task(curr_task_ref, next_task_ref);
            }
            // in case there are no tasks in the queue
            else {
                // if there is a task currently running simply return
                if self.curr_task.is_some() {
                    return;
                }

                // otherwise switch to idle task
                let next_task_ref = &self.idle_task;
                switch_task(curr_task_ref, next_task_ref)
            }
        });
    }

    pub fn yield_task(&mut self) {
        interrupts_disabled(|| {
            if let Some(curr_task) = self.curr_task.as_mut() {
                curr_task.is_blocked = true;
                self.schedule();
            }
        });
    }

    pub fn wake_up_task(&mut self, task_id: TaskId) {
        if let Some(mut task) = self.blocked_task_map.remove(&task_id) {
            task.is_blocked = false;
            self.task_queue.push_front(task);
            self.schedule();
        }
    }

    pub fn get_executing_task_id(&self) -> TaskId {
        debug_assert!(self.curr_task.is_none() == false);
        self.curr_task.as_ref().unwrap().id
    }
}


fn switch_task(curr_task: Option<&mut Task>, next_task: &Task) {
    let processor = processor::get();
    let is_handling_interrupt = *processor.active_interrupt_count() > 0;

    if is_handling_interrupt {
        let interrupt_saved_state = *processor.curr_interrupt_saved_state();
        debug_assert!(interrupt_saved_state.is_null() == false);
        switch_task_from_interrupt(interrupt_saved_state, curr_task, next_task);
    }
    else {
        switch_task_far_ret(curr_task, next_task);
    }
}

fn switch_task_far_ret(curr_task: Option<&mut Task>, next_task: &Task) {
    use core::arch::asm;

    let mut curr_task_state_ptr = ptr::null_mut();
    if let Some(curr_task) = curr_task {
        curr_task_state_ptr = &mut curr_task.saved_state.0 as *mut InterruptSavedState;
    }

    let next_task_state_ptr = &next_task.saved_state.0 as *const InterruptSavedState;

    unsafe {
        asm!(
            r#"
                # don't need to save current state if no task was running
                test rax, rax
                jz 0f

                # save curr task state (no point in saving rax and rcx)
                mov [rax+0x8] , rbx
                mov [rax+0x18], rdx
                mov [rax+0x20], rsi
                mov [rax+0x28], rdi
                mov [rax+0x30], r8
                mov [rax+0x38], r9
                mov [rax+0x40], r10
                mov [rax+0x48], r11
                mov [rax+0x50], r12
                mov [rax+0x58], r13
                mov [rax+0x60], r14
                mov [rax+0x68], r15
                mov [rax+0x70], rbp
                mov [rax+0x90], rsp

                # save end of asm block on rip
                lea rdx, 1f
                mov [rax+0x78], rdx

                # save RFLAGS
                pushfq
                pop rdx
                mov [rax+0x88], rdx

                0:
                # load next task state
                mov rsp, [rcx+0x90]
                mov ss , [rcx+0x98]

                # push cs and rip for retf
                mov rax, [rcx+0x80]
                push rax
                mov rax, [rcx+0x78]
                push rax

                mov rax, [rcx]
                mov rbx, [rcx+0x8]
                # load rcx later
                mov rdx, [rcx+0x18]
                mov rsi, [rcx+0x20]
                mov rdi, [rcx+0x28]
                mov r8 , [rcx+0x30]
                mov r9 , [rcx+0x38]
                mov r10, [rcx+0x40]
                mov r11, [rcx+0x48]
                mov r12, [rcx+0x50]
                mov r13, [rcx+0x58]
                mov r14, [rcx+0x60]
                mov r15, [rcx+0x68]
                mov rbp, [rcx+0x70]

                push [rcx+0x88] # push RFLAGS

                mov rcx, [rcx+0x18]

                popfq # restore RFLAGS

                retfq # retf to next task

                1:
            "#,
            in("rax") curr_task_state_ptr,
            in("rcx") next_task_state_ptr
        );
    }
}

fn switch_task_from_interrupt(interrupt_state_ptr: *mut InterruptSavedState,
    curr_task: Option<&mut Task>, next_task: &Task)
{
    unsafe {
        if let Some(curr_task) = curr_task {
            curr_task.saved_state.0 =  *interrupt_state_ptr;
        }
        *interrupt_state_ptr = next_task.saved_state.0;
    }
}
