use super::{
    FrameAllocator, MemoryRegion, FrameSize,
    address::{VirtAddr, VirtualAddress, PhysAddr},
    paging::{self, Flags}
};


pub const HEAP_BASE: usize = 0x1100_00000000;
pub const HEAP_LENGTH: usize = 0xA00000; // 10 MBs


pub fn init_heap(frame_allocator: &mut FrameAllocator) -> Result<(), &'static str> {
    // allocate tables for heap
    let memory_region = MemoryRegion::new(HEAP_BASE, HEAP_LENGTH);
    paging::allocate_tables(frame_allocator, &memory_region)?;

    // allocate and map physical frames for heap
    for twomb_frame in memory_region.iter(FrameSize::TwoMb) {
        let mut table = VirtAddr::new(twomb_frame).get_table();

        let inner_region_length = if twomb_frame+FrameSize::TwoMb.to_bytes() > HEAP_BASE+HEAP_LENGTH {
            HEAP_BASE+HEAP_LENGTH - twomb_frame
        }
        else {
            FrameSize::TwoMb.to_bytes()
        };
        let inner_memory_region = MemoryRegion::new(twomb_frame, inner_region_length);
        for fourkb_frame in &inner_memory_region {
            let virt_addr = PhysAddr::new(fourkb_frame).to_virtual();
            let phys_frame_addr = if let Some(phys_frame) = frame_allocator.get_next_frame() {
                phys_frame
            }
            else {
                return Err("Insufficient physical memory for heap");
            };
            table.set_entry(phys_frame_addr, Flags::PRESENT | Flags::WRITABLE, virt_addr.get_entry(table.level))
        }
    }

    // initialize the allocator
    unsafe { ALLOCATOR.lock().init(HEAP_BASE.into(), HEAP_LENGTH); }

    Ok(())
}


use crate::locks::spinlock::Spinlock;
use self::fixed_size_block_alloc::FixedSizeBlockAllocator;

#[global_allocator]
static ALLOCATOR: Spinlock<FixedSizeBlockAllocator> = Spinlock::new(FixedSizeBlockAllocator::new());


pub mod fixed_size_block_alloc {
    use core::mem;

    use alloc::alloc::{GlobalAlloc, Layout};
    use crate::{locks::spinlock::Spinlock, memory::address::VirtAddr};

    const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];

    struct BlockNode {
        next: Option<&'static mut BlockNode>
    }

    pub struct FixedSizeBlockAllocator {
        heads: [Option<&'static mut BlockNode>; BLOCK_SIZES.len()],
        fallback: LinkedListAllocator
    }
    impl FixedSizeBlockAllocator {
        pub const fn new() -> FixedSizeBlockAllocator {
            const EMPTY: Option<&'static mut BlockNode> = None;
            FixedSizeBlockAllocator { heads: [EMPTY; BLOCK_SIZES.len()], fallback: LinkedListAllocator::new() }
        }

        pub unsafe fn init(&mut self, heap_base: VirtAddr, heap_length: usize) {
            self.fallback.init(heap_base, heap_length);
        }

        fn get_index(layout: Layout) -> Option<usize> {
            let required_block_size = layout.size().max(layout.align());
            BLOCK_SIZES.iter().position(|&s| s >= required_block_size)
        }

        // if possible scraps blocks to make space for the layout
        fn scrap_free_blocks(&mut self, layout: Layout) -> Result<(), ()>{
            if let Some(index) = FixedSizeBlockAllocator::get_index(layout) {
                for i in index+1..BLOCK_SIZES.len() {
                    if let Some(node) = self.heads[i].take() {
                        self.heads[i] = node.next.take();
                        unsafe {
                            self.fallback.add_free_region(
                                (node as *mut BlockNode as usize).into(), BLOCK_SIZES[i]
                            );
                        }
                        return Ok(());
                    }
                }
            }

            Err(())
        }

        unsafe fn add_block_node(&mut self, node_ptr: *mut BlockNode, head_index: usize) {
            let new_node = BlockNode { next: self.heads[head_index].take() };
            node_ptr.write_volatile(new_node);
            self.heads[head_index] = Some(&mut *node_ptr);
        }
    }
    unsafe impl GlobalAlloc for Spinlock<FixedSizeBlockAllocator> {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let ret: *mut u8;
            let mut allocator = self.lock();

            if let Some(index) = FixedSizeBlockAllocator::get_index(layout) {
                if let Some(node) = allocator.heads[index].take() {
                    allocator.heads[index] = node.next.take();
                    ret = node as *mut BlockNode as *mut u8;
                }
                else {
                    // allocate a block for this size with fallback
                    let block_size = BLOCK_SIZES[index];
                    // align will be updated by fallback allocator
                    let layout = Layout::from_size_align(block_size, 1).unwrap();
                    ret = allocator.fallback.alloc(layout);

                    // since the smallest region the fallback can allocate is 16 bytes separate 8 byte blocks in 2
                    assert!(mem::size_of::<ListNode>() == 16 && mem::size_of::<BlockNode>() == 8);
                    if ret != ptr::null_mut() && BLOCK_SIZES[index] == 8 {
                        allocator.add_block_node((ret as *mut BlockNode).add(1), index);
                    }
                }
            }
            else {
                ret = allocator.fallback.alloc(layout)
            }

            // if alloc failed
            if ret == ptr::null_mut() {
                // try to scrap free blocks and alloc again
                if let Ok(_) = allocator.scrap_free_blocks(layout) {
                    return allocator.fallback.alloc(layout);
                }
            }

            ret
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            let mut allocator = self.lock();

            if let Some(index) = FixedSizeBlockAllocator::get_index(layout) {
                // should always have size and alignment for a node
                assert!(mem::size_of::<BlockNode>() <= BLOCK_SIZES[index]);
                assert!(mem::align_of::<BlockNode>() <= BLOCK_SIZES[index]);

                allocator.add_block_node(ptr as *mut BlockNode, index);
            }
            else {
                allocator.fallback.dealloc(ptr, layout);
            }
        }
    }


    use core::ptr;
    use crate::memory::{self, address::MutVirtAddr};

    struct ListNode {
        length: usize,
        next: Option<&'static mut ListNode>
    }
    impl ListNode {
        const fn new(length: usize) -> Self {
            ListNode { length, next: None }
        }

        fn start_addr(&self) -> VirtAddr {
            (self as *const Self as usize).into()
        }

        fn end_addr(&self) -> VirtAddr {
            self.start_addr() + self.length
        }
    }

    // FIXME: merge free regions next to each other
    pub struct LinkedListAllocator {
        head: ListNode
    }
    impl LinkedListAllocator {
        pub const fn new() -> LinkedListAllocator {
            LinkedListAllocator { head: ListNode::new(0) }
        }

        pub unsafe fn init(&mut self, heap_base: VirtAddr, heap_length: usize) {
            self.add_free_region(heap_base.into(), heap_length);
        }

        fn adjust_layout(layout: Layout) -> Layout {
            let layout = layout.align_to(mem::align_of::<ListNode>())
                .expect("Failed to adjust alloc layout").pad_to_align();
            let size = layout.size().max(mem::size_of::<ListNode>());
            Layout::from_size_align(size, layout.align()).expect("Failed to adjust alloc layout")
        }

        unsafe fn add_free_region(&mut self, address: MutVirtAddr, length: usize) {
            // should always be aligned and able to hold a Node
            assert!(memory::is_aligned(address.as_usize(), mem::align_of::<ListNode>()));
            assert!(length >= mem::size_of::<ListNode>());

            let mut new_node = ListNode::new(length);
            new_node.next = self.head.next.take();
            let node_ptr = address.as_ptr::<ListNode>();
            node_ptr.write_volatile(new_node);
            self.head.next = Some(&mut *node_ptr);
        }

        fn find_region(&mut self, length: usize, align: usize) -> Option<(&'static mut ListNode, MutVirtAddr)>
        {
            let mut current = &mut self.head;

            // look for a large enough memory region in linked list
            while let Some(ref mut region) = current.next {
                if let Ok(alloc_start_addr) = LinkedListAllocator::alloc_from_region(&region, length, align) {
                    // region suitable for allocation, remove node from list and return it
                    let next = region.next.take();
                    let ret = Some((current.next.take().unwrap(), alloc_start_addr));
                    current.next = next;
                    return ret;
                } else {
                    // region not suitable, continue with next region
                    current = current.next.as_mut().unwrap();
                }
            }

            // no suitable region found
            None
        }

        fn alloc_from_region(region: &ListNode, length: usize, align: usize) -> Result<MutVirtAddr, ()>
        {
            let alloc_start_addr: MutVirtAddr = memory::align_up(region.start_addr().as_usize(), align).into();
            let alloc_end_addr: VirtAddr = alloc_start_addr.as_usize().checked_add(length).expect("Overflow").into();

            // if region is too small
            if alloc_end_addr > region.end_addr() {
                return Err(());
            }

            // if rest of region is too small to hold Node
            if region.end_addr() != alloc_end_addr
                && region.end_addr() < alloc_end_addr + mem::size_of::<ListNode>()
            {
                return Err(());
            }

            // region suitable for allocation
            Ok(alloc_start_addr)
        }

        pub unsafe fn alloc(&mut self, layout: Layout) -> *mut u8 {
            let layout = LinkedListAllocator::adjust_layout(layout);

            let size = layout.size();
            let align = layout.align();
            if let Some((region, alloc_start_addr)) = self.find_region(size, align) {
                let alloc_end_addr: MutVirtAddr = alloc_start_addr.as_usize().checked_add(size).unwrap().into();
                let excess_size = region.end_addr().as_usize() - alloc_end_addr;
                if excess_size > 0 {
                    self.add_free_region(alloc_end_addr, excess_size);
                }
                alloc_start_addr.as_ptr::<u8>()
            } else {
                ptr::null_mut()
            }
        }

        pub unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
            let layout = LinkedListAllocator::adjust_layout(layout);
            self.add_free_region(MutVirtAddr::new(ptr as usize), layout.size());
        }
    }
}
