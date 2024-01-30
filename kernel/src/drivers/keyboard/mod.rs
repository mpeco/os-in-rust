use crate::{
    def_interrupt_handler,
    x86_64, utils::{lazy_static::LazyStatic, atomic},
    scheduler::{self, task::TaskId}
};


pub mod scancode;


const SCANCODE_QUEUE_SIZE: usize = 100;
const PS2_CONTROLLER_DATA_PORT: u16 = 0x60;
const PS2_CONTROLLER_STATUS_PORT: u16 = 0x64;
const PS2_CONTROLLER_STATUS_SCANCODE_FULL: u8 = 0x1;


static mut SCANCODE_QUEUE: LazyStatic<atomic::ArrayQueue<u8>> = LazyStatic::new();
static mut HALTED_TASK_ID: Option<TaskId> = None;


pub fn init() {
    use x86_64::{interrupts::{self, apic::io_apic}, structures::idt::{Index, Flags}};

    // init keyboard scancode queue
    let scancode_queue = atomic::ArrayQueue::<u8>::new(SCANCODE_QUEUE_SIZE)
                                            .expect("Unsufficient memory for keyboard driver");
    unsafe { SCANCODE_QUEUE.init(scancode_queue); }

    // set handler for keyboard interrupt
    interrupts::set_idt_entry(
        Index::KEYBOARD, keyboard_handler.get_addr(), 0x8, Flags::BASE, 0
    );

    // enable keyboard interrupt
    io_apic::enable_keyboard(Index::KEYBOARD);
    // flush output buffer
    crate::x86_64::cpu::instructions::inb(PS2_CONTROLLER_DATA_PORT);
}

pub fn retrieve_scancode() -> u8 {
    let queue = unsafe { &mut *SCANCODE_QUEUE };
    let mut scancode: Option<u8> = None;

    while scancode.is_none() {
        if let Some(retrieved_scancode) = queue.pop() {
            scancode = Some(retrieved_scancode);
        }
        else {
            scheduler::yield_on_condition(|| {
                scancode = queue.pop();
                if scancode.is_none() {
                    unsafe { HALTED_TASK_ID = Some(scheduler::get_executing_task_id()); }
                    true
                }
                else {
                    false
                }
            });
        }
    }

    scancode.unwrap()
}


def_interrupt_handler!(keyboard_handler,
    fn keyboard_handler_fn(_stack_frame: &StackFrame) {
        use x86_64::interrupts::apic;

        let scancode_status = x86_64::cpu::instructions::inb(PS2_CONTROLLER_STATUS_PORT) & 1;
        if scancode_status == PS2_CONTROLLER_STATUS_SCANCODE_FULL {
            let scancode = x86_64::cpu::instructions::inb(PS2_CONTROLLER_DATA_PORT);
            unsafe {
                if let Ok(_) = SCANCODE_QUEUE.push(scancode) {
                    if let Some(task_id) = HALTED_TASK_ID.take() {
                        scheduler::wake_up_task(task_id);
                    }
                }
                else {
                    crate::println_color!(crate::video::color::SAFETY_YELLOW, "\nWARNING: Failed to push scancode to queue, keypress dropped."); // FIXME
                }
            }
        }

        apic::lapic::eoi();
    }
);
