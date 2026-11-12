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

    let screen_w = crate::display::st7567::WIDTH as i32;

    // 1. Draw 28x27 Ferrox-RC Logo Emblem centered horizontally
    draw_ferrox_logo(lcd, (screen_w - 28) / 2, 3);

    // 2. Draw "FERROX-RC" in 6x10 (54px wide, centered)
    lcd.draw_str_6x10((screen_w - 54) / 2, 34, "FERROX-RC", false);

    // 3. Draw firmware ID dynamically centered horizontally in 4x6
    let id_x = ((screen_w - crate::board::ActiveBoard::FIRMWARE_ID.len() as i32 * 4) / 2).max(0);
    lcd.draw_str_4x6(id_x, 45, crate::board::ActiveBoard::FIRMWARE_ID, false);

    // 4. Draw Version Tag dynamically centered horizontally (4x6 is 4px wide)
    let version_tag = concat!(env!("FIRMWARE_VERSION"), " (", env!("GIT_HASH"), ")");
    let version_x = ((screen_w - version_tag.len() as i32 * 4) / 2).max(0);
    lcd.draw_str_4x6(version_x, 54, version_tag, false);
}

/// Render the dedicated DFU Bootloader screen onto the display buffer.
pub fn draw_bootloader(lcd: &mut St7567) {
    lcd.clear_buffer();
    let screen_w = crate::display::st7567::WIDTH as i32;

    // 1. Draw 28x27 Ferrox-RC Logo Emblem centered horizontally
    draw_ferrox_logo(lcd, (screen_w - 28) / 2, 2);

    // 2. Draw "BOOTLOADER" in 6x10 (60px wide, centered)
    lcd.draw_str_6x10((screen_w - 60) / 2, 32, "BOOTLOADER", false);

    // 3. Draw divider line centered
    let div_w = 96.min(screen_w - 16);
    lcd.draw_hline((screen_w - div_w) / 2, 44, div_w as u32, true);

    // 4. Draw DFU status lines centered in 4x6 font
    let s1 = "STM32 DFU MODE ACTIVE";
    lcd.draw_str_4x6(((screen_w - s1.len() as i32 * 4) / 2).max(0), 48, s1, false);
    let s2 = "Waiting for USB host...";
    lcd.draw_str_4x6(((screen_w - s2.len() as i32 * 4) / 2).max(0), 56, s2, false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_draw_splash_renders_pixels() {
        let mut lcd = St7567::new();
        draw_splash(&mut lcd);
        let non_zero = lcd.framebuffer().iter().any(|&b| b != 0);
        assert!(
            non_zero,
            "Splash screen should render pixels into framebuffer"
        );
    }

    #[test]
    fn test_draw_bootloader_renders_pixels() {
        let mut lcd = St7567::new();
        draw_bootloader(&mut lcd);
        let non_zero = lcd.framebuffer().iter().any(|&b| b != 0);
        assert!(
            non_zero,
            "Bootloader screen should render pixels into framebuffer"
        );
    }
}
