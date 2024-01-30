use crate::utils::lazy_static::LazyStatic;
use super::vesa::VBEModeInfo;


pub const GREY: Color = Color::new(160, 160, 160);
pub const RED: Color = Color::new(255, 0, 0);
pub const DARK_GREEN: Color = Color::new(0, 200, 0);
pub const SAFETY_YELLOW: Color = Color::new(238, 210, 2);


#[derive(Clone, Copy)]
pub struct Color{
    pub red: u8,
    pub green: u8,
    pub blue: u8
}
impl Color {
    pub const fn new(red: u8, green: u8, blue: u8) -> Color {
        Color{ red, green, blue }
    }
}

pub static COLOR_BUILDER: LazyStatic<ColorBuilder> = LazyStatic::new();

pub fn init(vbe_mode_info: &'static VBEModeInfo) {
    COLOR_BUILDER.init(ColorBuilder::new(vbe_mode_info));
}

// Builds R, G and B values into color for the set color depth
pub struct ColorBuilder {
    bpp: u8,
    red_mask: u8,
    red_position: u8,
    green_mask: u8,
    green_position: u8,
    blue_mask: u8,
    blue_position: u8,
}
impl ColorBuilder {
    fn new(vbe_mode_info: &'static VBEModeInfo) -> ColorBuilder {
        ColorBuilder {
            bpp: vbe_mode_info.bpp(),
            red_mask: vbe_mode_info.red_mask(), red_position: vbe_mode_info.red_position(),
            green_mask: vbe_mode_info.green_mask(), green_position: vbe_mode_info.green_position(),
            blue_mask: vbe_mode_info.blue_mask(), blue_position: vbe_mode_info.blue_position()
        }
    }

    pub fn build(&self, mut color: Color) -> u32 {
        if self.bpp < 24 {
            color.red = color.red >> (u8::BITS - self.red_mask as u32);
            color.green = color.green >> (u8::BITS - self.green_mask as u32);
            color.blue = color.blue >> (u8::BITS - self.blue_mask as u32);
        }

        (color.red as u32) << self.red_position
        | (color.green as u32) << self.green_position
        | (color.blue as u32) << self.blue_position
    }

    pub fn reverse(&self, color: u32) -> Color {
        let red = ((color >> self.red_position) as u8) << (u8::BITS - self.red_mask as u32)
            >> (u8::BITS - self.red_mask as u32);
        let green = ((color >> self.green_position) as u8) << (u8::BITS - self.green_mask as u32)
        >> (u8::BITS - self.green_mask as u32);
        let blue = ((color >> self.blue_position) as u8) << (u8::BITS - self.blue_mask as u32)
        >> (u8::BITS - self.blue_mask as u32);
        Color::new(red, green, blue)
    }
}
