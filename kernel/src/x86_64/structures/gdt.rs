use core::mem;

use crate::{x86_64, utils::lazy_static::LazyStatic};


static GDT_DESCRIPTOR: LazyStatic<GdtDescriptor> = LazyStatic::new();
static GDT: LazyStatic<Gdt> = LazyStatic::new();


pub fn init() {
    use EntryAccess as Access;
    use EntryFlags as Flags;

    // code segment
    let code_entry = Entry::new(
        Access::RW | Access::EXECUTABLE | Access::CODE_OR_DATA | Access::PRESENT,
        Flags::LONG_MODE | Flags::GRANULARITY
    );
    // data segment
    let data_entry = Entry::new(
        Access::RW | Access::CODE_OR_DATA | Access::PRESENT,
        Flags::SIZE | Flags::GRANULARITY
    );

    // init GDT and GDT_DESCRIPTOR
    GDT.init(Gdt::new(code_entry, data_entry));
    GDT_DESCRIPTOR.init(GdtDescriptor::new(&GDT));
}

pub fn load() {
    assert!(GDT_DESCRIPTOR.is_init(), "Attempted to load GDT before initializing it");
    GDT_DESCRIPTOR.load();
}


#[repr(C, packed)]
pub struct GdtDescriptor {
    limit: u16,
    address: &'static Gdt
}
impl GdtDescriptor {
    fn new(gdt: &'static Gdt) -> GdtDescriptor {
        let limit = (mem::size_of::<Gdt>()-1) as u16;
        GdtDescriptor { limit, address: gdt }
    }

    fn load(&'static self) {
        x86_64::cpu::instructions::lgdt(self as *const _ as u64);
    }
}

#[repr(C, packed)]
struct Gdt {
    null: u64,
    // code, data
    code_entry: Entry,
    data_entry: Entry,
}
impl Gdt {
    fn new(code_entry: Entry, data_entry: Entry) -> Gdt {
        Gdt { null: 0, code_entry, data_entry }
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct Entry {
    limit: u16,
    base1: u16,
    base2: u8,
    access: u8,
    flagslimit: u8,
    base3: u8
}
impl Entry {
    fn new(access: u8, flags: u8) -> Entry {
        let flagslimit = flags | 0xF;
        Entry { limit: 0xFFFF, base1: 0, base2: 0, access, flagslimit, base3: 0 }
    }
}
struct EntryAccess;
impl EntryAccess {
    const RW: u8 = 0x2;
    const EXECUTABLE: u8 = 0x8;
    const CODE_OR_DATA: u8 = 0x10;
    const PRESENT: u8 = 0x80;
}
struct EntryFlags;
impl EntryFlags {
    const LONG_MODE: u8 = 0x20;
    const SIZE: u8 = 0x40;
    const GRANULARITY: u8 = 0x80;
}

// FIXME: Implement TSS
// #[repr(C, packed)]
// struct TssEntry {
//     lower_half: Entry,
//     null: u32,
//     base4: u32,
// }
// impl TssEntry {
//     const TSS_ENTRY_ACCESS_TYPE: u8 = 0x9;

//     fn new(tss: &'static Tss) -> TssEntry {
//         let tss_addr = tss as *const _ as usize;

//         let limit = (mem::size_of::<Tss>()-1) as u16;
//         let base3 = (tss_addr >> 24) as u8;
//         let base2 = (tss_addr >> 16) as u8;
//         let base1 = tss_addr as u16;
//         let access = TssEntry::TSS_ENTRY_ACCESS_TYPE | EntryAccess::PRESENT;
//         let flagslimit = 0;
//         let entry = Entry { limit, base1, base2, access, flagslimit, base3 };

//         let base4 = (tss_addr >> 32) as u32;
//         TssEntry { lower_half: entry, null: 0, base4 }
//     }
// }
