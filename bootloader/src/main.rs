#![no_std]
#![no_main]
#![feature(panic_info_message)]


use core::{arch::global_asm, mem};

use bootloader::{
    print, println,
    Gdt64Descriptor, Gdt64,
    BootloaderInfo, logger::{Logger, LOGGER},
    kernel_loader::KernelLoader
};


global_asm!(include_str!("asm/stage1.s"));
global_asm!(include_str!("asm/stage2.s"));


#[allow(improper_ctypes)]
extern {
    // from stage1/2.s
    static drive_code: u8;
    static vbe_mode_info_structure: [u8; 256];
    // from bootloader.ld
    static memory_map: ();
    static vga_bitmap_font: [[u8; 16]; 256];
    static conventional_mem_addr: ();
    static start_addr_kernel: ();
    static end_addr_kernel: ();
    static kernel_addr: ();
}

static mut BOOTLOADER_INFO: BootloaderInfo = BootloaderInfo {
    drive_code: 0,
    vesa_mode_info_addr: 0,
    memory_map_addr: 0,
    vga_bitmap_font_addr: 0,
    rsdp_addr: 0,
    kernel_load_addr: 0,
    kernel_elf_size: 0,
    bss_start_addr: 0,
    bss_size: 0,
    conventional_mem_addr: 0
};

const GDT64_DESCRIPTOR: Gdt64Descriptor = Gdt64Descriptor {
    limit: mem::size_of::<Gdt64>() as u16 - 1,
    address: &GDT64
};
const GDT64: Gdt64 = Gdt64 {
    null: 0,
    code_limit: 0xFFFF,
    code_base1: 0,
    code_base2: 0,
    code_access: 0x9A,
    code_flagslimit: 0xAF,
    code_base3: 0,
    data_limit: 0xFFFF,
    data_base1: 0,
    data_base2: 0,
    data_access: 0x92,
    data_flagslimit: 0xCF,
    data_base3: 0,
};


#[no_mangle]
unsafe fn stage3_start() -> ! {
    // initialize logger
    LOGGER.write(Logger::new(&vbe_mode_info_structure, &vga_bitmap_font));
    println!("Booting third stage...");
    // initialize kernel loader
    let kernel_loader = KernelLoader::new(&kernel_addr as *const _ as usize);

    // fill up bootloader info structure
    BOOTLOADER_INFO.drive_code = drive_code;
    BOOTLOADER_INFO.vesa_mode_info_addr = &vbe_mode_info_structure as *const _ as u64;
    BOOTLOADER_INFO.memory_map_addr = &memory_map as *const _ as u64;
    BOOTLOADER_INFO.vga_bitmap_font_addr = &vga_bitmap_font as *const _ as u64;
    BOOTLOADER_INFO.rsdp_addr = bootloader::get_rsdp();
    BOOTLOADER_INFO.kernel_load_addr = &kernel_addr as *const _ as u64;
    BOOTLOADER_INFO.kernel_elf_size = &end_addr_kernel as *const _ as u64 - &start_addr_kernel as *const _ as u64;
    let (bss_start_addr, bss_size) = kernel_loader.get_bss();
    BOOTLOADER_INFO.bss_start_addr = bss_start_addr;
    BOOTLOADER_INFO.bss_size = bss_size;
    BOOTLOADER_INFO.conventional_mem_addr = &conventional_mem_addr as *const _ as u64;

    // these panic if not supported
    bootloader::detect_cpuid();
    bootloader::detect_long_mode();

    // identity maps first 2MB and loads PML4T in cr3
    bootloader::setup_paging();
    // maps virtual memory for kernel segments
    kernel_loader.load_segments();

    println!("Entering long mode and jumping to kernel...");

    // enables long mode bit, paging and loads gdt for long mode
    bootloader::enter_compatibility_mode();
    bootloader::load_gdt64(&GDT64_DESCRIPTOR);

    // retrieve entry address and jump to kernel
    let kernel_entry_addr = kernel_loader.get_entry_address();
    drop(kernel_loader);
    bootloader::jump_to_kernel(kernel_entry_addr, &BOOTLOADER_INFO);
}


use core::panic::PanicInfo;

// This function is called on panic.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    print!("ERROR: ");
    if let Some(err) = info.message() {
        println!("{}", err);
    }
    else {
        println!("Panic ocurred.");
    }

    loop {}
}
