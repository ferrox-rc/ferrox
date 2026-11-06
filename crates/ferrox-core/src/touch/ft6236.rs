//! FocalTech FT6236 Capacitive Touchscreen packet decoder and touch interface for FlySky FS-i6S.

pub const FT6236_I2C_ADDR: u8 = 0x38;

pub use crate::hal::register_touch_driver;

/// FT6236 Recognized Hardware Gestures.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Gesture {
    None,
    SwipeUp,
    SwipeRight,
    SwipeDown,
    SwipeLeft,
}

/// Touch Contact Event Kind.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TouchEventKind {
    PressDown,
    LiftUp,
    Contact,
    NoEvent,
}

/// Decoded Single Touch Point in 128×64 LCD space.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TouchPoint {
    pub x: u8, // 0..127
    pub y: u8, // 0..63
    pub raw_x: u16,
    pub raw_y: u16,
    pub event: TouchEventKind,
}

/// Complete Touch Sample parsed from a 7-byte FT6236 packet.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TouchSample {
    pub gesture: Gesture,
    pub point: Option<TouchPoint>,
}

/// Parse a raw 7-byte FT6236 register frame (`0x00..0x06`).
///
/// Decodes hardware gestures and translates raw capacitive coordinates into 128×64 LCD space.
pub fn parse_packet(buf: &[u8; 7]) -> TouchSample {
    let gest_id = buf[1];
    let gesture = match gest_id {
        0x10 => Gesture::SwipeUp,
        0x14 => Gesture::SwipeRight,
        0x18 => Gesture::SwipeDown,
        0x1C => Gesture::SwipeLeft,
        _ => Gesture::None,
    };

    let touch_count = buf[2] & 0x0F;
    let point = if touch_count > 0 {
        let raw_x = (((buf[3] & 0x0F) as u16) << 8) | (buf[4] as u16);
        let raw_y = (((buf[5] & 0x0F) as u16) << 8) | (buf[6] as u16);

        let event = match (buf[3] >> 6) & 0x03 {
            0 => TouchEventKind::PressDown,
            1 => TouchEventKind::LiftUp,
            2 => TouchEventKind::Contact,
            _ => TouchEventKind::NoEvent,
        };

        // Axis Swap & Scaling:
        // raw_x (0..128 approx) -> LCD Y (0..63)
        // raw_y (0..256 approx) -> LCD X (0..127, inverted)
        let lcd_y = ((raw_x >> 1) as u8).min(63);
        let y_scaled = (raw_y >> 1) as u8;
        let lcd_x = 128u8.saturating_sub(y_scaled).min(127);

        Some(TouchPoint {
            x: lcd_x,
            y: lcd_y,
            raw_x,
            raw_y,
            event,
        })
    } else {
        None
    };

    TouchSample { gesture, point }
}

/// Initialize FT6236 touch controller.
pub fn init() {
    if let Some(ref mut d) = crate::hal::get_touch_driver() {
        d.init();
    }
}

/// Hard reset FT6236 controller by toggling reset line.
pub fn reset_touch_controller() {
    init();
}

/// Check if the touch interrupt pin (PC12) is asserted (Active LOW).
#[inline(always)]
pub fn is_touch_int_asserted() -> bool {
    if let Some(ref mut d) = crate::hal::get_touch_driver() {
        d.is_touch_int_asserted()
    } else {
        false
    }
}

/// Poll the FT6236 touch controller.
pub fn read_touch() -> Option<TouchSample> {
    #[cfg(test)]
    {
        None
    }
    #[cfg(not(test))]
    if let Some(ref mut d) = crate::hal::get_touch_driver() {
        if let Some((raw_x, raw_y)) = d.read_touch() {
            let mut buf = [0u8; 7];
            buf[2] = 1;
            buf[3] = ((raw_x >> 8) & 0x0F) as u8;
            buf[4] = (raw_x & 0xFF) as u8;
            buf[5] = ((raw_y >> 8) & 0x0F) as u8;
            buf[6] = (raw_y & 0xFF) as u8;
            Some(parse_packet(&buf))
        } else {
            None
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gestures() {
        let mut buf = [0u8; 7];

        buf[1] = 0x10; // Swipe UP
        assert_eq!(parse_packet(&buf).gesture, Gesture::SwipeUp);

        buf[1] = 0x14; // Swipe RIGHT
        assert_eq!(parse_packet(&buf).gesture, Gesture::SwipeRight);

        buf[1] = 0x18; // Swipe DOWN
        assert_eq!(parse_packet(&buf).gesture, Gesture::SwipeDown);

        buf[1] = 0x1C; // Swipe LEFT
        assert_eq!(parse_packet(&buf).gesture, Gesture::SwipeLeft);

        buf[1] = 0x00; // None
        assert_eq!(parse_packet(&buf).gesture, Gesture::None);
    }

    #[test]
    fn test_parse_coordinates_and_axis_swap() {
        let mut buf = [0u8; 7];
        buf[2] = 1; // 1 touch point

        // raw_x = 40 (buf[3] = 0, buf[4] = 40)
        // raw_y = 100 (buf[5] = 0, buf[6] = 100)
        buf[3] = 0x00; // PressDown (bits 7:6 = 00), xhi = 0
        buf[4] = 40;
        buf[5] = 0x00;
        buf[6] = 100;

        let sample = parse_packet(&buf);
        let pt = sample.point.expect("Expected point");

        assert_eq!(pt.raw_x, 40);
        assert_eq!(pt.raw_y, 100);
        assert_eq!(pt.y, 20); // 40 >> 1
        assert_eq!(pt.x, 128 - 50); // 128 - (100 >> 1) = 78
        assert_eq!(pt.event, TouchEventKind::PressDown);
    }

    #[test]
    fn test_coordinate_clamping() {
        let mut buf = [0u8; 7];
        buf[2] = 1;
        buf[3] = 0x8F; // Contact (bits 7:6 = 10), xhi = 0x0F
        buf[4] = 0xFF; // raw_x = 4095
        buf[5] = 0x0F;
        buf[6] = 0xFF; // raw_y = 4095

        let sample = parse_packet(&buf);
        let pt = sample.point.expect("Expected point");
        assert_eq!(pt.y, 63); // Clamped to LCD height
        assert_eq!(pt.x, 0); // Saturated to 0
        assert_eq!(pt.event, TouchEventKind::Contact);
    }
}
