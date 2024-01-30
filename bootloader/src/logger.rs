use core::{fmt, mem::MaybeUninit};


pub static mut LOGGER: MaybeUninit<Logger> = MaybeUninit::uninit();

// Simple logger for printing to the screen (expects 8x16 VGA bitmap font)
pub struct Logger {
    vga_bitmap_font: &'static [[u8; 16]; 256],
    color: u32,
    framebuffer_addr: usize,
    pitch: u16,
    bpp: u8,
    x: u16,
    y: u16
}
impl Logger {
    pub fn new(vesa_mode_info_addr: &[u8; 256], vga_bitmap_font: &'static [[u8; 16]; 256]) -> Logger {
        let byte_array = *vesa_mode_info_addr;

        let framebuffer_addr = byte_array[40] as usize | (byte_array[41] as usize) << 8 |
                                      (byte_array[42] as usize) << 16 | (byte_array[43] as usize) << 24;
        let pitch = byte_array[16] as u16 | (byte_array[17] as u16) << 8;
        let bpp = byte_array[25];

        let red_mask = byte_array[31];
        let red_position = byte_array[32];
        let green_mask = byte_array[33];
        let green_position = byte_array[34];
        let blue_mask = byte_array[35];
        let blue_position = byte_array[36];

        let mut red:   u32 = 160;
        let mut green: u32 = 160;
        let mut blue:  u32 = 160;
        if bpp < 24 {
            red = red >> (u8::BITS - red_mask as u32);
            green = green >> (u8::BITS - green_mask as u32);
            blue = blue >> (u8::BITS - blue_mask as u32);
        }
        let color = red << red_position | green << green_position | blue << blue_position;


        Logger { x: 0, y: 0, vga_bitmap_font, color, framebuffer_addr, pitch, bpp }
    }

    pub fn write_string(&mut self, input: &str) {
        for i in input.as_bytes() {
            if *i == b'\n' {
                self.x = 0;
                self.y += 17;
            }
            else {
                self.draw_char(*i as usize, self.x, self.y);
                self.x += 9;
            }
        }
    }

    #[inline]
    fn draw_char(&self, i: usize, x: u16, mut y: u16) {
        for bitmap_row in self.vga_bitmap_font[i] {
            let mut x_pos = x;
            for i in (0..u8::BITS).rev() {
                if (bitmap_row & (1 << i)) != 0 {
                    unsafe { self.put_pixel(x_pos as usize, y as usize); }
                }
                x_pos += 1;
            }
            y += 1;
        }
    }

    #[inline]
    unsafe fn put_pixel(&self, x: usize, y: usize) {
        let location = x*(self.bpp/8) as usize + y*self.pitch as usize;
        let pixel_ptr = (self.framebuffer_addr + location) as *mut u32;
        *pixel_ptr = (*pixel_ptr >> self.bpp) << self.bpp | self.color;
    }
}
impl fmt::Write for Logger {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

// Print macros:
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::logger::_print(format_args!($($arg)*)));
}
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    unsafe {
        LOGGER.assume_init_mut().write_fmt(args).unwrap();
    }
}
