//! Startup splash screen renderer with Ferrox-RC logo and firmware information.

use crate::board::BoardProfile;
use crate::display::St7567;
use crate::ui::glyphs::draw_ferrox_logo;

/// Render the Ferrox-RC splash screen onto the display buffer.
/// - Top: 28x27 Ferrox-RC kinetic delta emblem centered at x = 50, y = 3
/// - Brand Header: "FERROX-RC" in 6x10 centered at x = 37, y = 34
/// - Firmware ID: "flysky-i6x-rs" in 4x6 centered at x = 38, y = 45
/// - Version / Build: "v0.18.1" in 4x6 centered at x = 50, y = 54
pub fn draw_splash(lcd: &mut St7567) {
    lcd.clear_buffer();

    // 1. Draw 28x27 Ferrox-RC Logo Emblem centered horizontally
    draw_ferrox_logo(lcd, 50, 3);

    // 2. Draw "FERROX-RC" in 6x10 (54px wide, centered at x = 37, y = 34)
    lcd.draw_str_6x10(37, 34, "FERROX-RC", false);

    // 3. Draw firmware ID dynamically centered horizontally in 4x6
    let id_x = ((128i32 - crate::board::ActiveBoard::FIRMWARE_ID.len() as i32 * 4) / 2).max(0);
    lcd.draw_str_4x6(id_x, 45, crate::board::ActiveBoard::FIRMWARE_ID, false);

    // 4. Draw Version Tag dynamically centered horizontally (4x6 is 4px wide)
    const VERSION_TAG: &str = concat!(env!("FIRMWARE_VERSION"), " (", env!("GIT_HASH"), ")");
    const VERSION_X: i32 = ((128 - (VERSION_TAG.len() * 4)) / 2) as i32;
    lcd.draw_str_4x6(VERSION_X, 54, VERSION_TAG, false);
}

/// Render the dedicated DFU Bootloader screen onto the display buffer.
///
/// Clears the display and renders:
/// - Top: 28x27 Ferrox-RC kinetic delta emblem centered horizontally (x = 50, y = 2)
/// - Title: "BOOTLOADER" in 6x10 bold font centered horizontally (x = 34, y = 32)
/// - Divider: Horizontal rule across the center (y = 44)
/// - Subtitle: "STM32 DFU MODE ACTIVE" in 4x6 font (x = 22, y = 48)
/// - Status: "Waiting for USB host..." in 4x6 font (x = 18, y = 56)
pub fn draw_bootloader(lcd: &mut St7567) {
    lcd.clear_buffer();

    // 1. Draw 28x27 Ferrox-RC Logo Emblem centered horizontally
    draw_ferrox_logo(lcd, 50, 2);

    // 2. Draw "BOOTLOADER" in 6x10 centered at x = 34, y = 32
    lcd.draw_str_6x10(34, 32, "BOOTLOADER", false);

    // 3. Draw divider line
    lcd.draw_hline(16, 44, 96, true);

    // 4. Draw DFU status lines in 4x6 font
    lcd.draw_str_4x6(22, 48, "STM32 DFU MODE ACTIVE", false);
    lcd.draw_str_4x6(18, 56, "Waiting for USB host...", false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_draw_splash_renders_pixels() {
        let mut lcd = St7567::new();
        draw_splash(&mut lcd);
        let non_zero = lcd.framebuffer().iter().any(|&b| b != 0);
        assert!(non_zero, "Splash screen should render pixels into framebuffer");
    }

    #[test]
    fn test_draw_bootloader_renders_pixels() {
        let mut lcd = St7567::new();
        draw_bootloader(&mut lcd);
        let non_zero = lcd.framebuffer().iter().any(|&b| b != 0);
        assert!(non_zero, "Bootloader screen should render pixels into framebuffer");
    }
}

