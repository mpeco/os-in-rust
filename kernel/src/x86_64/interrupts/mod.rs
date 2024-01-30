use crate::{def_interrupt_handler, processor, x86_64::{cpu, structures::{acpi, idt}}};

pub mod apic;
pub mod handler;


#[inline(never)]
// Fill IDT with exception handlers and load it
pub fn fill_and_load_idt() {
    use idt::{Index, Flags};

    let idt_descriptor = processor::get().idt_descriptor();

    // fill up IDT for exceptions
    idt_descriptor.set_entry(
        Index::BREAKPOINT, breakpoint_handler.get_addr(), 0x8, Flags::BASE | Flags::TRAP_GATE, 0
    );
    idt_descriptor.set_entry(
        Index::DOUBLE_FAULT, double_fault_handler.get_addr(), 0x8, Flags::BASE | Flags::TRAP_GATE, 0
    );
    idt_descriptor.set_entry(
        Index::GENERAL_PROTECTION_FAULT, general_protection_fault_handler.get_addr(), 0x8, Flags::BASE | Flags::TRAP_GATE, 0
    );
    idt_descriptor.set_entry(
        Index::PAGE_FAULT, page_fault_handler.get_addr(), 0x8, Flags::BASE | Flags::TRAP_GATE, 0
    );
    idt_descriptor.set_entry(
        Index::HALT, halt_handler.get_addr(), 0x8, Flags::BASE | Flags::TRAP_GATE, 0
    );

    idt_descriptor.load();
}

def_interrupt_handler!(breakpoint_handler,
    fn breakpoint_handler_fn(stack_frame: &StackFrame) {
        crate::println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame); // FIXME
    }
);
def_interrupt_handler!(double_fault_handler,
    fn double_fault_handler_fn(stack_frame: &StackFrame, _error: u64) {
        panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
    }
);
def_interrupt_handler!(general_protection_fault_handler,
    fn general_protection_fault_handler_fn(stack_frame: &StackFrame, error: u64) {
        panic!("EXCEPTION: GENERAL PROTECTION FAULT - ERROR: {:#x}\n{:#?}", error, stack_frame);
    }
);
def_interrupt_handler!(page_fault_handler,
    fn page_fault_handler_fn(stack_frame: &StackFrame, error: u64) {
        let cr2 = cpu::registers::cr2::read();
        panic!("EXCEPTION: PAGE FAULT - ERROR: {:#x} - CR2: {:#x}\n{:#?}", error, cr2, stack_frame);
    }
);
def_interrupt_handler!(halt_handler,
    fn halt_handler_fn(_stack_frame: &StackFrame) {
        cpu::instructions::cli();
        cpu::instructions::hlt();
    }
);


pub fn init_hardware_interrupts() -> Result<(), &'static str> {
    // initialize APIC
    let madt = acpi::get_madt();
    apic::init_apic(madt)?;

    Ok(())
}


pub fn set_task_priority_level(level: u8) {
    assert!(level <= 0xF);
    cpu::registers::cr8::write(level as u64);
}

pub fn set_idt_entry(index: u8, fn_ptr: usize, selector: u16, flags: u8, ist_index: u8) {
    let idt_descriptor = processor::get().idt_descriptor();
    idt_descriptor.set_entry(index, fn_ptr, selector, flags, ist_index);
}
pub fn remove_idt_entry(index: u8) {
    let idt_descriptor = processor::get().idt_descriptor();
    idt_descriptor.clear_entry(index);
}


// Executes given closure with interrupts disabled
pub fn interrupts_disabled<F>(closure: F)
    where F: FnOnce()
{
    use cpu::registers::rflags;

    let were_interrupts_enabled = rflags::is_flag_enabled(rflags::FLAG_INTERRUPT_ENABLED);

    cpu::instructions::cli();
    closure();
    if were_interrupts_enabled {
        cpu::instructions::sti();
    }
}

/*
 * Checks condition given by closure in a loop until it returns true,
 * halting in between checks
 */
pub fn hlt_wait<F>(condition: F)
    where F: Fn() -> bool
{
    use cpu::registers::rflags;

    let were_interrupts_enabled = rflags::is_flag_enabled(rflags::FLAG_INTERRUPT_ENABLED);

    cpu::instructions::cli();
    while condition() == false {
        cpu::instructions::sti_hlt();
        cpu::instructions::cli();
    }
    if were_interrupts_enabled {
        cpu::instructions::sti();
    }
}
