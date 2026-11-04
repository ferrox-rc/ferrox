//! Compile-time board profile and zero-cost HAL dispatch for Ferrox targets.

use crate::storage::AdcInputMode;

/// Static board capabilities, bounds, and hardware hooks.
pub trait BoardProfile: Copy + Clone + 'static {
    // Identity & USB Strings
    const MODEL_NAME: &'static str;
    const FIRMWARE_ID: &'static str;
    const USB_PRODUCT_JOYSTICK: &'static str;
    const USB_SERIAL_JOYSTICK: &'static str;
    const USB_PRODUCT_SERIAL: &'static str;
    const USB_SERIAL_SERIAL: &'static str;
    const USB_PRODUCT_COMPOSITE: &'static str;
    const USB_SERIAL_COMPOSITE: &'static str;

    // Analog & Switch Topology
    const EXT_ADC_COUNT: usize;
    const TOTAL_ADC_COUNT: usize = 6 + Self::EXT_ADC_COUNT;
    const DEFAULT_SB_MODE: AdcInputMode;
    const MAX_ADC_MODE: u8 = 7;
    const MAX_MIX_SRC: u8;
    const MAX_MIX_SWITCH: u8;
    const MAX_AUX_SRC: u8;

    // UI & Peripheral Capabilities
    const HAS_JOG_WHEELS: bool;
    const HAS_ROTARY_ENCODER: bool;
    const HAS_TOUCH: bool;
    const HAS_RGB_BACKLIGHT: bool;
    const HAS_REAR_AND_J4_SWITCHES: bool;
    const HAS_SOFT_POWER_LATCH: bool;

    // RF Capabilities
    const HAS_INTERNAL_AFHDS2A: bool;
    const HAS_INTERNAL_CRSF: bool;

    // CRC-16 Checksum for EEPROM
    fn eeprom_crc16(data: &[u8]) -> u16;
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct I6sSpec;

impl BoardProfile for I6sSpec {
    const MODEL_NAME: &'static str = "FS-i6S";
    const FIRMWARE_ID: &'static str = "ferrox-flysky-i6s";
    const USB_PRODUCT_JOYSTICK: &'static str = "FS-i6S Joystick";
    const USB_SERIAL_JOYSTICK: &'static str = "FS-I6S-SIM";
    const USB_PRODUCT_SERIAL: &'static str = "FS-i6S Serial";
    const USB_SERIAL_SERIAL: &'static str = "FS-I6S-VCP";
    const USB_PRODUCT_COMPOSITE: &'static str = "FS-i6S Radio";
    const USB_SERIAL_COMPOSITE: &'static str = "FS-I6S-COMP";

    const EXT_ADC_COUNT: usize = 5;
    const DEFAULT_SB_MODE: AdcInputMode = AdcInputMode::ThreePos;
    const MAX_MIX_SRC: u8 = 49;
    const MAX_MIX_SWITCH: u8 = 28;
    const MAX_AUX_SRC: u8 = 14;

    const HAS_JOG_WHEELS: bool = true;
    const HAS_ROTARY_ENCODER: bool = false;
    const HAS_TOUCH: bool = true;
    const HAS_RGB_BACKLIGHT: bool = true;
    const HAS_REAR_AND_J4_SWITCHES: bool = true;
    const HAS_SOFT_POWER_LATCH: bool = true;

    const HAS_INTERNAL_AFHDS2A: bool = true;
    const HAS_INTERNAL_CRSF: bool = false;

    #[inline]
    fn eeprom_crc16(data: &[u8]) -> u16 {
        crate::storage::crc16_ccitt_false(data)
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct I6xSpec;

impl BoardProfile for I6xSpec {
    const MODEL_NAME: &'static str = "FS-i6X";
    const FIRMWARE_ID: &'static str = "ferrox-flysky-i6x";
    const USB_PRODUCT_JOYSTICK: &'static str = "FS-i6X Joystick";
    const USB_SERIAL_JOYSTICK: &'static str = "FS-I6X-SIM";
    const USB_PRODUCT_SERIAL: &'static str = "FS-i6X Serial";
    const USB_SERIAL_SERIAL: &'static str = "FS-I6X-VCP";
    const USB_PRODUCT_COMPOSITE: &'static str = "FS-i6X Radio";
    const USB_SERIAL_COMPOSITE: &'static str = "FS-I6X-COMP";

    const EXT_ADC_COUNT: usize = 4;
    const DEFAULT_SB_MODE: AdcInputMode = AdcInputMode::TwoPos;
    const MAX_MIX_SRC: u8 = 45;
    const MAX_MIX_SWITCH: u8 = 22;
    const MAX_AUX_SRC: u8 = 12;

    const HAS_JOG_WHEELS: bool = false;
    const HAS_ROTARY_ENCODER: bool = false;
    const HAS_TOUCH: bool = false;
    const HAS_RGB_BACKLIGHT: bool = false;
    const HAS_REAR_AND_J4_SWITCHES: bool = false;
    const HAS_SOFT_POWER_LATCH: bool = false;

    const HAS_INTERNAL_AFHDS2A: bool = true;
    const HAS_INTERNAL_CRSF: bool = false;

    #[inline]
    fn eeprom_crc16(data: &[u8]) -> u16 {
        crate::storage::crc16_arc(data)
    }
}

/// RadioMaster Pocket specification: STM32F407, ST7567 128x64 mono, rotary encoder, native ELRS.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct PocketSpec;

impl BoardProfile for PocketSpec {
    const MODEL_NAME: &'static str = "RM-Pocket";
    const FIRMWARE_ID: &'static str = "ferrox-rm-pocket";
    const USB_PRODUCT_JOYSTICK: &'static str = "Pocket Joystick";
    const USB_SERIAL_JOYSTICK: &'static str = "Pocket-SIM";
    const USB_PRODUCT_SERIAL: &'static str = "Pocket Serial";
    const USB_SERIAL_SERIAL: &'static str = "Pocket-VCP";
    const USB_PRODUCT_COMPOSITE: &'static str = "Pocket Radio";
    const USB_SERIAL_COMPOSITE: &'static str = "Pocket-COMP";

    const EXT_ADC_COUNT: usize = 4;
    const DEFAULT_SB_MODE: AdcInputMode = AdcInputMode::ThreePos;
    const MAX_MIX_SRC: u8 = 45;
    const MAX_MIX_SWITCH: u8 = 22;
    const MAX_AUX_SRC: u8 = 12;

    const HAS_JOG_WHEELS: bool = false;
    const HAS_ROTARY_ENCODER: bool = true;
    const HAS_TOUCH: bool = false;
    const HAS_RGB_BACKLIGHT: bool = true;
    const HAS_REAR_AND_J4_SWITCHES: bool = false;
    const HAS_SOFT_POWER_LATCH: bool = true;

    const HAS_INTERNAL_AFHDS2A: bool = false;
    const HAS_INTERNAL_CRSF: bool = true;

    #[inline]
    fn eeprom_crc16(data: &[u8]) -> u16 {
        crate::storage::crc16_ccitt_false(data)
    }
}

#[cfg(feature = "board-i6s")]
pub type ActiveBoard = I6sSpec;

#[cfg(all(not(feature = "board-i6s"), not(feature = "board-pocket")))]
pub type ActiveBoard = I6xSpec;

#[cfg(feature = "board-pocket")]
pub type ActiveBoard = PocketSpec;
