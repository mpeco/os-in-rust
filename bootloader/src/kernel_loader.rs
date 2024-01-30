use core::{mem, intrinsics};


// Page tables for mapping kernel segments
#[allow(improper_ctypes)]
extern {
    static k_pdpt_address: ();
    static k_pdt_address: ();
    static k_pt_address: ();
}


// Loads kernel ELF
pub struct KernelLoader {
    kernel_addr: usize,
    e_phoff: u64,     // offset to first program header
    e_phentsize: u16, // size of each program header
    e_phnum: u16,     // number of program headers
}
impl KernelLoader {
    pub fn new(kernel_addr: usize) -> KernelLoader {
        let kernel_elfb = kernel_addr as *const u8;
        let kernel_elfw = kernel_addr as *const u16;
        let kernel_elfd = kernel_addr as *const u32;
        let kernel_elfq = kernel_addr as *const u64;

        let e_phoff: u64;
        let e_phentsize: u16;
        let e_phnum: u16;

        unsafe {
            // check magic bytes, elf64 and little endian
            if *kernel_elfd != 0x464C457F || *kernel_elfw.add(2) != 0x0102 {
                panic!("Kernel ELF invalid.");
            }
            // check if elf type is executable
            if *kernel_elfb.add(16) != 2 {
                panic!("Kernel ELF not of executable type.");
            }

            e_phoff = *(kernel_elfq.add(4));
            e_phentsize = *(kernel_elfw.add(27));
            e_phnum = *(kernel_elfw.add(28));
        }

        KernelLoader { kernel_addr, e_phoff, e_phentsize, e_phnum }
    }

    /*
        Maps virutal memory for kernel segments
        Would need to be updated if kernel loadable segments size > 2MB
    */
    pub unsafe fn load_segments(&self) {
        let k_pdpt_addr = &k_pdpt_address as *const _ as usize as *mut u64;
        let k_pdt_addr  = &k_pdt_address as *const _ as usize as *mut u64;
        let k_pt_addr   = &k_pt_address as *const _ as usize as *mut u64;

        let first_pheader = (self.kernel_addr as *const u8).add(self.e_phoff as usize);

        // clear memory
        intrinsics::volatile_set_memory(k_pdpt_addr, 0, 0x3000/mem::size_of::<u64>());

        // for each segment
        let mut are_tables_initialized = false;
        for i in 0..self.e_phnum {
            let pheader = first_pheader.add((self.e_phentsize * i) as usize) as *const u32;

            // if type of segment isn't load
            if *pheader != 1 { continue; }

            let phdr_flags  = *(pheader.add(1) as *const u32);
            let phdr_offset  = *(pheader.add(2) as *const u64);
            let phdr_vaddr  = *(pheader.add(4) as *const u64);
            let phdr_memsz  = *(pheader.add(10) as *const u64);

            if !are_tables_initialized {
                let pml4t_entry = ((phdr_vaddr << 16) >> 55) as usize;
                let pdpt_entry  = ((phdr_vaddr << 25) >> 55) as usize;
                let pdt_entry   = ((phdr_vaddr << 34) >> 55) as usize;

                let pml4t_addr    = &crate::pml4t_address as *const _ as usize as *mut u64;
                let mut pdpt_addr = &crate::pdpt_address as *const _ as usize as *mut u64;
                let mut pdt_addr  = &crate::pdt_address as *const _ as usize as *mut u64;

                if pml4t_entry > 0 {
                    pml4t_addr.add(pml4t_entry).write_volatile((k_pdpt_addr as u64) | 0x3);
                    pdpt_addr = k_pdpt_addr;
                }
                if pdpt_addr == k_pdpt_addr || pdpt_entry > 0 {
                    pdpt_addr.add(pdpt_entry).write_volatile((k_pdt_addr as u64) | 0x3);
                    pdt_addr = k_pdt_addr;
                }
                pdt_addr.add(pdt_entry).write_volatile((k_pt_addr as u64) | 0x3);

                are_tables_initialized = true;
            }

            let pt_entry = ((phdr_vaddr << 43) >> 55) as usize;
            let addr_offset = (phdr_vaddr << 52) >> 52;
            for i in 0..((phdr_memsz + addr_offset + (0x1000-1)) / 0x1000) as usize {
                if *k_pt_addr.add(pt_entry+i) == 0 {
                    let mut flags = 0x8000000000000001; // present and no execute
                    // executable
                    if phdr_flags & 0x1 != 0 {
                        flags ^= 0x8000000000000000;
                    }
                    // writable
                    if phdr_flags & 0x2 != 0 {
                        flags |= 0x2;
                    }
                    k_pt_addr.add(pt_entry+i).write_volatile(
                        self.kernel_addr as u64 + (0x1000*(phdr_offset/0x1000)) + (0x1000*i as u64) | flags
                    );
                }
            }
        }
    }

    pub unsafe fn get_bss(&self) -> (u64, u64) {
        let e_shoff = *((self.kernel_addr as *const u64).add(5));
        let e_shentsize = *((self.kernel_addr as *const u16).add(29));
        let e_shnum = *((self.kernel_addr as *const u16).add(30));

        let first_sheader = (self.kernel_addr as *const u8).add(e_shoff as usize);
        let mut sh_addr: u64 = 0;
        let mut sh_size: u64 = 0;

        for i in 0..e_shnum {
            let sheader = first_sheader.add((e_shentsize * i) as usize) as *const u32;
            // continue until its of type SHT_NOBITS
            if *(sheader.add(1)) != 8 { continue; }

            sh_addr = *(sheader.add(4) as *const u64);
            sh_size = *(sheader.add(8) as *const u64);
        }

        (sh_addr, sh_size)
    }

    pub unsafe fn get_entry_address(&self) -> u64 {
        *((self.kernel_addr as *const u64).add(3))
    }
}
