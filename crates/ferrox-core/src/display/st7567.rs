//! ST7567 128×64 Monochrome LCD 8-bit Parallel Driver with Embedded-Graphics Support.

use core::convert::Infallible;
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    pixelcolor::BinaryColor,
    Pixel,
};
use crate::hal::DisplayBus;

static mut DISPLAY_BUS: Option<&'static mut dyn DisplayBus> = None;

/// Register the target MCU's display bus and backlight driver.
pub fn register_display_bus(bus: &'static mut dyn DisplayBus) {
    unsafe {
        DISPLAY_BUS = Some(bus);
    }
}

pub const WIDTH: usize = 128;
pub const HEIGHT: usize = 64;
pub const BUFFER_SIZE: usize = WIDTH * (HEIGHT / 8); // 1024 bytes

/// ST7567 LCD Driver with 1024-byte internal framebuffer.
pub struct St7567 {
    framebuffer: [u8; BUFFER_SIZE],
}

impl Default for St7567 {
    fn default() -> Self {
        Self::new()
    }
}

impl St7567 {
    /// Initialize framebuffer and clear display.
    pub fn new() -> Self {
        unsafe {
            if let Some(ref mut bus) = DISPLAY_BUS {
                bus.init();
            }
        }
        let mut display = Self {
            framebuffer: [0u8; BUFFER_SIZE],
        };
        display.clear_buffer();
        display.flush();
        display.set_backlight_level(100);
        display
    }


    /// Set Backlight brightness level (0..100%).
    pub fn set_backlight_level(&self, pct: u8) {
        unsafe {
            if let Some(ref mut bus) = DISPLAY_BUS {
                bus.set_backlight_level(pct);
            }
        }
    }

    /// Convenience helper for binary ON/OFF control.
    #[allow(dead_code)]
    pub fn set_backlight(&self, on: bool) {
        self.set_backlight_level(if on { 100 } else { 0 });
    }

    /// Set ST7567 Electronic Volume (EV) contrast value (0..63).
    pub fn set_contrast(&self, ev: u8) {
        let val = ev.min(63);
        unsafe {
            if let Some(ref mut bus) = DISPLAY_BUS {
                bus.set_contrast(val);
            }
        }
    }

    /// Flush the entire 1024-byte framebuffer to the screen via the registered display bus.
    pub fn flush(&self) {
        unsafe {
            if let Some(ref mut bus) = DISPLAY_BUS {
                bus.flush(&self.framebuffer);
            }
        }
    }

    /// Clear the framebuffer (all pixels off).
    #[allow(dead_code)]
    pub fn clear_buffer(&mut self) {
        self.framebuffer.fill(0);
    }

    /// Set a single pixel at (x, y) to on or off with bounds checking.
    #[inline]
    pub fn set_pixel(&mut self, x: i32, y: i32, on: bool) {
        if x >= 0 && (x as usize) < WIDTH && y >= 0 && (y as usize) < HEIGHT {
            let page = (y as usize) / 8;
            let bit = (y as usize) % 8;
            let index = page * WIDTH + (x as usize);
            if on {
                self.framebuffer[index] |= 1 << bit;
            } else {
                self.framebuffer[index] &= !(1 << bit);
            }
        }
    }

    /// Fast horizontal line from (x, y) with length `w`.
    pub fn draw_hline(&mut self, x: i32, y: i32, w: u32, on: bool) {
        if y < 0 || (y as usize) >= HEIGHT || w == 0 {
            return;
        }
        let x_start = x.max(0) as usize;
        let x_end = ((x + w as i32).max(0) as usize).min(WIDTH);
        if x_start >= x_end {
            return;
        }
        let page = (y as usize) / 8;
        let bit = (y as usize) % 8;
        let mask = 1u8 << bit;
        let start_idx = page * WIDTH + x_start;
        let end_idx = page * WIDTH + x_end;

        if on {
            for b in &mut self.framebuffer[start_idx..end_idx] {
                *b |= mask;
            }
        } else {
            let inv_mask = !mask;
            for b in &mut self.framebuffer[start_idx..end_idx] {
                *b &= inv_mask;
            }
        }
    }

    /// Fast vertical line from (x, y) with length `h`.
    pub fn draw_vline(&mut self, x: i32, y: i32, h: u32, on: bool) {
        if x < 0 || (x as usize) >= WIDTH || h == 0 {
            return;
        }
        let y_start = y.max(0) as usize;
        let y_end = ((y + h as i32).max(0) as usize).min(HEIGHT);
        if y_start >= y_end {
            return;
        }
        let start_page = y_start / 8;
        let end_page = (y_end - 1) / 8;
        let col = x as usize;

        for page in start_page..=end_page {
            let page_y_start = page * 8;
            let page_y_end = page_y_start + 7;
            let bit_start = y_start.saturating_sub(page_y_start);
            let bit_end = if y_end - 1 < page_y_end {
                (y_end - 1) - page_y_start
            } else {
                7
            };

            let mut mask = 0u8;
            for b in bit_start..=bit_end {
                mask |= 1 << b;
            }

            let idx = page * WIDTH + col;
            if on {
                self.framebuffer[idx] |= mask;
            } else {
                self.framebuffer[idx] &= !mask;
            }
        }
    }

    /// Fast filled rectangle from (x, y) with size (w, h).
    pub fn fill_rect(&mut self, x: i32, y: i32, w: u32, h: u32, on: bool) {
        if w == 0 || h == 0 {
            return;
        }
        let x_start = x.max(0) as usize;
        let x_end = ((x + w as i32).max(0) as usize).min(WIDTH);
        if x_start >= x_end {
            return;
        }
        let y_start = y.max(0) as usize;
        let y_end = ((y + h as i32).max(0) as usize).min(HEIGHT);
        if y_start >= y_end {
            return;
        }
        let start_page = y_start / 8;
        let end_page = (y_end - 1) / 8;

        for page in start_page..=end_page {
            let page_y_start = page * 8;
            let page_y_end = page_y_start + 7;
            let bit_start = y_start.saturating_sub(page_y_start);
            let bit_end = if y_end - 1 < page_y_end {
                (y_end - 1) - page_y_start
            } else {
                7
            };

            let mask = if bit_start == 0 && bit_end == 7 {
                0xFFu8
            } else {
                let mut m = 0u8;
                for b in bit_start..=bit_end {
                    m |= 1 << b;
                }
                m
            };

            let row_start = page * WIDTH + x_start;
            let row_end = page * WIDTH + x_end;

            if on {
                if mask == 0xFF {
                    self.framebuffer[row_start..row_end].fill(0xFF);
                } else {
                    for b in &mut self.framebuffer[row_start..row_end] {
                        *b |= mask;
                    }
                }
            } else {
                if mask == 0xFF {
                    self.framebuffer[row_start..row_end].fill(0x00);
                } else {
                    let inv_mask = !mask;
                    for b in &mut self.framebuffer[row_start..row_end] {
                        *b &= inv_mask;
                    }
                }
            }
        }
    }

    /// Fast outline rectangle from (x, y) with size (w, h).
    pub fn draw_rect(&mut self, x: i32, y: i32, w: u32, h: u32, on: bool) {
        if w == 0 || h == 0 {
            return;
        }
        self.draw_hline(x, y, w, on);
        if h > 1 {
            self.draw_hline(x, y + h as i32 - 1, w, on);
            if h > 2 {
                self.draw_vline(x, y + 1, h - 2, on);
                if w > 1 {
                    self.draw_vline(x + w as i32 - 1, y + 1, h - 2, on);
                }
            }
        }
    }

    #[inline(never)]
    fn draw_glyph_cols(&mut self, x: i32, y: i32, cols: &[u16], height: i32, invert: bool) {
        for (col, &col_bits) in cols.iter().enumerate() {
            let px = x + col as i32;
            if px < 0 || px >= WIDTH as i32 {
                continue;
            }
            for row in 0..height {
                let py = y + row;
                if py < 0 || py >= HEIGHT as i32 {
                    continue;
                }
                let bit_set = (col_bits & (1 << row)) != 0;
                if invert {
                    if bit_set {
                        self.set_pixel(px, py, false);
                    }
                } else if bit_set {
                    self.set_pixel(px, py, true);
                }
            }
        }
    }

    /// Render a single 4x6 character at (x, y).
    /// If `invert` is true, pixels that are ON become OFF (useful over filled inverted bars).
    #[inline(never)]
    pub fn draw_char_4x6(&mut self, x: i32, y: i32, c: u8, invert: bool) {
        if !(0x20..=0x7E).contains(&c) {
            return;
        }
        let glyph_idx = (c - 0x20) as usize;
        let glyph = &crate::display::fonts::FONT_4X6_DATA[glyph_idx];
        let cols = [glyph[0] as u16, glyph[1] as u16, glyph[2] as u16, glyph[3] as u16];
        self.draw_glyph_cols(x, y, &cols, 6, invert);
    }

    /// Render a 4x6 ASCII string starting at top-left (x, y).
    #[inline(never)]
    pub fn draw_str_4x6(&mut self, mut x: i32, y: i32, text: &str, invert: bool) {
        for &byte in text.as_bytes() {
            self.draw_char_4x6(x, y, byte, invert);
            x += 4;
        }
    }

    /// Render a single 6x10 character at (x, y).
    /// If `invert` is true, pixels that are ON become OFF (useful over filled inverted bars).
    #[inline(never)]
    pub fn draw_char_6x10(&mut self, x: i32, y: i32, c: u8, invert: bool) {
        if !(0x20..=0x7E).contains(&c) {
            return;
        }
        let glyph_idx = (c - 0x20) as usize;
        let glyph = &crate::display::fonts::FONT_6X10_DATA[glyph_idx];
        self.draw_glyph_cols(x, y, glyph, 10, invert);
    }

    /// Render a 6x10 ASCII string starting at top-left (x, y).
    #[inline(never)]
    pub fn draw_str_6x10(&mut self, mut x: i32, y: i32, text: &str, invert: bool) {
        for &byte in text.as_bytes() {
            self.draw_char_6x10(x, y, byte, invert);
            x += 6;
        }
    }

    #[cfg(test)]
    pub fn framebuffer(&self) -> &[u8; BUFFER_SIZE] {
        &self.framebuffer
    }
}

impl OriginDimensions for St7567 {
    fn size(&self) -> Size {
        Size::new(WIDTH as u32, HEIGHT as u32)
    }
}

impl DrawTarget for St7567 {
    type Color = BinaryColor;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels.into_iter() {
            if coord.x >= 0 && coord.x < WIDTH as i32 && coord.y >= 0 && coord.y < HEIGHT as i32 {
                let x = coord.x as usize;
                let y = coord.y as usize;

                let page = y / 8;
                let bit = y % 8;
                let index = page * WIDTH + x;

                match color {
                    BinaryColor::On => {
                        self.framebuffer[index] |= 1 << bit;
                    }
                    BinaryColor::Off => {
                        self.framebuffer[index] &= !(1 << bit);
                    }
                }
            }
        }
        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        let fill = match color {
            BinaryColor::On => 0xFF,
            BinaryColor::Off => 0x00,
        };
        self.framebuffer.fill(fill);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::prelude::*;
    use embedded_graphics::primitives::{Line, PrimitiveStyle, Rectangle};

    #[test]
    fn test_draw_hline_matches_embedded_graphics() {
        let mut d1 = St7567::new();
        let mut d2 = St7567::new();

        // Direct primitive
        d1.draw_hline(10, 15, 50, true);

        // embedded_graphics
        Line::new(Point::new(10, 15), Point::new(59, 15))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(&mut d2)
            .unwrap();

        assert_eq!(d1.framebuffer(), d2.framebuffer());
    }

    #[test]
    fn test_draw_vline_matches_embedded_graphics() {
        let mut d1 = St7567::new();
        let mut d2 = St7567::new();

        // Direct primitive spanning multiple pages
        d1.draw_vline(25, 6, 20, true);

        // embedded_graphics
        Line::new(Point::new(25, 6), Point::new(25, 25))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(&mut d2)
            .unwrap();

        assert_eq!(d1.framebuffer(), d2.framebuffer());
    }

    #[test]
    fn test_fill_rect_matches_embedded_graphics() {
        let mut d1 = St7567::new();
        let mut d2 = St7567::new();

        // List item highlight shape (e.g. 2, 14, 124, 9)
        d1.fill_rect(2, 14, 124, 9, true);

        Rectangle::new(Point::new(2, 14), Size::new(124, 9))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(&mut d2)
            .unwrap();

        assert_eq!(d1.framebuffer(), d2.framebuffer());
    }

    #[test]
    fn test_draw_rect_matches_embedded_graphics() {
        let mut d1 = St7567::new();
        let mut d2 = St7567::new();

        d1.draw_rect(10, 10, 40, 20, true);

        Rectangle::new(Point::new(10, 10), Size::new(40, 20))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(&mut d2)
            .unwrap();

        assert_eq!(d1.framebuffer(), d2.framebuffer());
    }

    #[test]
    fn test_primitives_out_of_bounds_no_panic() {
        let mut d = St7567::new();
        // Negative coords, zero width/height, exceeding width/height
        d.draw_hline(-10, 20, 5, true);
        d.draw_hline(100, 20, 50, true);
        d.draw_hline(10, -5, 50, true);
        d.draw_hline(10, 100, 50, true);
        d.draw_vline(-10, 20, 5, true);
        d.draw_vline(20, -10, 5, true);
        d.draw_vline(20, 50, 30, true);
        d.fill_rect(-20, -20, 200, 200, true);
        d.draw_rect(-20, -20, 200, 200, true);
    }

    #[test]
    fn test_baremetal_fonts_match_embedded_graphics() {
        use embedded_graphics::mono_font::ascii::{FONT_4X6, FONT_6X10};
        use embedded_graphics::text::{Baseline, Text};

        // Verify FONT_4X6 for all printable ASCII characters
        for c in 0x20u8..=0x7Eu8 {
            let mut d1 = St7567::new();
            let mut d2 = St7567::new();

            d1.draw_char_4x6(10, 15, c, false);

            let buf = [c];
            let s = core::str::from_utf8(&buf).unwrap();
            let style =
                embedded_graphics::mono_font::MonoTextStyle::new(&FONT_4X6, BinaryColor::On);
            Text::with_baseline(s, Point::new(10, 15), style, Baseline::Top)
                .draw(&mut d2)
                .unwrap();

            assert_eq!(
                d1.framebuffer(),
                d2.framebuffer(),
                "Mismatch for 4x6 char '{}' (0x{:02x})",
                c as char,
                c
            );
        }

        // Verify FONT_6X10 for all printable ASCII characters
        for c in 0x20u8..=0x7Eu8 {
            let mut d1 = St7567::new();
            let mut d2 = St7567::new();

            d1.draw_char_6x10(10, 15, c, false);

            let buf = [c];
            let s = core::str::from_utf8(&buf).unwrap();
            let style =
                embedded_graphics::mono_font::MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
            Text::with_baseline(s, Point::new(10, 15), style, Baseline::Top)
                .draw(&mut d2)
                .unwrap();

            assert_eq!(
                d1.framebuffer(),
                d2.framebuffer(),
                "Mismatch for 6x10 char '{}' (0x{:02x})",
                c as char,
                c
            );
        }
    }
}
