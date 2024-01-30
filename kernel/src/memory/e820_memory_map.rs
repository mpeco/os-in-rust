use core::mem;

use super::address::PhysAddr;


// Creates reserved entry for kernel map, sorts entries and align RAM entries to 4KB
pub fn init(memory_map: &mut MemoryMap, kernel_base: usize, kernel_len: usize) -> Result<(), &'static str> {
    use crate::memory;
    use super::{FrameSize, MemoryRegion};

    // get memory map entry that contains kernel elf
    let mut kernel_entry_index = memory_map.size as usize;
    for (i, entry) in memory_map.iter().enumerate() {
        let entry_region = MemoryRegion::from_e820_entry(entry);
        if entry_region.is_within(kernel_base, kernel_len) {
            kernel_entry_index = i;
            break;
        }
    }
    if kernel_entry_index == memory_map.size as usize {
        return Err("Error with E820 Memory Map, perhaps lack of memory?");
    }

    // split entry into up to 3 parts so kernel elf has entry of type reserved
    // entry before start of kernel (if exists)
    let entry = &memory_map.entries[kernel_entry_index];
    if (entry.base as usize) < kernel_base {
        let prev_entry = MemoryMapEntry::new(
            (entry.base as usize).into(), kernel_base as u64 - entry.base,
            MemoryMapRegionType::Ram
        );
        memory_map.add_entry(prev_entry, kernel_entry_index);
        kernel_entry_index += 1;
    }
    // entry after kernel (if exists)
    let entry = &memory_map.entries[kernel_entry_index];
    if (entry.base+entry.length) as usize > kernel_base+kernel_len {
        let next_entry = MemoryMapEntry::new(
            (kernel_base+kernel_len).into(), entry.base+entry.length-(kernel_base+kernel_len) as u64,
            MemoryMapRegionType::Ram
        );
        memory_map.add_entry(next_entry, kernel_entry_index+1);
    }
    // kernel entry
    let entry = &mut memory_map.entries[kernel_entry_index];
    entry.base = kernel_base as u64;
    entry.length = kernel_len as u64;
    entry.region_type = MemoryMapRegionType::Reserved as u32;

    // sort memory map
    memory_map.sort();

    // align usable memory regions from memory map to 4KB
    for entry in memory_map.iter_mut_usable()
    {
        entry.base = memory::align_up(entry.base as usize, FrameSize::FourKb.to_bytes()) as u64;
        entry.length = memory::align_down(entry.length as usize, FrameSize::FourKb.to_bytes()) as u64;
    }

    Ok(())
}


#[repr(C, packed)]
pub struct MemoryMap {
    size: u32,
    entries: [MemoryMapEntry; 0xFF0/(mem::size_of::<u64>()*3)]
}
impl MemoryMap {
    pub fn add_entry(&mut self, entry: MemoryMapEntry, index: usize) {
        let mut prev_entry = entry;
        for entry in self.iter_mut().skip(index) {
            let temp = *entry;
            *entry = prev_entry;
            prev_entry = temp;
        }
        self.entries[self.size as usize] = prev_entry;
        self.size += 1;
    }

    // Sorts entries in ascending order of base address
    pub fn sort(&mut self) {
        self.entries[0..self.size as usize].sort_unstable();
    }

    pub fn iter(&self) -> MemoryMapIterator {
        MemoryMapIterator { memory_map: self, index: 0 }
    }
    pub fn iter_usable(&self) -> impl Iterator<Item = &MemoryMapEntry> {
        let iter = MemoryMapIterator { memory_map: self, index: 0 };
        iter.filter(|e| (*e).region_type == MemoryMapRegionType::Ram as u32)
    }
    pub fn iter_mut(&mut self) -> MemoryMapMutIterator {
        MemoryMapMutIterator { memory_map: self, index: 0 }
    }
    pub fn iter_mut_usable(&mut self) -> impl Iterator<Item = &mut MemoryMapEntry>{
        let iter = MemoryMapMutIterator { memory_map: self, index: 0 };
        iter.filter(|e| (*e).region_type == MemoryMapRegionType::Ram as u32)
    }
}
impl<'a> IntoIterator for &'a MemoryMap {
    type Item = &'a MemoryMapEntry;
    type IntoIter = MemoryMapIterator<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'a> IntoIterator for &'a mut MemoryMap {
    type Item = &'a mut MemoryMapEntry;
    type IntoIter = MemoryMapMutIterator<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
pub struct MemoryMapIterator<'a> {
    memory_map: &'a MemoryMap,
    index: usize,
}
impl<'a> Iterator for MemoryMapIterator<'a> {
    type Item = &'a MemoryMapEntry;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.memory_map.size as usize {
            return None;
        }

        let index = self.index;
        self.index += 1;
        Some(&self.memory_map.entries[index])
    }
}
pub struct MemoryMapMutIterator<'a> {
    memory_map: &'a mut MemoryMap,
    index: usize,
}
impl<'a> Iterator for MemoryMapMutIterator<'a> {
    type Item = &'a mut MemoryMapEntry;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.memory_map.size as usize {
            return None;
        }

        let index = self.index;
        self.index += 1;
        unsafe {
            let ptr = self.memory_map.entries.as_mut_ptr().add(index);
            Some(&mut *ptr)
        }
    }
}

pub enum MemoryMapRegionType {
    Ram = 1,
    Reserved,
    Acpi,
    AcpiNvs,
    Unusable
}
#[repr(C, packed)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MemoryMapEntry {
    pub base: u64,
    pub length: u64,
    pub region_type: u32,
    pub extended_attributes: u32,
}
impl MemoryMapEntry {
    pub fn new(base: PhysAddr, length: u64, region_type: MemoryMapRegionType) -> MemoryMapEntry {
        MemoryMapEntry { base: base.as_usize() as u64, length, region_type: region_type as u32, extended_attributes: 1 }
    }
}
