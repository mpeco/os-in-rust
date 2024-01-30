use core::{
    ops::{Add, Sub, Rem},
    mem, fmt::Debug
};

use super::paging::{Table, TableEntry, TableLevel};


// virtual memory offset where physical memory is stored
pub const PHYS_MEM_VIRT_ADDR: VirtAddr = VirtAddr::new(0x100_00000000);


#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysAddr {
    address: usize
}
impl PhysAddr {
    pub const fn new(address: usize) -> PhysAddr {
        PhysAddr { address }
    }

    pub const fn as_usize(&self) -> usize {
        self.address
    }

    pub const fn offset<T>(&self, count: usize) -> PhysAddr
        where T: Sized
    {
        PhysAddr::new(self.as_usize() + count*mem::size_of::<T>())
    }

    pub const fn to_virtual(&self) -> VirtAddr {
        PHYS_MEM_VIRT_ADDR.offset::<u8>(self.as_usize())
    }
    pub const fn to_mut_virtual(&self) -> MutVirtAddr {
        PHYS_MEM_VIRT_ADDR.to_mut().offset::<u8>(self.as_usize())
    }
}
impl Debug for PhysAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "PhysAddr({:#x})", self.address)
    }
}


pub trait VirtualAddress {
    fn to_usize(&self) -> usize;

    fn get_entry(&self, level: TableLevel) -> usize {
        match level {
            TableLevel::Four => (self.to_usize() << 16) >> 55,
            TableLevel::Three => (self.to_usize() << 25) >> 55,
            TableLevel::Two => (self.to_usize() << 34) >> 55,
            TableLevel::One => (self.to_usize() << 43) >> 55,
        }
    }
    fn get_offset(&self, level: TableLevel) -> usize {
        if level == TableLevel::Three {
            return (self.to_usize() << 34) >> 34;
        }
        else if level == TableLevel::Two {
            return (self.to_usize() << 43) >> 43;
        }

        (self.to_usize() << 52) >> 52
    }

    // Returns deepest table in the address
    fn get_table(&self) -> Table {
        let mut table = Table::table4();
        let mut entry = self.get_entry(TableLevel::Four);
        while let Some(TableEntry::Table { table: next_table, .. }) = table.get_entry(entry) {
            table = next_table;
            entry = self.get_entry(table.level);
            if table.level == TableLevel::One {
                break;
            }
        }

        table
    }

    fn to_phys(&self) -> Option<PhysAddr> {
        let table = self.get_table();
        let entry = self.get_entry(table.level);

        if let Some(TableEntry::Frame { address, .. }) = table.get_entry(entry) {
            return Some(address + self.get_offset(table.level));
        }

        None
    }
    // Caller must make sure the virtual address points to entire physical memory mapping
    unsafe fn to_phys_direct(&self) -> PhysAddr {
        PhysAddr::new(self.to_usize() - PHYS_MEM_VIRT_ADDR)
    }
}


#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtAddr {
    address: usize
}
impl VirtAddr {
    pub const fn new(address: usize) -> VirtAddr {
        VirtAddr { address }
    }

    pub const fn as_usize(&self) -> usize {
        self.address
    }

    pub const fn as_ptr<T>(&self) -> *const T
    {
        self.as_usize() as *const T
    }

    pub const fn to_mut(&self) -> MutVirtAddr {
        MutVirtAddr::new(self.as_usize())
    }

    pub const fn offset<T>(&self, count: usize) -> VirtAddr
    where T: Sized,
    {
        VirtAddr::new(self.as_usize() + count*mem::size_of::<T>())
    }
}
impl VirtualAddress for VirtAddr {
    fn to_usize(&self) -> usize {
        self.as_usize()
    }
}
impl From<MutVirtAddr> for VirtAddr {
    fn from(address: MutVirtAddr) -> Self {
        address.to_unmut()
    }
}
impl Debug for VirtAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "VirtAddr({:#x})", self.address)
    }
}


#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MutVirtAddr {
    address: usize
}
impl MutVirtAddr {
    pub const fn new(address: usize) -> MutVirtAddr {
        MutVirtAddr { address }
    }

    pub const fn as_usize(&self) -> usize {
        self.address
    }

    pub const fn as_ptr<T>(&self) -> *mut T
    {
        self.as_usize() as *mut T
    }

    pub const fn to_unmut(&self) -> VirtAddr {
        VirtAddr::new(self.as_usize())
    }

    pub const fn offset<T>(&self, count: usize) -> MutVirtAddr
    where T: Sized,
    {
        MutVirtAddr::new(self.as_usize() + count*mem::size_of::<T>())
    }
}
impl VirtualAddress for MutVirtAddr {
    fn to_usize(&self) -> usize {
        self.as_usize()
    }
}
impl From<VirtAddr> for MutVirtAddr {
    fn from(address: VirtAddr) -> Self {
        address.to_mut()
    }
}
impl Debug for MutVirtAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "MutVirtAddr({:#x})", self.address)
    }
}


// Impl macros:
macro_rules! impl_add {
    (for $($t:ty),+) => {
        $(impl Add for $t {
            type Output = $t;
            fn add(self, rhs: Self) -> Self::Output {
                <$t>::new(self.address + rhs.address)
            }
        })*
    }
}
impl_add!(for PhysAddr, VirtAddr, MutVirtAddr);
macro_rules! impl_add_addr_usize {
    (for $($t:ty),+) => {
        $(impl Add<$t> for usize {
            type Output = usize;
            fn add(self, rhs: $t) -> Self::Output {
                self + rhs.address
            }
        })*
    }
}
impl_add_addr_usize!(for PhysAddr, VirtAddr, MutVirtAddr);
macro_rules! impl_add_usize_addr {
    (for $($t:ty),+) => {
        $(impl Add<usize> for $t {
            type Output = $t;
            fn add(self, rhs: usize) -> Self::Output {
                <$t>::new(rhs + self)
            }
        })*
    }
}
impl_add_usize_addr!(for PhysAddr, VirtAddr, MutVirtAddr);

macro_rules! impl_sub {
    (for $($t:ty),+) => {
        $(impl Sub for $t {
            type Output = $t;
            fn sub(self, rhs: Self) -> Self::Output {
                <$t>::new(self.address - rhs.address)
            }
        })*
    }
}
impl_sub!(for PhysAddr, VirtAddr, MutVirtAddr);
macro_rules! impl_sub_addr_usize {
    (for $($t:ty),+) => {
        $(impl Sub<$t> for usize {
            type Output = usize;
            fn sub(self, rhs: $t) -> Self::Output {
                self - rhs.address
            }
        })*
    }
}
impl_sub_addr_usize!(for PhysAddr, VirtAddr, MutVirtAddr);
macro_rules! impl_sub_usize_addr {
    (for $($t:ty),+) => {
        $(impl Sub<usize> for $t {
            type Output = $t;
            fn sub(self, rhs: usize) -> Self::Output {
                <$t>::new(self.address - rhs)
            }
        })*
    }
}
impl_sub_usize_addr!(for PhysAddr, VirtAddr, MutVirtAddr);

macro_rules! impl_rem {
    (for $($t:ty),+) => {
        $(impl Rem for $t {
            type Output = usize;
            fn rem(self, rhs: Self) -> Self::Output {
                self.address % rhs.address
            }
        })*
    }
}
impl_rem!(for PhysAddr, VirtAddr, MutVirtAddr);
macro_rules! impl_rem_addr_usize {
    (for $($t:ty),+) => {
        $(impl Rem<usize> for $t {
            type Output = usize;
            fn rem(self, rhs: usize) -> Self::Output {
                self.address % rhs
            }
        })*
    }
}
impl_rem_addr_usize!(for PhysAddr, VirtAddr, MutVirtAddr);

macro_rules! impl_partialeq_addr_usize {
    (for $($t:ty),+) => {
        $(impl PartialEq<usize> for $t {
            fn eq(&self, other: &usize) -> bool {
                self.address == *other
            }
            fn ne(&self, other: &usize) -> bool {
                !self.eq(other)
            }
        })*
    }
}
impl_partialeq_addr_usize!(for PhysAddr, VirtAddr, MutVirtAddr);
macro_rules! impl_partialord_addr_usize {
    (for $($t:ty),+) => {
        $(impl PartialOrd<usize> for $t {
            fn partial_cmp(&self, other: &usize) -> Option<core::cmp::Ordering> {
                self.address.partial_cmp(&other)
            }

            fn lt(&self, other: &usize) -> bool {
                self.address < *other
            }
            fn le(&self, other: &usize) -> bool {
                self.address <= *other
            }
            fn gt(&self, other: &usize) -> bool {
                self.address > *other
            }
            fn ge(&self, other: &usize) -> bool {
                self.address >= *other
            }
        })*
    }
}
impl_partialord_addr_usize!(for PhysAddr, VirtAddr, MutVirtAddr);

macro_rules! impl_from_addr_usize {
    (for $($t:ty),+) => {
        $(impl From<$t> for usize {
            fn from(address: $t) -> Self {
                address.as_usize()
            }
        })*
    }
}
impl_from_addr_usize!(for PhysAddr, &PhysAddr, VirtAddr, &VirtAddr, MutVirtAddr, &MutVirtAddr);
macro_rules! impl_from_usize_addr {
    (for $($t:ty),+) => {
        $(impl From<usize> for $t {
            fn from(address: usize) -> Self {
                <$t>::new(address)
            }
        })*
    }
}
impl_from_usize_addr!(for PhysAddr, VirtAddr, MutVirtAddr);
