use crate::processor;


pub struct InterruptHandler();
impl InterruptHandler {
    pub fn get_addr(&self) -> usize {
        self as *const _ as usize
    }
}

// In the reverse order as it is pushed to the stack
#[repr(C, packed)]
#[derive(Default, Debug, Clone, Copy)]
pub struct StackFrame {
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}
#[repr(C, packed)]
#[derive(Default, Clone, Copy)]
pub struct SavedState {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rbp: u64,
    pub stack_frame: StackFrame
}


/**
 * Increments the processor's active interrupt count, if it's not a nested interrupt saves the
 * current task state in the scheduler in case of a task switch
 */
pub unsafe extern "sysv64" fn handler_wrapper(handler_addr: usize, saved_state_addr: usize) {
    let processor = processor::get();
    let active_interrupt_count = processor.active_interrupt_count();
    *active_interrupt_count += 1;

    let saved_state_ptr = saved_state_addr as *mut SavedState;

    if *active_interrupt_count == 1 {
        *processor.curr_interrupt_saved_state() = saved_state_addr as *mut SavedState;
    }

    let stack_frame = &(*saved_state_ptr).stack_frame;
    let handler_fn: fn(&StackFrame) = core::mem::transmute(handler_addr);
    handler_fn(stack_frame);

    debug_assert!(*active_interrupt_count > 0);
    *active_interrupt_count -= 1;
}
pub unsafe extern "sysv64" fn handler_with_err_wrapper(handler_addr: usize, saved_state_addr: usize, error: u64) {
    let processor = processor::get();
    let active_interrupt_count = processor.active_interrupt_count();
    *active_interrupt_count += 1;

    let saved_state_ptr = saved_state_addr as *mut SavedState;

    if *active_interrupt_count == 1 {
        *processor.curr_interrupt_saved_state() = saved_state_addr as *mut SavedState;
    }

    let stack_frame = &(*saved_state_ptr).stack_frame;
    let handler_fn: fn(&StackFrame, u64) = core::mem::transmute(handler_addr);
    handler_fn(stack_frame, error);

    debug_assert!(*active_interrupt_count > 0);
    *active_interrupt_count -= 1;
}

/*
 * Defines, in the first given identifier, the InterruptHandler with the address to the entry point
 * which saves the register state previous to the interrupt and calls either "handler_wrapper" or
 * "handler_with_err_wrapper" which, in turn, will then call the actual handler function.
 *
 * Function passed must receive either &StackFrame or &StackFrame and u64
 */
#[macro_export]
macro_rules! def_interrupt_handler {
    ($handler_name:ident, fn $handler_fn_name:ident($param:ident: &StackFrame) $handler_fn_body:block) => {
        fn $handler_fn_name($param: &crate::x86_64::interrupts::handler::StackFrame)
            $handler_fn_body

        #[allow(improper_ctypes)]
        extern {
            paste::paste! {
                static [<$handler_name _isr_entry_point>]: crate::x86_64::interrupts::handler::InterruptHandler;
            }
        }
        #[allow(non_upper_case_globals)]
        static $handler_name: &crate::x86_64::interrupts::handler::InterruptHandler =
            unsafe { paste::paste! { &[<$handler_name _isr_entry_point>] } };

        paste::paste! {
            core::arch::global_asm!(
                r#"
                {}:
                    push rbp
                    push r15
                    push r14
                    push r13
                    push r12
                    push r11
                    push r10
                    push r9
                    push r8
                    push rdi
                    push rsi
                    push rdx
                    push rcx
                    push rbx
                    push rax

                    lea rdi, {}  # 1st param, address to handler function
                    mov rsi, rsp # 2nd param, address to saved state
                    call {}

                    pop rax
                    pop rbx
                    pop rcx
                    pop rdx
                    pop rsi
                    pop rdi
                    pop r8
                    pop r9
                    pop r10
                    pop r11
                    pop r12
                    pop r13
                    pop r14
                    pop r15
                    pop rbp
                    iretq
                "#,
                sym [<$handler_name _isr_entry_point>],
                sym $handler_fn_name,
                sym crate::x86_64::interrupts::handler::handler_wrapper
            );
        }
    };

    // Interrupt handler with error
    ($handler_name:ident, fn $handler_fn_name:ident($param:ident: &StackFrame, $param2:ident: u64) $handler_fn_body:block) => {
        fn $handler_fn_name($param: &crate::x86_64::interrupts::handler::StackFrame, $param2: u64)
            $handler_fn_body

        #[allow(improper_ctypes)]
        extern {
            paste::paste! {
                static [<$handler_name _isr_entry_point>]: crate::x86_64::interrupts::handler::InterruptHandler;
            }
        }
        #[allow(non_upper_case_globals)]
        static $handler_name: &crate::x86_64::interrupts::handler::InterruptHandler =
            unsafe { paste::paste! { &[<$handler_name _isr_entry_point>] } };

        paste::paste! {
            core::arch::global_asm!(
                r#"
                {}:
                    # push rbp and r15, remove error from stack and store in rbp
                    push rbp
                    push r15
                    mov rbp, [rsp+16]
                    mov r15, [rsp+8]
                    mov [rsp+16], r15
                    pop r15
                    mov [rsp], r15

                    push r14
                    push r13
                    push r12
                    push r11
                    push r10
                    push r9
                    push r8
                    push rdi
                    push rsi
                    push rdx
                    push rcx
                    push rbx
                    push rax

                    lea rdi, {}  # 1st param, address to handler function
                    mov rsi, rsp # 2nd param, address to saved state
                    mov rdx, rbp # 3rd param, error code
                    call {}

                    pop rax
                    pop rbx
                    pop rcx
                    pop rdx
                    pop rsi
                    pop rdi
                    pop r8
                    pop r9
                    pop r10
                    pop r11
                    pop r12
                    pop r13
                    pop r14
                    pop r15
                    pop rbp
                    iretq
                "#,
                sym [<$handler_name _isr_entry_point>],
                sym $handler_fn_name,
                sym crate::x86_64::interrupts::handler::handler_with_err_wrapper
            );
        }
    };
}
