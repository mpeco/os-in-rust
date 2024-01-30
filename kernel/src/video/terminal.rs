use alloc::string::String;
use alloc::vec::Vec;

use crate::{
    drivers::keyboard, locks::spinlock::Spinlock,
    memory::address::VirtAddr, utils::{init_once::InitOnce, lazy_static::LazyStatic}
};
use super::{
    vesa::{Framebuffer, VBEModeInfo},
    color::{self, COLOR_BUILDER}
};


const PIXELS_PER_COLUMN: u16 = 9; // 8 bytes per char plus 1 byte for space
const PIXELS_PER_LINE: u16 = 17;  // 16 bytes per char plus 1 byte for space
const INIT_STRING_CAPACITY: usize = 128;

static TERMINAL: LazyStatic<Spinlock<Terminal>> = LazyStatic::new();
static HAS_FIRST_CHARACTER_BEEN_TYPED: InitOnce = InitOnce::new();


pub fn init(vbe_mode_info: &'static VBEModeInfo, vga_bitmap_font_addr: VirtAddr, buffer_capacity: usize) {
    TERMINAL.init(Spinlock::new(Terminal::new(vbe_mode_info, vga_bitmap_font_addr, buffer_capacity)));
}

pub fn terminal_task(_args: *const ()) {
    use keyboard::scancode::IbmXt;

    let mut terminal = TERMINAL.lock_hlt();

    loop {
        let scancode = keyboard::retrieve_scancode(); // halts until a key is pressed
        if let Ok(key) = TryInto::<IbmXt>::try_into(scancode) {
            if let Some(char) = key.to_char() {
                if let Ok(()) = HAS_FIRST_CHARACTER_BEEN_TYPED.init() {
                    terminal.clear_screen();
                }

                terminal.write_string(char);

                if char != "\n" {
                    terminal.cur_string.push(char.chars().next().unwrap());
                }
                else {
                    terminal.cur_string.shrink_to_fit();
                    let prev_string = core::mem::replace(&mut terminal.cur_string, String::with_capacity(INIT_STRING_CAPACITY));
                    terminal.buffer.push(prev_string);
                }
            }
            else {
                match key {
                    IbmXt::Backspace => {
                    }
                    _ => {}
                }
            }
        }
    }
}

struct Terminal {
    framebuffer: Framebuffer,
    vga_bitmap_font: &'static [[u8; 16]; 256],
    width: u16,
    column: u16,
    line: u16,
    max_column: u16,
    max_line: u16,
    color: u32,
    buffer: Vec<String>,
    cur_string: String
}
impl Terminal {
    fn new(vbe_mode_info: &'static VBEModeInfo, vga_bitmap_font_addr: VirtAddr, buffer_capacity: usize) -> Terminal {
        Terminal {
            framebuffer: Framebuffer::new(vbe_mode_info),
            vga_bitmap_font: unsafe { &*vga_bitmap_font_addr.as_ptr::<[[u8; 16]; 256]>() },
            width: vbe_mode_info.width(),
            column: 0, line: 0,
            max_column: vbe_mode_info.width()/PIXELS_PER_COLUMN,
            max_line: vbe_mode_info.height()/PIXELS_PER_LINE,
            color: COLOR_BUILDER.build(color::GREY),
            buffer: Vec::with_capacity(buffer_capacity),
            cur_string: String::with_capacity(INIT_STRING_CAPACITY)
        }
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
    fn scroll_down(&mut self) {
        // copy 2nd line below one line up
        let src = self.width as usize * PIXELS_PER_LINE as usize;
        let length = self.width as usize * ((self.max_line-1)*PIXELS_PER_LINE) as usize;
        unsafe { self.framebuffer.copy(src, 0, length); }
        // clear last line
        let start = self.width as usize * ((self.max_line-1)*PIXELS_PER_LINE) as usize;
        let length = self.width as usize * PIXELS_PER_LINE as usize;
        unsafe { self.framebuffer.clear(start, length); }
    }

    // fn get_color(&self) -> Color {
    //     COLOR_BUILDER.reverse(self.color)
    // }
    // fn set_color(&mut self, color: Color) {
    //     self.color = COLOR_BUILDER.build(color);
    // }

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

    fn clear_screen(&mut self) {
        self.framebuffer.clear_screen();
    }
}
