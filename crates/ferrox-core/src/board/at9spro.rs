//! RadioLink AT9S Pro board specification (320x240 Color TFT).

use super::qx7_family::{impl_qx7_board_profile, Qx7FamilyProfile};
use super::*;
use crate::storage::AdcInputMode;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct At9sProSpec;

impl Qx7FamilyProfile for At9sProSpec {
    const MODEL_NAME: &'static str = "RADIOLINK-AT9S";
    const FIRMWARE_ID: &'static str = "ferrox-at9spro";
    const USB_PRODUCT_JOYSTICK: &'static str = "AT9S Joystick";
    const USB_SERIAL_JOYSTICK: &'static str = "AT9S-SIM";
    const USB_PRODUCT_SERIAL: &'static str = "AT9S Serial";
    const USB_SERIAL_SERIAL: &'static str = "AT9S-VCP";
    const USB_PRODUCT_COMPOSITE: &'static str = "AT9S Radio";
    const USB_SERIAL_COMPOSITE: &'static str = "AT9S-COMP";

    const CLI_PROMPT: &'static [u8] = b"at9spro> ";
    const CLI_PAUSE_PROMPT: &'static [u8] = b"\r\n[Stream paused]\r\nat9spro> ";
    const STATUS_BANNER: &'static [u8] = concat!(
        "RadioLink AT9S Pro Ferrox Firmware ",
        env!("FIRMWARE_VERSION"),
        " (",
        env!("GIT_HASH"),
        ")\r\n"
    )
    .as_bytes();

    const DISPLAY_WIDTH: usize = 320;
    const DISPLAY_HEIGHT: usize = 240;

    const HAS_S2_POT: bool = true;
    const DEFAULT_SA_MODE: AdcInputMode = AdcInputMode::ThreePos;
    const DEFAULT_SB_MODE: AdcInputMode = AdcInputMode::ThreePos;
    const DEFAULT_SC_MODE: AdcInputMode = AdcInputMode::ThreePos;
    const DEFAULT_SD_MODE: AdcInputMode = AdcInputMode::ThreePos;

    const HAS_SE: bool = true;
    const HAS_SF: bool = true;
    const HAS_SG: bool = true;
    const HAS_SH: bool = true;

    const DIAG_SW_TITLE: &'static str = "SWITCHES (SA-SH)";
    const AUX_NAMES: &'static [&'static str] = &[
        "SA:", "SB:", "SC:", "SD:", "S1:", "S2:", "SE:", "SF:", "SG:", "SH:",
    ];
    const ADAPTIVE_CH_NAMES: &'static [&'static str] =
        &["A", "B", "C", "D", "1", "2", "E", "F", "G", "H"];
    const AUX_SOURCES: &'static [u8] = &[0, 7, 8, 9, 10, 5, 6, 31, 32, 33, 34];
}

impl_qx7_board_profile!(At9sProSpec);
