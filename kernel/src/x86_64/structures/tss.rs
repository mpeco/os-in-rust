use crate::memory::address::VirtAddr;


#[repr(C, packed)]
pub struct Tss {
    reserved0: u32,
    pst: [usize; 3],
    reserved1: u64,
    ist: [usize; 7],
    reserved2: u64,
    reserved3: u16,
    io_map_base_addr: u16,
}
impl Tss {
    pub const fn new() -> Tss {
        Tss{ reserved0: 0, pst: [0; 3], reserved1: 0, ist: [0; 7], reserved2: 0, reserved3: 0, io_map_base_addr: 0 }
    }

    pub fn set_ist_entry(&mut self, index: usize, stack_end_addr: VirtAddr) {
        assert!(index < 7);
        self.ist[index] = stack_end_addr.as_usize();
    }
}
