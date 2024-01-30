#![no_std]
#![no_main]


use kernel::{
    BootloaderInfo, x86_64,
    memory::address::PhysAddr,
    scheduler::{self, task::Task}
};


#[no_mangle]
fn _start() -> ! {
    // retrieve bootloader_info structure address from rcx register
    let mut bootloader_info = unsafe { &mut *(x86_64::cpu::registers::rcx::read() as *mut BootloaderInfo) };

    // sets up paging, heap, interrupts, smp and timer
    if let Err(str) = kernel::setup(&mut bootloader_info) {
        panic!("Panicked during setup: {}", str);
    }

    kernel::drivers::keyboard::init();
    let vbe_mode_info_addr = PhysAddr::new(bootloader_info.vesa_mode_info_addr as usize).to_virtual();
    let vbe_mode_info =  unsafe { &*vbe_mode_info_addr.as_ptr::<kernel::video::vesa::VBEModeInfo>() };
    let vga_bitmap_font_addr = PhysAddr::new(bootloader_info.vga_bitmap_font_addr as usize).to_virtual();
    kernel::video::terminal::init(vbe_mode_info, vga_bitmap_font_addr, 100);

    let terminal_task = Task::new(32768, kernel::video::terminal::terminal_task, None);
    scheduler::add_task(terminal_task);

    scheduler::enable_preemption();
    scheduler::schedule();

    loop { unreachable!(); }
}
