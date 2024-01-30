#![no_std]
#![feature(core_intrinsics)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(const_mut_refs)]

extern crate alloc;


pub mod utils;
pub mod x86_64;
pub mod locks;
pub mod memory;
pub mod drivers;
pub mod video;
pub mod processor;
pub mod time;
pub mod scheduler;


// Needs to be the exact same as the struct in ../../bootloader/src/lib.rs
pub struct BootloaderInfo {
    pub drive_code: u8,
    pub vesa_mode_info_addr: u64,
    pub memory_map_addr: u64,
    pub vga_bitmap_font_addr: u64,
    pub rsdp_addr: u64,
    pub kernel_load_addr: u64,
    pub kernel_elf_size: u64,
    pub bss_start_addr: u64,
    pub bss_size: u64,
    /*
        Start of conventional mem not used by bootloader.
        Used by kernel for allocating tables to map physical memory
    */
    pub conventional_mem_addr: u64
}


// Sets up gdt, interrupts, memory, logger and heap
pub fn setup(bootloader_info: &mut &mut BootloaderInfo) -> Result<(), &'static str> {
    use x86_64::{cpu, structures::{gdt, acpi}, interrupts};
    use memory::{
        FrameSize, FrameAllocator, address::PhysAddr,
        e820_memory_map::{self, MemoryMap}, kalloc
    };
    use video::{vesa::VBEModeInfo, color, logger};

    // memsets the bss section to 0
    zero_out_bss(bootloader_info);

    // maps first 2mb to virtual memory at set offset
    map_first_2mb(bootloader_info);

    // convert bootloader_info struct to virtual address
    let bootloader_info_addr = PhysAddr::new(*bootloader_info as *const _ as usize).to_mut_virtual();
    *bootloader_info = unsafe { &mut *bootloader_info_addr.as_ptr::<BootloaderInfo>() };

    // initialize memory map
    let memory_map_addr = PhysAddr::new(bootloader_info.memory_map_addr as usize).to_mut_virtual();
    let memory_map = unsafe { &mut *memory_map_addr.as_ptr::<MemoryMap>() };
    e820_memory_map::init(memory_map, bootloader_info.kernel_load_addr as usize,
                          bootloader_info.kernel_elf_size as usize)?;
    // start of unused conventional memory as reported by bootloader
    let start_conventional_addr = PhysAddr::new(bootloader_info.conventional_mem_addr as usize);
    // initialize frame allocator
    let mut frame_allocator = FrameAllocator::new(
        memory_map, start_conventional_addr, FrameSize::FourKb
    );

    // initialize vbe mode info struct
    let vbe_mode_info_addr = PhysAddr::new(bootloader_info.vesa_mode_info_addr as usize).to_virtual();
    let vbe_mode_info = unsafe { &*vbe_mode_info_addr.as_ptr::<VBEModeInfo>() };
    // map framebuffer to virtual memory at set offset
    map_framebuffer(vbe_mode_info, &mut frame_allocator)?;

    // initialize color builder
    color::init(vbe_mode_info);
    // initialize logger
    let vga_bitmap_font_addr = PhysAddr::new(bootloader_info.vga_bitmap_font_addr as usize).to_virtual();
    logger::init(vbe_mode_info, vga_bitmap_font_addr, color::GREY);

    // initialize and load gdt
    gdt::init();
    gdt::load();

    // have to use this macro to print here since interrupts aren't setup yet
    no_enable_irq_print!("Mapping physical memory: ");
    // map physical memory past first 2MB detected by the e820 memory map structure to virtual memory at set offset
    map_physical_memory(memory_map, &mut frame_allocator)?;
    no_enable_irq_print_color!(color::DARK_GREEN, "DONE.\n");

    no_enable_irq_print!("Initializing heap: ");
    // initialize heap
    kalloc::init_heap(&mut frame_allocator)?;
    no_enable_irq_print_color!(color::DARK_GREEN, "DONE.\n");

    // retrieve and validate system description pointer and table
    let rsdp_addr = PhysAddr::new(bootloader_info.rsdp_addr as usize).to_virtual();
    acpi::init_rsdp_and_rsdt(rsdp_addr)?;
    acpi::init_madt()?;
    let madt = acpi::get_madt();
    // map apic MMIO addresses retrieved from MADT
    map_apic_registers(madt.get_lapic_addr(), madt.get_io_apic_addr_base_0()?, &mut frame_allocator)?;

    // initialize hardware interrupts
    interrupts::init_hardware_interrupts()?;

    // register bootstrap processor struct
    processor::register_bsp();

    // fill bsp idt with exception handlers and load it
    interrupts::fill_and_load_idt();

    // enable interrupts
    cpu::instructions::sti();

    // initialize bootstrap processor lapic and timer
    let bsp = processor::get();
    bsp.lapic().enable();
    bsp.timer().init();

    // initialize smp
    cpu::smp::init();

    // remove first 2mb identity mapping
    remove_first_2mb_identity_mapping();

    Ok(())
}

fn zero_out_bss(bootloader_info: &BootloaderInfo) {
    use core::intrinsics::volatile_set_memory;
    let ptr = bootloader_info.bss_start_addr as *mut u8;
    unsafe { volatile_set_memory(ptr, 0, bootloader_info.bss_size as usize); }
}

// Maps first 2mb to virtual memory at set offset
fn map_first_2mb(bootloader_info: &mut BootloaderInfo) {
    use core::intrinsics::volatile_set_memory;
    use x86_64::cpu::registers;
    use memory::{
        address::{PhysAddr, VirtualAddress, VirtAddr, MutVirtAddr},
        paging::{Table, TableLevel, Flags}
    };

    let mut next_table_addr = MutVirtAddr::new(bootloader_info.conventional_mem_addr as usize);

    // map first 2MB
    unsafe {
        let virt_base = memory::address::PHYS_MEM_VIRT_ADDR;
        let mut table4 = Table::new(VirtAddr::new(registers::cr3::read() as usize), TableLevel::Four);

        volatile_set_memory(next_table_addr.as_ptr::<u8>(), 0, 0x1000);
        let t3_addr: PhysAddr = next_table_addr.as_usize().into();
        table4.set_entry(t3_addr, Flags::PRESENT | Flags::WRITABLE, virt_base.get_entry(TableLevel::Four));
        let mut table3 = Table::new(VirtAddr::new(t3_addr.as_usize()), TableLevel::Three);

        next_table_addr = next_table_addr.offset::<u8>(0x1000);

        volatile_set_memory(next_table_addr.as_ptr::<u8>(), 0, 0x1000);
        let t2_addr: PhysAddr = next_table_addr.as_usize().into();
        table3.set_entry(t2_addr, Flags::PRESENT | Flags::WRITABLE, virt_base.get_entry(TableLevel::Three));
        let mut table2 = Table::new(VirtAddr::new(t2_addr.as_usize()), TableLevel::Two);

        next_table_addr = next_table_addr.offset::<u8>(0x1000);
        bootloader_info.conventional_mem_addr = next_table_addr.as_usize() as u64;

        let first_frame = PhysAddr::new(0x0);
        table2.set_entry(first_frame, Flags::PRESENT | Flags::WRITABLE | Flags::HUGE, 0)
    }
}

fn map_physical_region(memory_region: memory::MemoryRegion,
    frame_allocator: &mut memory::FrameAllocator) -> Result<(), ()>
{
    use memory::{
        FrameSize,
        address::{PhysAddr, VirtualAddress},
        paging::{Table, TableLevel, Flags}
    };

    for frame in memory_region.iter(FrameSize::TwoMb) {
        let virt_addr = PhysAddr::new(frame).to_virtual();
        let mut table = virt_addr.get_table();

        while table.level != TableLevel::Two {
            let entry = virt_addr.get_entry(table.level);
            if let Some(phys_frame_addr) = frame_allocator.get_next_frame() {
                unsafe {
                    table.map_table_at(phys_frame_addr.to_mut_virtual(), Flags::PRESENT | Flags::WRITABLE, entry);
                }
                table = Table::new(phys_frame_addr.to_virtual(), table.level.get_next_level().unwrap());
            }
            else {
                return Err(());
            }
        }

        // map with huge page (2MB per entry)
        let t2_entry = virt_addr.get_entry(TableLevel::Two);
        table.set_entry(PhysAddr::new(frame), Flags::PRESENT | Flags::WRITABLE | Flags::HUGE, t2_entry);
    }

    Ok(())
}

fn map_framebuffer(vbe_mode_info: &video::vesa::VBEModeInfo,
    frame_allocator: &mut memory::FrameAllocator) -> Result<(), &'static str>
{
    use memory::MemoryRegion;

    let length = vbe_mode_info.length();
    let memory_region = MemoryRegion::new(vbe_mode_info.framebuffer_addr().as_usize(), length);
    if let Err(_) = map_physical_region(memory_region, frame_allocator) {
        return Err("Insufficient physical memory for mapping framebuffer");
    }
    Ok(())
}

/*
    Maps entire physical memory reported by the BIOS in fixed virtual memory offset
    with 2MB level 2 table pages.
    Uses conventional memory not taken by the bootloader for storing the tables.
    If out of conventional memory starts using memory right after the kernel elf.
*/
fn map_physical_memory(memory_map: &memory::e820_memory_map::MemoryMap,
    frame_allocator: &mut memory::FrameAllocator) -> Result<(), &'static str>
{
    use memory::MemoryRegion;

    // map all 2MB frames reported by the e820 memory map
    for entry in memory_map {
        let base = entry.base as usize;
        let length = entry.length as usize;
        let memory_region = MemoryRegion::new(base, length);

        if let Err(_) = map_physical_region(memory_region, frame_allocator) {
            return Err("Insufficient physical memory for mapping physical memory");
        }
    }
    Ok(())
}

fn map_apic_registers(lapic_base_addr: memory::address::PhysAddr, io_apic_base_addr: memory::address::PhysAddr,
    frame_allocator: &mut memory::FrameAllocator) -> Result<(), &'static str>
{
    use memory::MemoryRegion;

    let memory_region = MemoryRegion::new(lapic_base_addr.as_usize(), 0x1000);
    if let Err(_) = map_physical_region(memory_region, frame_allocator) {
        return Err("Insufficient physical memory for mapping apic registers");
    }

    // this is probably already by mapped by the above function call but just to be sure
    let memory_region = MemoryRegion::new(io_apic_base_addr.as_usize(), 0x1000);
    if let Err(_) = map_physical_region(memory_region, frame_allocator) {
        return Err("Insufficient physical memory for mapping apic registers");
    }

    Ok(())
}

// Remove first 2mb identity mapping
fn remove_first_2mb_identity_mapping() {
    use x86_64::cpu::registers;
    use memory::paging::{Table, TableEntry};

    let table4 = Table::table4();
    let table3 = if let Some(TableEntry::Table { table, .. }) = table4.get_entry(0) {
        table
    }
    else {
        unreachable!();
    };
    let table2 = if let Some(TableEntry::Table { table, .. }) = table3.get_entry(0) {
        table
    }
    else {
        unreachable!();
    };
    let mut table1 = if let Some(TableEntry::Table { table, .. }) = table2.get_entry(0) {
        table
    }
    else {
        unreachable!();
    };
    // removes all mappings except 0x1000-0x8000 because of stack
    for i in 8..512 {
        table1.remove_entry(i);
    }

    // flush tlb
    registers::cr3::flush_tlb();
}


// This function is called on alloc error.
#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("Allocation error: {:?}", layout)
}


use core::panic::PanicInfo;

// This function is called on panic.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    use x86_64::{cpu::{self, smp}, interrupts::apic::lapic, structures::idt::Index};

    cpu::instructions::cli();

    if smp::is_init() {
        lapic::broadcast_ipi(Index::HALT);
    }

    crate::video::logger::LOGGER.lock().clear_screen();
    no_enable_irq_print_color!(video::color::RED, "{info}\n");
    loop { x86_64::cpu::instructions::hlt(); }
}
