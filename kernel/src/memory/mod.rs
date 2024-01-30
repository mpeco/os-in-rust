use address::PhysAddr;
use e820_memory_map::MemoryMap;


pub mod address;
pub mod e820_memory_map;
pub mod paging;
pub mod kalloc;


// Aligns value down to bytes
pub fn is_aligned(value: usize, bytes: usize) -> bool {
    value % bytes == 0
}
pub fn align_down(value: usize, bytes: usize) -> usize {
    let remainder = value % bytes;
    value - remainder
}
pub fn align_up(mut value: usize, bytes: usize) -> usize {
    if !is_aligned(value, bytes) {
        let remainder = value % bytes;
        value += bytes - remainder;
    }
    value
}


#[derive(Clone, Copy)]
pub enum FrameSize {
    FourKb,
    TwoMb, // level 2 table huge page
    OneGb  // level 3 table huge page
}
impl FrameSize {
    pub fn to_bytes(&self) -> usize {
        match self {
            FrameSize::FourKb => 0x1000,
            FrameSize::TwoMb  => 0x200000,
            FrameSize::OneGb  => 0x40000000
        }
    }
}


pub struct MemoryRegion {
    base: usize,
    length: usize
}
impl MemoryRegion {
    pub fn new(base: usize, length: usize) -> MemoryRegion {
        MemoryRegion { base, length }
    }
    pub fn from_e820_entry(entry: &e820_memory_map::MemoryMapEntry) -> MemoryRegion {
        MemoryRegion::new(entry.base as usize, entry.length as usize)
    }

    // Whether given region is within self
    pub fn is_within(&self, base: usize, length: usize) -> bool {
        base >= self.base && base + length <= self.base + self.length
    }

    pub fn iter(&self, frame_size: FrameSize) -> MemoryRegionIterator {
        MemoryRegionIterator::new(self.base, self.length, frame_size, 0)
    }
}
impl IntoIterator for &MemoryRegion {
    type Item = usize;
    type IntoIter = MemoryRegionIterator;
    fn into_iter(self) -> Self::IntoIter {
        self.iter(FrameSize::FourKb)
    }
}
pub struct MemoryRegionIterator {
    base: usize,
    length: usize,
    frame_size: FrameSize,
    index: usize
}
impl MemoryRegionIterator {
    fn new(mut base: usize, mut length: usize, frame_size: FrameSize, index: usize)
        -> MemoryRegionIterator
    {
        // align base down to frame_size
        base = align_down(base, frame_size.to_bytes()).into();
        // align length up to frame_size
        length = align_up(length, frame_size.to_bytes());

        MemoryRegionIterator { base, length, frame_size, index }
    }
}
impl Iterator for MemoryRegionIterator {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        let current_base = self.base + (self.frame_size.to_bytes()*self.index);

        if current_base >= self.base + self.length {
            return None;
        }

        self.index += 1;
        Some(current_base)
    }
}


// Simple allocator that takes frames linearly from RAM memory map entries
pub struct FrameAllocator<'a> {
    memory_map: &'a MemoryMap,
    next_frame_addr: address::PhysAddr,
    frame_size: FrameSize,
    cur_entry: usize
}
impl<'a> FrameAllocator<'a> {
    pub fn new(memory_map: &'a MemoryMap, next_frame_addr: PhysAddr, frame_size: FrameSize) -> FrameAllocator<'a> {
        let mut cur_entry = 0;
        for (i, entry) in memory_map.iter_usable().enumerate() {
            let entry_region = MemoryRegion::from_e820_entry(entry);
            if entry_region.is_within(next_frame_addr.into(), frame_size.to_bytes()) {
                cur_entry = i;
                break;
            }
        }

        FrameAllocator { memory_map, next_frame_addr, frame_size, cur_entry }
    }

    pub fn get_next_frame(&mut self) -> Option<PhysAddr> {
        for (i, entry) in self.memory_map.iter_usable().enumerate().skip(self.cur_entry) {
            if self.next_frame_addr < entry.base as usize {
                self.next_frame_addr = (entry.base as usize).into();
                self.cur_entry = i;
            }

            let entry_region = MemoryRegion::from_e820_entry(entry);
            if entry_region.is_within(self.next_frame_addr.into(), self.frame_size.to_bytes()) {
                let next_frame_addr = self.next_frame_addr;
                self.next_frame_addr = self.next_frame_addr + self.frame_size.to_bytes();
                return Some(next_frame_addr);
            }
        }

        None
    }
}
