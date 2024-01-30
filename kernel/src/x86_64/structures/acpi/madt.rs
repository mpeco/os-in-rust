use core::mem;

use crate::memory::address::{PhysAddr, VirtAddr};
use super::SDTHeader;


#[repr(C, packed)]
pub struct MADT {
    header: SDTHeader,
    lapic_addr: u32,
    flags: u32
}
impl MADT {
    // Returns MMIO address of the LAPIC's registers
    pub fn get_lapic_addr(&self) -> PhysAddr {
        PhysAddr::new(self.lapic_addr as usize)
    }

    // Returns MMIO address of IO APIC with interrupt base 0
    pub fn get_io_apic_addr_base_0(&self) -> Result<PhysAddr, &'static str> {
        for entry in self.iter()
            .filter(|h| h.entry_type == EntryType::IO_APIC_ENTRY)
            .map(|h| h.to_entry::<IOApicEntry>())
        {
            if entry.global_system_interrupt_base == 0 {
                return Ok(PhysAddr::new(entry.io_apic_addr as usize));
            }
        }
        Err("IO APIC not found in MADT")
    }

    // Returns interrupt source override for the given interrupt source
    pub fn get_interrupt_source_override(&self, irq_source: u8) -> Option<&'static IOInterruptSourceOverride> {
        for entry in self.iter()
            .filter(|h| h.entry_type == EntryType::IO_INTERRUPT_SOURCE_OVERRIDE)
            .map(|h| h.to_entry::<IOInterruptSourceOverride>())
        {
            if entry.irq_source == irq_source {
                return Some(entry);
            }
        }
        None
    }

    // Returns an iterator to Processor Local APIC entries
    pub fn processor_lapic_iter(&self) -> impl Iterator<Item = &'static dyn LocalApicEntry> {
        self.iter()
            .filter(|h| h.entry_type == EntryType::PROCESSOR_LAPIC_ENTRY || h.entry_type == EntryType::PROCESSOR_X2LAPIC_ENTRY)
            .map(|h| {
                if h.entry_type == EntryType::PROCESSOR_LAPIC_ENTRY { h.to_entry::<LapicEntry>() as &dyn LocalApicEntry }
                else { h.to_entry::<X2LapicEntry>() as &dyn LocalApicEntry }
            })
    }

    pub fn iter(&self) -> MADTIterator {
        MADTIterator::new(self)
    }
}
pub struct MADTIterator {
    start_addr: VirtAddr,
    end_addr: VirtAddr,
    offset: usize
}
impl MADTIterator {
    fn new(madt: &MADT) -> MADTIterator {
        let start_addr = VirtAddr::new(madt as *const _ as usize + mem::size_of::<MADT>());
        let end_addr = start_addr + madt.header.length as usize - mem::size_of::<MADT>();
        MADTIterator { start_addr, end_addr, offset: 0 }
    }
}
impl Iterator for MADTIterator {
    type Item = &'static EntryHeader;
    fn next(&mut self) -> Option<Self::Item> {
        if self.start_addr + self.offset < self.end_addr {
            let header = unsafe {
                &*self.start_addr.offset::<u8>(self.offset).as_ptr::<EntryHeader>()
            };
            self.offset += header.length as usize;
            return Some(header);
        }
        None
    }
}


#[repr(C, packed)]
pub struct EntryHeader {
    entry_type: u8,
    length: u8,
}
impl EntryHeader {
    fn to_entry<T>(&self) -> &'static T {
        unsafe { core::mem::transmute(self) }
    }
}
struct EntryType();
impl EntryType {
    const PROCESSOR_LAPIC_ENTRY: u8 = 0;
    const IO_APIC_ENTRY: u8 = 1;
    const IO_INTERRUPT_SOURCE_OVERRIDE: u8 = 2;
    const PROCESSOR_X2LAPIC_ENTRY: u8 = 9;
}

pub trait LocalApicEntry {
    fn get_id(&self) -> u32;
    fn get_acpi_id(&self) -> u32;
    fn get_flags(&self) -> u32;
}
#[repr(C, packed)]
struct LapicEntry {
    header: EntryHeader,
    acpi_id: u8,
    id: u8,
    flags: u32,
}
impl LocalApicEntry for LapicEntry {
    fn get_id(&self) -> u32 {
        self.id as u32
    }
    fn get_acpi_id(&self) -> u32 {
        self.acpi_id as u32
    }
    fn get_flags(&self) -> u32 {
        self.flags
    }
}
#[repr(C, packed)]
struct X2LapicEntry {
    header: EntryHeader,
    reserved: u16,
    id: u32,
    flags: u32,
    acpi_id: u32
}
impl LocalApicEntry for X2LapicEntry {
    fn get_id(&self) -> u32 {
        self.id
    }
    fn get_acpi_id(&self) -> u32 {
        self.acpi_id
    }
    fn get_flags(&self) -> u32 {
        self.flags
    }
}

#[repr(C, packed)]
struct IOApicEntry {
    header: EntryHeader,
    id: u8,
    reserved: u8,
    io_apic_addr: u32,
    global_system_interrupt_base: u32
}

#[repr(C, packed)]
pub struct IOInterruptSourceOverride {
    header: EntryHeader,
    bus_source: u8,
    irq_source: u8,
    pub global_system_interrupt: u32,
    pub flags: u16
}
