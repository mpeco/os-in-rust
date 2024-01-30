use core::intrinsics::{volatile_copy_memory, volatile_set_memory};

use crate::memory::address::{PhysAddr, MutVirtAddr};


pub struct VBEModeInfo {
    values: [u8; 256]
}
impl VBEModeInfo {
    pub fn framebuffer_addr(&self) -> PhysAddr {
        (self.values[40] as usize | (self.values[41] as usize) << 8 |
        (self.values[42] as usize) << 16 | (self.values[43] as usize) << 24).into()
    }

    pub fn red_mask(&self) -> u8 {
        self.values[31]
    }
    pub fn red_position(&self) -> u8 {
        self.values[32]
    }
    pub fn green_mask(&self) -> u8 {
        self.values[33]
    }
    pub fn green_position(&self) -> u8 {
        self.values[34]
    }
    pub fn blue_mask(&self) -> u8 {
        self.values[35]
    }
    pub fn blue_position(&self) -> u8 {
        self.values[36]
    }
    pub fn reserved_mask(&self) -> u8 {
        self.values[37]
    }
    pub fn reserved_position(&self) -> u8 {
        self.values[38]
    }
    pub fn bpp(&self) -> u8 {
        self.values[25]
    }

    pub fn pitch(&self) -> u16 {
        self.values[16] as u16 | (self.values[17] as u16) << 8
    }
    pub fn width(&self) -> u16 {
        self.values[18] as u16 | (self.values[19] as u16) << 8
    }
    pub fn height(&self) -> u16 {
        self.values[20] as u16 | (self.values[21] as u16) << 8
    }
    pub fn length(&self) -> usize {
        self.pitch() as usize * self.height() as usize
    }
}


pub struct Framebuffer {
    address: MutVirtAddr,
    length: usize,
    pitch: u16,
    bpp: u8,
}
impl Framebuffer {
    pub fn new(vbe_mode_info: &'static VBEModeInfo) -> Framebuffer {
        Framebuffer {
            address: vbe_mode_info.framebuffer_addr().to_mut_virtual(),
            length: vbe_mode_info.length(), pitch: vbe_mode_info.pitch(),
            bpp: vbe_mode_info.bpp()
        }
    }

    pub unsafe fn copy(&mut self, src: usize, dst: usize, length: usize) {
        let src = self.address.as_ptr::<u8>().add(src * (self.bpp/8) as usize);
        let dst = self.address.as_ptr::<u8>().add(dst * (self.bpp/8) as usize);
        let count = length * (self.bpp/8) as usize;
        volatile_copy_memory(dst, src, count);
    }

    pub unsafe fn clear(&mut self, start: usize, length: usize) {
        let dst = self.address.as_ptr::<u8>().add(start * (self.bpp/8) as usize);
        let length = length * (self.bpp/8) as usize;
        volatile_set_memory(dst, 0, length);
    }
    pub fn clear_screen(&mut self) {
        unsafe { volatile_set_memory(self.address.as_ptr::<u8>(), 0, self.length); }
    }

    // Caller must check framebuffer bounds
    #[inline]
    pub unsafe fn put_pixel(&mut self, x: usize, y: usize, color: u32) {
        let location = x*(self.bpp/8) as usize + y*self.pitch as usize;
        let pixel_ptr = (self.address + location).as_ptr::<u32>();
        unsafe { pixel_ptr.write_volatile((*pixel_ptr >> self.bpp) << self.bpp | color); }
    }
}
