use core::fmt;

use crate::{
    locks::spinlock::Spinlock,
    memory::address::VirtAddr, utils::lazy_static::LazyStatic,
};
use super::{
    vesa::{Framebuffer, VBEModeInfo},
    color::{self, Color, COLOR_BUILDER}
};


const PIXELS_PER_COLUMN: u16 = 9; // 8 bytes per char plus 1 byte for space
const PIXELS_PER_LINE: u16 = 17;  // 16 bytes per char plus 1 byte for space

pub static LOGGER: LazyStatic<Spinlock<Logger>> = LazyStatic::new();

pub fn init(vbe_mode_info: &'static VBEModeInfo, vga_bitmap_font_addr: VirtAddr, color: Color) {
    LOGGER.init(Spinlock::new(Logger::new(vbe_mode_info, vga_bitmap_font_addr, color)));
    LOGGER.lock().clear_screen();
}

pub struct Logger {
    framebuffer: Framebuffer,
    vga_bitmap_font: &'static [[u8; 16]; 256],
    width: u16,
    column: u16,
    line: u16,
    max_column: u16,
    max_line: u16,
    color: u32
}
impl Logger {
    fn new(vbe_mode_info: &'static VBEModeInfo, vga_bitmap_font_addr: VirtAddr, color: Color) -> Logger {
        let framebuffer = Framebuffer::new(vbe_mode_info);
        let vga_bitmap_font = unsafe { &*vga_bitmap_font_addr.as_ptr::<[[u8; 16]; 256]>() };
        let width = vbe_mode_info.width();
        let max_column = vbe_mode_info.width()/PIXELS_PER_COLUMN;
        let max_line = vbe_mode_info.height()/PIXELS_PER_LINE;
        let color = COLOR_BUILDER.build(color);
        Logger { framebuffer, vga_bitmap_font, width, column: 0, line: 0, max_column, max_line, color }
    }

    fn write_string(&mut self, input: &str) {
        for i in input.as_bytes() {
            if *i == b'\n' {
                self.new_line();
            }
            else {
                if self.column+1 > self.max_column {
                    self.wrap_line();
                }
                self.draw_char(*i as usize);
                self.column += 1;
            }
        }
    }

    pub fn get_color(&self) -> Color {
        COLOR_BUILDER.reverse(self.color)
    }
    pub fn set_color(&mut self, color: Color) {
        self.color = COLOR_BUILDER.build(color);
    }

    fn new_line(&mut self) {
        if self.line+1 >= self.max_line {
            self.scroll_down();
        }
        else {
            self.line += 1;
        }
        self.column = 0;
    }
    fn wrap_line(&mut self) {
        if self.line+1 >= self.max_line {
            self.scroll_down();
        }
        else {
            self.line += 1;
        }
        self.column = 0;
    }

    // Moves every line up by one
    pub fn scroll_down(&mut self) {
        // copy 2nd line below one line up
        let src = self.width as usize * PIXELS_PER_LINE as usize;
        let length = self.width as usize * ((self.max_line-1)*PIXELS_PER_LINE) as usize;
        unsafe { self.framebuffer.copy(src, 0, length); }
        // clear last line
        let start = self.width as usize * ((self.max_line-1)*PIXELS_PER_LINE) as usize;
        let length = self.width as usize * PIXELS_PER_LINE as usize;
        unsafe { self.framebuffer.clear(start, length); }
    }

    #[inline]
    fn draw_char(&mut self, i: usize) {
        let x = self.column*PIXELS_PER_COLUMN;
        let mut y = self.line*PIXELS_PER_LINE;

        for bitmap_row in self.vga_bitmap_font[i] {
            let mut x_pos = x;
            for i in (0..u8::BITS).rev() {
                if (bitmap_row & (1 << i)) != 0 {
                    unsafe {
                        self.framebuffer.put_pixel(x_pos as usize, y as usize, self.color);
                    }
                }
                x_pos += 1;
            }
            y += 1;
        }
    }

    pub fn clear_screen(&mut self) {
        self.column = 0; self.line = 0;
        self.framebuffer.clear_screen();
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
    ($($arg:tt)*) => ($crate::video::logger::_print(format_args!($($arg)*)));
}
#[macro_export]
macro_rules! no_enable_irq_print {
    ($($arg:tt)*) => ($crate::video::logger::_no_enable_irq_print(format_args!($($arg)*)));
}
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
#[macro_export]
macro_rules! print_color {
    ($c:expr,$($arg:tt)*) => ($crate::video::logger::_print_color($c, format_args!($($arg)*)));
}
#[macro_export]
macro_rules! no_enable_irq_print_color {
    ($c:expr,$($arg:tt)*) => ($crate::video::logger::_no_enable_irq_print_color($c, format_args!($($arg)*)));
}
#[macro_export]
macro_rules! println_color {
    () => ($crate::print!("\n"));
    ($c:expr,$($arg:tt)*) => ($crate::print_color!($c, "{}\n", format_args!($($arg)*)));
}
#[macro_export]
macro_rules! eprint {
    ($($arg:tt)*) => ($crate::video::logger::_eprint(format_args!($($arg)*)));
}
#[macro_export]
macro_rules! eprintln {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::eprint!("{}\n", format_args!($($arg)*)));
}

pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use crate::x86_64::interrupts::interrupts_disabled;

    // execute with interrupts disabled to avoid deadlock
    interrupts_disabled(|| {
        LOGGER.lock().write_fmt(args).unwrap();
    });
}
pub fn _no_enable_irq_print(args: fmt::Arguments) {
    use core::fmt::Write;

    LOGGER.lock().write_fmt(args).unwrap();
}
pub fn _print_color(color: Color, args: fmt::Arguments) {
    use core::fmt::Write;
    use crate::x86_64::interrupts::interrupts_disabled;

    // execute with interrupts disabled to avoid deadlock
    interrupts_disabled(|| {
        let mut logger = LOGGER.lock();
        let prev_color = logger.get_color();
        logger.set_color(color);
        logger.write_fmt(args).unwrap();
        logger.set_color(prev_color);
    });
}
pub fn _no_enable_irq_print_color(color: Color, args: fmt::Arguments) {
    use core::fmt::Write;

    let mut logger = LOGGER.lock();
    let prev_color = logger.get_color();
    logger.set_color(color);
    logger.write_fmt(args).unwrap();
    logger.set_color(prev_color);
}
pub fn _eprint(args: fmt::Arguments) {
    print_color!(color::RED, "{args}");
}
