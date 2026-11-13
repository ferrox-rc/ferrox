//! Zero-heap 4-bit indexed palette color display engine and canvas.
//!
//! Provides 16-color indexed framebuffers (<38 KB RAM for 320x240)
//! expanding via RGB565 look-up tables for color TFT targets (e.g. RadioLink AT9S Pro, TX16S).

use crate::hal::DisplayBus;
use core::convert::Infallible;
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    pixelcolor::{BinaryColor, PixelColor},
    Pixel,
};

/// 16 Semantic UI Color Indices (4 bits per pixel).
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ColorIndex {
    Bg = 0,
    Fg = 1,
    HeaderBg = 2,
    HeaderFg = 3,
    Accent = 4,
    Highlight = 5,
    Muted = 6,
    Ok = 7,
    Warn = 8,
    Danger = 9,
    BarBg = 10,
    BarFg = 11,
    Grid = 12,
    Border = 13,
    Custom1 = 14,
    Custom2 = 15,
}

impl PixelColor for ColorIndex {
    type Raw = ();
}

impl ColorIndex {
    #[inline(always)]
    pub const fn from_u8(val: u8) -> Self {
        match val & 0x0F {
            0 => Self::Bg,
            1 => Self::Fg,
            2 => Self::HeaderBg,
            3 => Self::HeaderFg,
            4 => Self::Accent,
            5 => Self::Highlight,
            6 => Self::Muted,
            7 => Self::Ok,
            8 => Self::Warn,
            9 => Self::Danger,
            10 => Self::BarBg,
            11 => Self::BarFg,
            12 => Self::Grid,
            13 => Self::Border,
            14 => Self::Custom1,
            _ => Self::Custom2,
        }
    }
}

/// Convert 8-bit R, G, B channels into standard 16-bit RGB565 format.
#[inline(always)]
pub const fn rgb565(r: u8, g: u8, b: u8) -> u16 {
    let r5 = ((r as u16) >> 3) & 0x1F;
    let g6 = ((g as u16) >> 2) & 0x3F;
    let b5 = ((b as u16) >> 3) & 0x1F;
    (r5 << 11) | (g6 << 5) | b5
}

/// Default high-contrast dark theme RGB565 palette.
pub const DARK_PALETTE: [u16; 16] = [
    rgb565(20, 24, 30),    // 0: Bg (Dark charcoal/navy)
    rgb565(240, 244, 250), // 1: Fg (Crisp white)
    rgb565(36, 44, 56),    // 2: HeaderBg (Deep slate)
    rgb565(255, 255, 255), // 3: HeaderFg (Pure white)
    rgb565(0, 168, 255),   // 4: Accent (Vibrant electric blue)
    rgb565(255, 170, 0),   // 5: Highlight (Amber yellow)
    rgb565(120, 130, 145), // 6: Muted (Cool gray)
    rgb565(46, 204, 113),  // 7: Ok (Bright green)
    rgb565(243, 156, 18),  // 8: Warn (Vibrant orange)
    rgb565(231, 76, 60),   // 9: Danger (Crimson red)
    rgb565(40, 48, 60),    // 10: BarBg (Dark bar trough)
    rgb565(0, 210, 255),   // 11: BarFg (Cyan bar fill)
    rgb565(50, 60, 75),    // 12: Grid (Subtle grid lines)
    rgb565(70, 82, 100),   // 13: Border (Framing border)
    rgb565(155, 89, 182),  // 14: Custom1 (Purple)
    rgb565(26, 188, 156),  // 15: Custom2 (Turquoise)
];

/// High-visibility direct daylight theme RGB565 palette.
pub const DAYLIGHT_PALETTE: [u16; 16] = [
    rgb565(255, 255, 255), // 0: Bg (White)
    rgb565(0, 0, 0),       // 1: Fg (Black)
    rgb565(220, 225, 230), // 2: HeaderBg (Light gray)
    rgb565(0, 0, 0),       // 3: HeaderFg (Black)
    rgb565(0, 100, 220),   // 4: Accent (Deep blue)
    rgb565(200, 100, 0),   // 5: Highlight (Dark orange)
    rgb565(100, 100, 100), // 6: Muted (Mid gray)
    rgb565(0, 140, 50),    // 7: Ok (Dark green)
    rgb565(210, 110, 0),   // 8: Warn (Dark amber)
    rgb565(200, 20, 20),   // 9: Danger (Dark red)
    rgb565(210, 215, 220), // 10: BarBg (Light bar trough)
    rgb565(0, 90, 200),    // 11: BarFg (Blue bar fill)
    rgb565(190, 195, 200), // 12: Grid (Grid lines)
    rgb565(80, 80, 80),    // 13: Border (Border line)
    rgb565(120, 40, 160),  // 14: Custom1
    rgb565(0, 130, 110),   // 15: Custom2
];

/// Zero-heap 4-bit indexed Color Canvas.
/// `BUF_SIZE` is exactly `(W * H + 1) / 2` bytes (e.g. 38,400 bytes for 320x240).
pub struct ColorCanvas<const W: usize, const H: usize, const BUF_SIZE: usize> {
    framebuffer: [u8; BUF_SIZE],
}

impl<const W: usize, const H: usize, const BUF_SIZE: usize> Default
    for ColorCanvas<W, H, BUF_SIZE>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<const W: usize, const H: usize, const BUF_SIZE: usize> ColorCanvas<W, H, BUF_SIZE> {
    pub const fn new() -> Self {
        Self {
            framebuffer: [0u8; BUF_SIZE],
        }
    }

    #[inline(always)]
    pub fn framebuffer(&self) -> &[u8; BUF_SIZE] {
        &self.framebuffer
    }

    #[inline(always)]
    pub fn clear(&mut self, color: ColorIndex) {
        let nibble = color as u8 & 0x0F;
        let byte_val = (nibble << 4) | nibble;
        self.framebuffer.fill(byte_val);
    }

    #[inline]
    pub fn set_pixel(&mut self, x: i32, y: i32, color: ColorIndex) {
        if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
            return;
        }
        let index = y as usize * W + x as usize;
        let byte_idx = index / 2;
        let val = color as u8 & 0x0F;
        if (index & 1) == 0 {
            // Even pixel: high nibble
            self.framebuffer[byte_idx] = (self.framebuffer[byte_idx] & 0x0F) | (val << 4);
        } else {
            // Odd pixel: low nibble
            self.framebuffer[byte_idx] = (self.framebuffer[byte_idx] & 0xF0) | val;
        }
    }

    #[inline]
    pub fn get_pixel(&self, x: i32, y: i32) -> ColorIndex {
        if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
            return ColorIndex::Bg;
        }
        let index = y as usize * W + x as usize;
        let byte_idx = index / 2;
        let byte = self.framebuffer[byte_idx];
        let val = if (index & 1) == 0 {
            byte >> 4
        } else {
            byte & 0x0F
        };
        ColorIndex::from_u8(val)
    }

    /// Flush the indexed canvas to the screen via the registered display bus.
    pub fn flush(&self, bus: &mut dyn DisplayBus, palette: &[u16; 16]) {
        bus.flush_indexed_rgb565(&self.framebuffer, palette);
    }
}

impl<const W: usize, const H: usize, const BUF_SIZE: usize> OriginDimensions
    for ColorCanvas<W, H, BUF_SIZE>
{
    fn size(&self) -> Size {
        Size::new(W as u32, H as u32)
    }
}

/// Implement DrawTarget for ColorIndex (enables direct color primitives).
impl<const W: usize, const H: usize, const BUF_SIZE: usize> DrawTarget
    for ColorCanvas<W, H, BUF_SIZE>
{
    type Color = ColorIndex;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels.into_iter() {
            self.set_pixel(coord.x, coord.y, color);
        }
        Ok(())
    }
}

/// Implement DrawTarget for BinaryColor (backward-compatible drawing of existing monochrome UI).
/// Maps BinaryColor::On -> ColorIndex::Fg, BinaryColor::Off -> ColorIndex::Bg.
pub struct BinaryColorAdapter<'a, const W: usize, const H: usize, const BUF_SIZE: usize> {
    canvas: &'a mut ColorCanvas<W, H, BUF_SIZE>,
}

impl<'a, const W: usize, const H: usize, const BUF_SIZE: usize>
    BinaryColorAdapter<'a, W, H, BUF_SIZE>
{
    pub fn new(canvas: &'a mut ColorCanvas<W, H, BUF_SIZE>) -> Self {
        Self { canvas }
    }
}

impl<'a, const W: usize, const H: usize, const BUF_SIZE: usize> OriginDimensions
    for BinaryColorAdapter<'a, W, H, BUF_SIZE>
{
    fn size(&self) -> Size {
        Size::new(W as u32, H as u32)
    }
}

impl<'a, const W: usize, const H: usize, const BUF_SIZE: usize> DrawTarget
    for BinaryColorAdapter<'a, W, H, BUF_SIZE>
{
    type Color = BinaryColor;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels.into_iter() {
            let mapped = match color {
                BinaryColor::On => ColorIndex::Fg,
                BinaryColor::Off => ColorIndex::Bg,
            };
            self.canvas.set_pixel(coord.x, coord.y, mapped);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb565_conversion() {
        assert_eq!(rgb565(0, 0, 0), 0x0000); // Black
        assert_eq!(rgb565(255, 255, 255), 0xFFFF); // White
        assert_eq!(rgb565(255, 0, 0), 0xF800); // Red
        assert_eq!(rgb565(0, 255, 0), 0x07E0); // Green
        assert_eq!(rgb565(0, 0, 255), 0x001F); // Blue
    }

    #[test]
    fn test_4bpp_pixel_packing_and_clipping() {
        let mut canvas = ColorCanvas::<320, 240, 38400>::new();
        canvas.clear(ColorIndex::Bg);

        // Even pixel (high nibble)
        canvas.set_pixel(0, 0, ColorIndex::Accent);
        assert_eq!(canvas.get_pixel(0, 0), ColorIndex::Accent);
        assert_eq!(canvas.get_pixel(1, 0), ColorIndex::Bg);

        // Odd pixel (low nibble)
        canvas.set_pixel(1, 0, ColorIndex::Ok);
        assert_eq!(canvas.get_pixel(0, 0), ColorIndex::Accent);
        assert_eq!(canvas.get_pixel(1, 0), ColorIndex::Ok);

        // Out-of-bounds pixels should not panic or corrupt
        canvas.set_pixel(-1, 0, ColorIndex::Danger);
        canvas.set_pixel(320, 0, ColorIndex::Danger);
        canvas.set_pixel(0, 240, ColorIndex::Danger);
        assert_eq!(canvas.get_pixel(-1, 0), ColorIndex::Bg);
    }

    #[test]
    fn test_binary_color_adapter() {
        let mut canvas = ColorCanvas::<320, 240, 38400>::new();
        canvas.clear(ColorIndex::Bg);

        {
            let mut adapter = BinaryColorAdapter::new(&mut canvas);
            adapter
                .draw_iter([
                    Pixel(
                        embedded_graphics::geometry::Point::new(10, 10),
                        BinaryColor::On,
                    ),
                    Pixel(
                        embedded_graphics::geometry::Point::new(11, 10),
                        BinaryColor::Off,
                    ),
                ])
                .unwrap();
        }

        assert_eq!(canvas.get_pixel(10, 10), ColorIndex::Fg);
        assert_eq!(canvas.get_pixel(11, 10), ColorIndex::Bg);
    }
}
