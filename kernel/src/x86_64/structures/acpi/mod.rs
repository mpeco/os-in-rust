use core::mem;

use crate::{
    memory::address::{VirtAddr, PhysAddr},
    utils::{init_once::InitOnce, lazy_static::LazyStatic, checksum}
};
use self::madt::MADT;


pub mod madt;


static IS_RSDT_INIT: InitOnce = InitOnce::new();
static RSDP: LazyStatic<&'static RSDP> = LazyStatic::new();
static RSDT: LazyStatic<&'static dyn RootSystemDescriptionTable> = LazyStatic::new();

static MADT: LazyStatic<&'static MADT> = LazyStatic::new();


pub fn init_rsdp_and_rsdt(rsdp_addr: VirtAddr) -> Result<(), &'static str> {
    IS_RSDT_INIT.init().expect("Attempt to initialize RSDP and RSDT more than once");

    RSDP.init(unsafe { &*rsdp_addr.as_ptr::<RSDP>() });
    RSDP.validate()?;

    RSDT.init(RSDP.get_table());
    RSDT.validate()?;

    Ok(())
}

pub fn init_madt() -> Result<(), &'static str> {
    assert!(MADT.is_init() == false, "Attempt to initialize MADT more than once");

    if let Some(addr) = RSDT.find_table("APIC") {
        MADT.init(unsafe { &*addr.as_ptr::<MADT>() });
        Ok(())
    }
    else {
        Err("Could not locate MADT")
    }
}
pub fn get_madt() -> &'static MADT {
    assert!(MADT.is_init(), "Attempt to access MADT before initializing it");
    *MADT
}


#[repr(C, packed)]
struct RSDP1 {
    signature: [u8; 8],
    checksum: u8,
    oemid: [u8; 6],
    revision: u8,
    rsdt_addr: u32
}
#[repr(C, packed)]
struct RSDP {
    first_part: RSDP1,
    length: u32,
    xsdt_addr: usize,
    extended_checksum: u8,
    reserved: [u8; 3]
}
impl RSDP {
    // Checks version and validates checksum
    pub fn validate(&self) -> Result<(), &'static str> {
        // validate first part
        let byte_array = unsafe { &*(self as *const _ as usize as *const [u8; mem::size_of::<RSDP1>()]) };
        let remainder = checksum::eight_bit_modulo(byte_array);

        // if ACPI version 2.0 or higher validate rest
        let mut remainder2: u64 = 0;
        if self.first_part.revision != 0 {
            let addr = (self as *const _ as usize) + mem::size_of::<RSDP1>();
            let byte_array = unsafe { &*(addr as *const [u8; mem::size_of::<RSDP>() - mem::size_of::<RSDP1>()]) };
            remainder2 = checksum::eight_bit_modulo(byte_array);
        }

        if remainder != 0 || remainder2 != 0 {
            return Err("RSDP checksum invalid");
        }

        Ok(())
    }

    pub fn get_table(&self) -> &'static dyn RootSystemDescriptionTable {
        if self.first_part.revision == 0 {
            let rsdt_addr = PhysAddr::new(self.first_part.rsdt_addr as usize).to_virtual();
            return unsafe { &*rsdt_addr.as_ptr::<RSDT>() };
        }
        else {
            let xsdt_addr = PhysAddr::new(self.xsdt_addr).to_virtual();
            return unsafe { &*xsdt_addr.as_ptr::<XSDT>() };
        }
    }
}


#[repr(C, packed)]
struct SDTHeader {
    pub signature: [u8; 4],
    pub length: u32,
    pub revision: u8,
    pub checksum: u8,
    pub oemid: [u8; 6],
    pub oem_table_id: [u8; 8],
    pub oem_revision: u32,
    pub creator_id: u32,
    pub creator_revision: u32
}

trait RootSystemDescriptionTable: Sync {
    fn validate(&self) -> Result<(), &'static str>;
    fn find_table(&self, signature: &str) -> Option<VirtAddr>;
}

#[repr(C, packed)]
struct RSDT {
    header: SDTHeader
}
impl RSDT {
    // returns the iterator with the addresses of the tables this SDT points to
    fn table_addresses(&self) -> impl Iterator<Item = VirtAddr> {
        self.iter().map(|addr| PhysAddr::new(addr as usize).to_virtual())
    }

    fn iter(&self) -> RSDTIterator {
        RSDTIterator::new(self)
    }
}
impl RootSystemDescriptionTable for RSDT {
    fn validate(&self) -> Result<(), &'static str> {
        let byte_array = unsafe { &*(self as *const _ as usize as *const [u8; mem::size_of::<SDTHeader>()]) };
        let mut remainder = checksum::eight_bit_modulo(byte_array);
        for addr in self.iter() {
            let byte_array = unsafe { &*(&addr as *const _ as *const [u8; 4]) };
            remainder += checksum::eight_bit_modulo(byte_array);
        }
        remainder %= (u8::MAX as u64) + 1;

        if remainder != 0 {
            return Err("RSDT checksum invalid");
        }

        Ok(())
    }

    // Signature must have 4 characters
    fn find_table(&self, signature: &str) -> Option<VirtAddr> {
        let expected_ba = signature.as_bytes();

        for addr in self.table_addresses() {
            let byte_array = unsafe { *addr.as_ptr::<[u8; 4]>() };
            if expected_ba == byte_array {
                return Some(addr);
            }
        }

        None
    }
}
struct RSDTIterator {
    start_addr: VirtAddr,
    length: usize,
    index: usize
}
impl RSDTIterator {
    fn new(rsdt: &RSDT) -> RSDTIterator {
        let start_addr = VirtAddr::new((rsdt as *const _ as usize) + mem::size_of::<SDTHeader>());
        let length = (rsdt.header.length as usize - mem::size_of::<SDTHeader>()) / mem::size_of::<u32>();
        RSDTIterator { start_addr, length, index: 0 }
    }
}
impl Iterator for RSDTIterator {
    type Item = u32;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.length as usize {
            return None;
        }
        let cur_addr = self.start_addr.offset::<u32>(self.index);
        self.index += 1;
        Some(unsafe{ *cur_addr.as_ptr::<u32>() })
    }
}

#[repr(C, packed)]
struct XSDT {
    header: SDTHeader
}
impl XSDT {
    // returns the iterator with the addresses of the tables this SDT points to
    fn table_addresses(&self) -> impl Iterator<Item = VirtAddr> {
        self.iter().map(|addr| addr.to_virtual())
    }

    fn iter(&self) -> XSDTIterator {
        XSDTIterator::new(self)
    }
}
impl RootSystemDescriptionTable for XSDT {
    fn validate(&self) -> Result<(), &'static str> {
        // validate first part
        let byte_array = unsafe { &*(self as *const _ as usize as *const [u8; mem::size_of::<SDTHeader>()]) };
        let mut remainder = checksum::eight_bit_modulo(byte_array);
        for addr in self.iter() {
            let addr = addr.as_usize();
            let byte_array = unsafe { &*(&addr as *const _ as *const [u8; 8]) };
            remainder += checksum::eight_bit_modulo(byte_array);
        }
        remainder %= (u8::MAX as u64) + 1;

        if remainder != 0 {
            return Err("XSDT checksum invalid");
        }

        Ok(())
    }

    // Signature must have 4 characters
    fn find_table(&self, signature: &str) -> Option<VirtAddr> {
        let expected_ba = signature.as_bytes();

        for addr in self.table_addresses() {
            let byte_array = unsafe { *addr.as_ptr::<[u8; 4]>() };
            if expected_ba == byte_array {
                return Some(addr);
            }
        }

        None
    }
}
struct XSDTIterator {
    start_addr: VirtAddr,
    length: usize,
    index: usize
}
impl XSDTIterator {
    fn new(xsdt: &XSDT) -> XSDTIterator {
        let start_addr = VirtAddr::new((xsdt as *const _ as usize) + mem::size_of::<SDTHeader>());
        let length = (xsdt.header.length as usize - mem::size_of::<SDTHeader>()) / mem::size_of::<PhysAddr>();
        XSDTIterator { start_addr, length, index: 0 }
    }
}
impl Iterator for XSDTIterator {
    type Item = PhysAddr;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.length as usize {
            return None;
        }
        let cur_addr = self.start_addr.offset::<PhysAddr>(self.index);
        self.index += 1;
        Some(unsafe{ *cur_addr.as_ptr::<PhysAddr>() })
    }
}
