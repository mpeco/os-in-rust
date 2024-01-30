use core::{cell::UnsafeCell, ptr};
use alloc::collections::BTreeMap;

use crate::{
    time::timer::Timer, utils::lazy_static::LazyStatic, scheduler::Scheduler,
    x86_64::{interrupts::{apic::lapic::{self, Lapic}, handler}, structures::idt::Idt}
};


static mut PROCESSORS: BTreeMap<u32, Processor> = BTreeMap::new();
static BSP_LAPIC_ID: LazyStatic<u32> = LazyStatic::new();


pub struct Processor {
    idt: UnsafeCell<Idt>,
    lapic: UnsafeCell<Lapic>,
    timer: UnsafeCell<Timer>,
    active_interrupt_count: UnsafeCell<u64>, // number of interrupts currently being handled
    curr_interrupt_saved_state: UnsafeCell<*mut handler::SavedState>,
    scheduler: UnsafeCell<Scheduler>
}
impl Processor {
    pub fn new() -> Processor {
        Processor{
            idt: UnsafeCell::new(Idt::new()),
            lapic: UnsafeCell::new(Lapic::new()),
            timer: UnsafeCell::new(Timer::new()),
            active_interrupt_count: UnsafeCell::new(0),
            curr_interrupt_saved_state: UnsafeCell::new(ptr::null_mut()),
            scheduler: UnsafeCell::new(Scheduler::new())
        }
    }

    /**
     * Only the processor to which this structure pertains should have access
     * to it, so race conditions should never happen
     */
    pub fn idt_descriptor(&self) -> &mut Idt {
        unsafe { &mut *self.idt.get() }
    }
    pub fn lapic(&self) -> &mut Lapic {
        unsafe { &mut *self.lapic.get() }
    }
    pub fn timer(&self) -> &mut Timer {
        unsafe { &mut *self.timer.get() }
    }
    pub fn active_interrupt_count(&self) -> &mut u64 {
        unsafe { &mut *self.active_interrupt_count.get() }
    }
    pub fn curr_interrupt_saved_state(&self) -> &mut *mut handler::SavedState {
        unsafe { &mut *self.curr_interrupt_saved_state.get() }
    }
    pub fn scheduler(&self) -> &mut Scheduler {
        unsafe { &mut *self.scheduler.get() }
    }
}


pub fn register_bsp() {
    BSP_LAPIC_ID.init(lapic::get_id());
    unsafe { PROCESSORS.insert(*BSP_LAPIC_ID, Processor::new()); }
}
pub fn register(lapic_id: u32) {
    assert!(BSP_LAPIC_ID.is_init(), "Attempted to register processor before registering BSP");
    assert_eq!(lapic::get_id(), *BSP_LAPIC_ID, "Can't call register_processor from non BSP");
    // safe since only BSP will be reaching this
    unsafe { PROCESSORS.insert(lapic_id, Processor::new()); }
}
pub fn unregister(lapic_id: u32) {
    assert!(BSP_LAPIC_ID.is_init(), "Attempted to unregister processor before registering BSP");
    assert_eq!(lapic::get_id(), *BSP_LAPIC_ID, "Can't call unregister_processor from non BSP");
    // safe since only BSP will be reaching this
    unsafe { PROCESSORS.remove(&lapic_id); }
}

/*
 * Retrieves the processor struct for the bootstrap processor,
 * potentially allowing concurrent mutable access to its fields
 */
pub unsafe fn get_bsp() -> &'static Processor {
    // should never fail
    unsafe { PROCESSORS.get(&*BSP_LAPIC_ID).unwrap() }
}

// Retrieves the processor struct for the processor currently executing
pub fn get() -> &'static Processor {
    // should never fail
    unsafe { PROCESSORS.get_mut(&lapic::get_id()).unwrap() }
}
