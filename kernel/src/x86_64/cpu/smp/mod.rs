use core::{
    arch::global_asm, intrinsics::volatile_copy_memory,
    sync::atomic::{AtomicBool, Ordering}, mem, ptr
};
use alloc::alloc::{alloc, dealloc, Layout};

use crate::{
    memory::{address::VirtualAddress, paging}, ms, us, processor, scheduler::task::Task,
    time::{Time, timer}, utils::init_once::InitOnce,
    x86_64::{structures::acpi, interrupts::{self, apic::lapic}, cpu}
};


const TRAMPOLINE_ADDR: u32  = 0x8000;
const AP_TEMP_STACK_LENGTH: usize = 4096;
const INIT_AP_STACK_LENGTH: usize = 32768;

const WAS_TRAMPOLINE_EXECUTED_MAX_TRIES: u8 = 5;
const WAS_TRAMPOLINE_EXECUTED_TIME_PER_TRY: Time = us!(500);


global_asm!(include_str!("asm/trampoline.s"));


#[allow(improper_ctypes)]
extern {
    // from trampoline.s
    static trampoline_start: ();
    static trampoline_end: ();
    static mut pml4_addr_0x8080: u64;
    static mut init_ap_fn_addr_0x8088: u64;
    static mut stack_top_addr_ptr_0x8090: u64;
    static mut trampoline_lock_addr_0x8098: u64;
}

static IS_SMP_INIT: InitOnce = InitOnce::new();
static BSP_LOCK: AtomicBool = AtomicBool::new(true);
static INIT_AP_LOCK: AtomicBool = AtomicBool::new(true);


pub fn is_init() -> bool {
    IS_SMP_INIT.is_init()
}

#[allow(unused_assignments)]
pub fn init() {
    IS_SMP_INIT.init().expect("Attempted to initialize SMP more than once");

    let mut curr_ap_stack_top_addr: usize = 0;
    let mut trampoline_lock: u8 = 1;

    unsafe {
        // fill values to be used in trampoline code
        let table4 = paging::Table::table4();
        pml4_addr_0x8080 = table4.address.to_phys().unwrap().as_usize() as u64;
        init_ap_fn_addr_0x8088 = init_ap as u64;
        stack_top_addr_ptr_0x8090 = &curr_ap_stack_top_addr as *const _ as u64;
        trampoline_lock_addr_0x8098 = &trampoline_lock as *const _ as u64;

        let trampoline_dst = TRAMPOLINE_ADDR as *mut u8;
        let trampoline_src = &trampoline_start as *const _ as usize as *const u8;
        let trampoline_len = &trampoline_end as *const _ as usize - trampoline_src as usize;

        volatile_copy_memory(trampoline_dst, trampoline_src, trampoline_len);
    }

    let bsp_id = lapic::get_id();
    let madt = acpi::get_madt();
    for entry in madt.processor_lapic_iter()
        .filter(|e| e.get_id() != bsp_id)
    {
        curr_ap_stack_top_addr = unsafe { alloc_temp_stack() } + AP_TEMP_STACK_LENGTH;

        let lapic_id = entry.get_id();
        processor::register(lapic_id);

        trampoline_lock = 1;
        // send IPIs to init AP
        lapic::send_init_ipi(lapic_id);
        timer::wait(ms!(10));
        lapic::send_startup_ipi(lapic_id, TRAMPOLINE_ADDR);
        timer::wait(us!(200));
        lapic::send_startup_ipi(lapic_id, TRAMPOLINE_ADDR);
        trampoline_lock = 0;

        // wait for AP to unlock BSP
        let mut was_ap_init = false;
        for _ in 0..WAS_TRAMPOLINE_EXECUTED_MAX_TRIES {
            if BSP_LOCK.load(Ordering::Acquire) == false {
                was_ap_init = true;
                break;
            }
            timer::wait(WAS_TRAMPOLINE_EXECUTED_TIME_PER_TRY);
        }
        BSP_LOCK.store(true, Ordering::SeqCst);

        // if AP wasn't initialized unregister it
        if was_ap_init == false {
            processor::unregister(lapic_id);
        }
    }

    INIT_AP_LOCK.store(false, Ordering::Release);
}

// Allocates the temp stack and returns its address
unsafe fn alloc_temp_stack() -> usize {
    // allocate the buffer
    let layout = Layout::from_size_align(
        mem::size_of::<u8>()*AP_TEMP_STACK_LENGTH, mem::align_of::<u8>()
    ).unwrap();
    let buffer = alloc(layout) as *mut u8;
    assert_ne!(buffer, ptr::null_mut(), "Unsufficient memory to allocate stack");
    buffer as usize
}
// Deallocates the temp stack from "alloc_temp_stack"
unsafe fn dealloc_temp_stack(stack_buf_addr: usize) {
    let layout = Layout::from_size_align(
        mem::size_of::<u8>()*AP_TEMP_STACK_LENGTH, mem::align_of::<u8>()
    ).unwrap();
    dealloc(stack_buf_addr as *mut u8, layout);
}

extern "sysv64" fn init_ap(stack_top_addr: usize) {
    BSP_LOCK.store(false, Ordering::Release);

    // Wait for all APs to get to this point
    while INIT_AP_LOCK.load(Ordering::Acquire) == true {
        core::hint::spin_loop();
    }

    crate::x86_64::structures::gdt::load();

    let stack_buf =
        (stack_top_addr - AP_TEMP_STACK_LENGTH) as *const [u8; AP_TEMP_STACK_LENGTH];

    let scheduler = processor::get().scheduler();
    scheduler.add_task(
        Task::new(INIT_AP_STACK_LENGTH, init_ap_task, Some(stack_buf))
    );
    scheduler.schedule();
}

// AP initialization task FIXME
fn init_ap_task(args: *const [u8; AP_TEMP_STACK_LENGTH]) {
    let stack_buf_addr = args as usize;
    unsafe { dealloc_temp_stack(stack_buf_addr); }

    interrupts::fill_and_load_idt();

    let processor = processor::get();
    processor.lapic().enable();

    cpu::instructions::sti();

    let timer = processor.timer();
    timer.init();

    processor.scheduler().enable_preemption();

    crate::println!("PROC ID: {}: INITIALIZED", lapic::get_id());

    loop {
        cpu::instructions::cli();
        cpu::instructions::hlt();
    }
}
