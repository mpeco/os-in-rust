use super::{
    FrameSize, MemoryRegion, FrameAllocator,
    address::{PhysAddr, VirtualAddress, VirtAddr, MutVirtAddr},
};


// Allocates tables for virtual memory region // FIXME: ONLY FOR 4KB FOR NOW
pub fn allocate_tables(frame_allocator: &mut FrameAllocator, memory_region: &MemoryRegion) -> Result<(), &'static str> {
    for frame in memory_region {
        let virt_addr = VirtAddr::new(frame);

        // Check if page is already mapped
        if virt_addr.to_phys() != None {
            return Err("Page in range already mapped");
        }

        let mut table = virt_addr.get_table();
        while table.level != TableLevel::One {
            let phys_frame_addr = if let Some(phys_frame) = frame_allocator.get_next_frame() {
                phys_frame.to_mut_virtual()
            }
            else {
                return Err("Insufficient physical memory for table allocation");
            };

            let entry = virt_addr.get_entry(table.level);
            unsafe {
                table.map_table_at(phys_frame_addr, Flags::PRESENT | Flags::WRITABLE, entry);
            }
            table = Table::new(phys_frame_addr.into(), table.level.get_next_level().unwrap());
        }
    }

    Ok(())
}


#[non_exhaustive]
pub struct Flags;
impl Flags {
    pub const PRESENT: u64 = 1;
    pub const WRITABLE: u64 = 2;
    pub const USER: u64 = 4;
    pub const WRITE_THROUGH: u64 = 8;
    pub const NO_CACHE: u64 = 16;
    pub const ACCESSED: u64 = 32;
    pub const DIRTY: u64 = 64;
    pub const HUGE: u64 = 128;
    pub const GLOBAL: u64 = 256;
    pub const NO_EXECUTE: u64 = 0x8000000000000000;
}

#[derive(Clone, Copy, PartialEq)]
pub enum TableLevel {
    Four,
    Three,
    Two,
    One
}
impl TableLevel {
    pub fn get_next_level(&self) -> Option<TableLevel> {
        match self {
            TableLevel::Four => Some(TableLevel::Three),
            TableLevel::Three => Some(TableLevel::Two),
            TableLevel::Two => Some(TableLevel::One),
            TableLevel::One => None,
        }
    }

    pub fn get_frame_size(&self) -> Option<FrameSize> {
        match self {
            TableLevel::Three => Some(FrameSize::OneGb),
            TableLevel::Two => Some(FrameSize::TwoMb),
            TableLevel::One => Some(FrameSize::FourKb),
            _ => None,
        }
    }
}

pub enum TableEntry {
    Table{ table: Table, flags: u64 },
    Frame{ address: PhysAddr, flags: u64 }
}

pub struct Table {
    pub address: VirtAddr,
    pub level: TableLevel
}
impl Table {
    const ADRESS_BITMASK: u64 = 0xFFFFFFFFFF000;
    const FLAGS_BITMASK: u64  = 0xFFF0000000000FFF;

    pub const fn new(address: VirtAddr, level: TableLevel) -> Table {
        Table { address, level }
    }
    pub fn table4() -> Table {
        use crate::x86_64::cpu::registers;

        Table {
            address: PhysAddr::new(registers::cr3::read() as usize).to_virtual(),
            level: TableLevel::Four
        }
    }

    pub fn get_entry(&self, entry: usize) -> Option<TableEntry> {
        if !self.is_entry_mapped(entry) {
            return None;
        }

        let (address, flags) = self.get_entry_raw(entry);

        match self.level {
            TableLevel::Four => Some(TableEntry::Table {
                table: Table::new(address.to_virtual(), TableLevel::Three), flags
            }),
            TableLevel::One => Some(TableEntry::Frame { address, flags }),
            // level 3 and 2
            _ => if flags & Flags::HUGE == 0 {
                let next_level = self.level.get_next_level().unwrap();
                Some(TableEntry::Table { table: Table::new(address.to_virtual(), next_level), flags })
            }
            // if table has huge page bit on
            else {
                Some(TableEntry::Frame { address, flags })
            }
        }
    }
    fn get_entry_raw(&self, entry: usize) -> (PhysAddr, u64) {
        let entry_value = unsafe { *self.address.as_ptr::<u64>().add(entry) };
        let address = (entry_value & Self::ADRESS_BITMASK) as usize;
        let flags = entry_value & Self::FLAGS_BITMASK;

        (address.into(), flags)
    }

    pub fn set_entry(&mut self, address: PhysAddr, flags: u64, entry: usize) {
        let mut_table = self.address.to_mut();
        unsafe { mut_table.as_ptr::<u64>().add(entry).write_volatile(address.as_usize() as u64 | flags); }
    }

    pub fn remove_entry(&mut self, entry: usize) {
        let mut_table = self.address.to_mut();
        unsafe { mut_table.as_ptr::<u64>().add(entry).write_volatile(0); }
    }

    fn is_entry_mapped(&self, entry: usize) -> bool {
        let entry_value = self.get_entry_raw(entry);
        !(entry_value.0.as_usize() == 0 && entry_value.1 == 0)
    }

    /*
        Allocates table at specified address and maps it to entry
        Caller must ensure the page frame at "address" is aligned, available and accessible
    */
    pub unsafe fn map_table_at(&mut self, address: MutVirtAddr, flags: u64, entry: usize) {
        use core::intrinsics::volatile_set_memory;

        if self.level == TableLevel::One {
            return;
        }

        // clear memory
        volatile_set_memory(address.as_ptr::<u8>(), 0, 0x1000);
        let phys_addr = address.to_phys().unwrap();
        self.set_entry(phys_addr, flags, entry);
    }
}
