#![no_std]
#![feature(core_intrinsics)]


use core::{mem, arch::{asm, global_asm}, intrinsics};


pub mod logger;
pub mod kernel_loader;


// Info the bootloader passes to the kernel
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

#[repr(C, packed)]
pub struct Gdt64Descriptor {
    pub limit: u16,
    pub address: &'static Gdt64
}
#[repr(C, packed)]
pub struct Gdt64 {
    pub null: u64,
    // code desc
    pub code_limit: u16,
    pub code_base1: u16,
    pub code_base2: u8,
    pub code_access: u8,
    pub code_flagslimit: u8,
    pub code_base3: u8,
    // data desc
    pub data_limit: u16,
    pub data_base1: u16,
    pub data_base2: u8,
    pub data_access: u8,
    pub data_flagslimit: u8,
    pub data_base3: u8,
}


#[allow(improper_ctypes)]
extern {
    // from bootloader.ld
    static pml4t_address: ();
    static pdpt_address: ();
    static pdt_address: ();
    static pt_address: ();
}


// Retrieves RSDP address
pub fn get_rsdp() -> u64 {
    // "RSD PTR " signature
    let expected_sign: [u8; 8] = [82, 83, 68, 32, 80, 84, 82, 32];

    // search for RSDP in the EBDA
    let rsdp_ptr = unsafe { *(0x40E as *const u16) as usize } * 16;
    let signature = unsafe { *(rsdp_ptr as *const [u8; 8]) };

    // if not found search for RSDP in region 0xE0000 -> 0xFFFFF
    if expected_sign != signature {
        for addr in (0xE0000..0xFFFFF).step_by(0x10) {
            let signature = unsafe { *(addr as *const [u8; 8]) };
            if expected_sign == signature {
                return addr;
            }
        }
        panic!("Could not find RSDP");
    }
    else {
        return rsdp_ptr as u64;
    }
}

// Panics if CPUID isn't supported by CPU
pub fn detect_cpuid() {
    let mut is_supported: i32;

    unsafe {
        asm!(
            "pushfd",           // store original EFLAGS
            "pushfd",           // store original EFLAGS

            "mov ebx, 0x00200000",
            "xor [esp], ebx",   // invert the ID bit
            "popfd",            // load the inverted bit EFLAGS
            "pushfd",           // store EFLAGS (bit may be inverted or not)
            "pop {0}",          // copy EFLAGS (bit may be inverted or not) to EAX
            "xor eax, [esp]",   // different bits in EAX

            "popfd",            // restore original EFLAGS
            "test {0}, ebx",    // zero if ID bit can't be changed
            out(reg) is_supported,
        );
    }

    if is_supported == 0 {
        panic!("CPUID isn't supported by this CPU.");
    }
}

// Panics if Long Mode isn't supported by CPU
#[allow(unused_assignments)]
pub fn detect_long_mode() {
    let is_supported: u16;

    unsafe{
        asm!(
            "mov eax, 0x80000000",
            "cpuid",    // returns highest value CPU recognizes in eax
            "cmp eax, 0x80000001",
            "jb 1f",    // if less than 0x80000001 long mode not supported

            "mov eax, 0x80000001",
            "cpuid",    // returns CPU signature and feature bits
            "test edx, 0x20000000",
            "jz 1f",    // if 29th bit on edx is not set long mode isn't supported

            "jmp 2f",

            "1:",
            "mov {0:x}, 0",
            "jmp 3f",

            "2:",
            "mov {0:x}, 1",

            "3:",
            out(reg) is_supported,
        );
    }

    if is_supported == 0 {
        panic!("Long Mode isn't supported by this CPU.");
    }
}

// Sets up paging by identity mappping first 2MB and loading PML4T in cr3
pub unsafe fn setup_paging() {
    let pml4t_addr = &pml4t_address as *const _ as usize as *mut u64;
    let pdpt_addr  = &pdpt_address as *const _ as usize as *mut u64;
    let pdt_addr   = &pdt_address as *const _ as usize as *mut u64;
    let pt_addr    = &pt_address as *const _ as usize as *mut u64;

    // clear memory
    intrinsics::volatile_set_memory(pml4t_addr, 0, 0x4000/mem::size_of::<u64>());

    // initiate first index of each table (set first two bits for present and writable flags)
    pml4t_addr.write_volatile((pdpt_addr as u64) | 0x3);
    pdpt_addr.write_volatile((pdt_addr as u64) | 0x3);
    pdt_addr.write_volatile((pt_addr as u64) | 0x3);

    // initiate page table (don't map page 0 for guard page and so null pointers cause page fault)
    let mut aux = pt_addr.add(1);
    let mut mem_ptr = 0x1003;
    while aux < pt_addr.add(0x200) {
        aux.write_volatile(mem_ptr);
        mem_ptr += 0x1000;
        aux = aux.add(1);
    }

    // finish setting up paging
    asm!(
        "mov cr3, {}",  // move PML4 table address to cr3
        "mov eax, cr4",
        "or eax, 0x20", // flip PAE bit in cr4
        "mov cr4, eax",
        in(reg) pml4t_addr,
    );
}

// Enters compatibility mode
pub unsafe fn enter_compatibility_mode() {
    asm!(
        // set long mode and NXE bit
        "mov ecx, 0xC0000080",
        "rdmsr",
        "or eax, 0x900",
        "wrmsr",

        // enable paging
        "mov eax, cr0",
        "or eax, 0x80000000",
        "mov cr0, eax",
    );
}

// Loads GDT for Long Mode
pub unsafe fn load_gdt64(gdt64_descriptor: &'static Gdt64Descriptor) {
    asm!(
        "lgdt [{}]",
        in(reg) gdt64_descriptor,
    )
}

/*
    Jumps to long mode (loads descriptor in CS) in lmjump.s so kernel entry address can be longer than 32 bits
    Loads low kernel address in eax, high kernel address in ebx and address of bootloader_info struct in ecx
*/
global_asm!(include_str!("asm/lmjump.s"));
#[allow(improper_ctypes)]
extern {
    // from bootloader.ld
    static lm_jump: ();
}
pub fn jump_to_kernel(kernel_entry_addr: u64, bootloader_info: &BootloaderInfo) -> ! {
    let kernel_entry_addr_low = kernel_entry_addr as u32;
    let kernel_entry_addr_high = (kernel_entry_addr >> 32) as u32;

    unsafe {
        asm!(
            "push 0x8",
            "push {}",
            "retf",
            in(reg) &lm_jump,
            in("eax") kernel_entry_addr_low,
            in("ebx") kernel_entry_addr_high,
            in("ecx") bootloader_info,
        );
    }

    loop{}
}
