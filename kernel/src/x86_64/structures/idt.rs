use alloc::boxed::Box;


const IDT_MAX_NUM_OF_ENTRIES: usize = 256;


pub struct Idt {
    descriptor: Descriptor,
    table: Table
}
impl Idt {
    pub fn new() -> Idt {
        let table = Table::new();
        Idt { descriptor: Descriptor::new(table.get_address()), table }
    }

    pub fn load(&self) {
        self.descriptor.load();
    }

    pub fn set_entry(&mut self, index: u8, fn_ptr: usize, selector: u16, flags: u8, ist_index: u8) {
        self.table.set_entry(index, fn_ptr, selector, flags, ist_index);
    }
    pub fn clear_entry(&mut self, index: u8) {
        self.table.clear_entry(index);
    }
}

#[repr(C, packed)]
struct Descriptor {
    limit: u16,
    table_address: u64,
}
impl Descriptor {
    pub fn new(table_address: u64) -> Descriptor {
        use core::mem;
        let limit = ((IDT_MAX_NUM_OF_ENTRIES * mem::size_of::<Entry>()) - 1) as u16;
        Descriptor { limit, table_address }
    }

    pub fn load(&self) {
        crate::x86_64::cpu::instructions::lidt(self as *const _ as u64);
    }
}

#[repr(C, packed)]
struct Table {
    table: Box<[Entry; IDT_MAX_NUM_OF_ENTRIES]>
}
impl Table {
    const ZERO_ENTRY: Entry = Entry::new(0, 0, 0, 0);

    pub fn new() -> Table {
        Table { table: Box::new([Self::ZERO_ENTRY; IDT_MAX_NUM_OF_ENTRIES]) }
    }

    pub fn get_address(&self) -> u64 {
        self.table.as_ptr() as u64
    }

    pub fn set_entry(&mut self, index: u8, fn_ptr: usize, selector: u16, flags: u8, ist_index: u8) {
        assert!((index as usize) < IDT_MAX_NUM_OF_ENTRIES);
        let entry = Entry::new(fn_ptr, selector, flags, ist_index);
        self.table[index as usize] = entry;
    }

    pub fn clear_entry(&mut self, index: u8) {
        assert!((index as usize) < IDT_MAX_NUM_OF_ENTRIES);
        self.table[index as usize] = Self::ZERO_ENTRY;
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct Entry {
    fn_ptr_low: u16,
    gdt_selector: u16,
    ist_index: u8,
    flags: u8,
    fn_ptr_middle: u16,
    fn_ptr_high: u32,
    zero: u32,
}
impl Entry {
    const fn new(fn_ptr: usize, selector: u16, flags: u8, ist_index: u8) -> Entry {
        Entry {
            fn_ptr_low: fn_ptr as u16,
            gdt_selector: selector,
            ist_index,
            flags,
            fn_ptr_middle: (fn_ptr >> 16) as u16,
            fn_ptr_high: (fn_ptr >> 32) as u32,
            zero: 0
        }
    }
}

pub struct Index {}
impl Index {
    pub const DIVISION_BY_ZERO: u8 = 0;
    pub const DEBUG: u8 = 1;
    pub const NMI: u8 = 2;
    pub const BREAKPOINT: u8 = 3;
    pub const DOUBLE_FAULT: u8 = 8;
    pub const GENERAL_PROTECTION_FAULT: u8 = 13;
    pub const PAGE_FAULT: u8 = 14;
    pub const KEYBOARD: u8 = 0xE9;
    pub const SYS_TIMER: u8 = 0xF6;
    pub const LAPIC_TIMER: u8 = 0xF7;
    pub const HALT: u8 = 0xFE;
    pub const SPURIOUS: u8 = 0xFF;
}

pub struct Flags {}
impl Flags {
    pub const BASE: u8 = 0x8E;
    pub const TRAP_GATE: u8 = 0x1;
    pub const PRESENT: u8 = 0x80;
}
