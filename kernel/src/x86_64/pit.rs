use core::sync::atomic::{AtomicBool, Ordering};

use crate::{def_interrupt_handler, locks::spinlock::{Spinlock, SpinlockGuard}};
use super::{cpu::instructions, interrupts};


const FREQUENCY: u32 = 1193180;
const COMMAND_PORT: u16 = 0x43;
const CHANNEL_O_PORT: u16 = 0x40;
const COMMAND_CHANNEL0_ACCESSLOHI_MODE0: u8 = 0b00110000;


static PIT: Spinlock<Pit> = Spinlock::new(Pit { divisor: 0 });
static IS_WAIT_OVER: AtomicBool = AtomicBool::new(false);


pub struct Pit {
    divisor: u16
}
impl Pit {
    pub fn prepare_wait(&mut self, hz: u32) {
        use super::{interrupts::{set_idt_entry, apic::io_apic}, structures::idt::{Index, Flags}};

        assert!(hz <= FREQUENCY);

        // channel 0, access lobyte and hibyte, mode 0
        instructions::outb(COMMAND_PORT, COMMAND_CHANNEL0_ACCESSLOHI_MODE0);

        // 0 divisor is lowest possible frequency
        self.divisor = if FREQUENCY/hz > u16::MAX as u32 { 0 }
                       else { (FREQUENCY/hz) as u16 };

        // set pit handler on IDT
        set_idt_entry(Index::SYS_TIMER, pit_handler.get_addr(), 0x8, Flags::BASE, 0);
        // direct system timer irq to be sent to current processor's lapic
        io_apic::enable_system_timer(Index::SYS_TIMER);
    }

    pub fn wait(&self) {
        // set divisor so PIT frequency
        instructions::outb(CHANNEL_O_PORT, self.divisor as u8);        // low byte
        instructions::outb(CHANNEL_O_PORT, (self.divisor >> 8) as u8); // high byte

        interrupts::hlt_wait(
            || { IS_WAIT_OVER.load(Ordering::Acquire) }
        );
        IS_WAIT_OVER.store(false, Ordering::Release);
    }
}


pub fn lock() -> SpinlockGuard<'static, Pit> {
    PIT.lock()
}
pub fn unlock(pit: SpinlockGuard<'static, Pit>) {
    pit.unlock();
}

def_interrupt_handler!(pit_handler,
    fn pit_handler_fn(_stack_frame: &StackFrame) {
        use interrupts::apic::lapic;
        IS_WAIT_OVER.store(true, Ordering::Release);
        lapic::eoi();
    }
);
