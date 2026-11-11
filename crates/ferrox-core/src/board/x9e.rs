//! FrSky Taranis X9E tray radio board specification (212x64 wide monochrome LCD).

use crate::storage::AdcInputMode;
use super::qx7_family::{impl_qx7_board_profile, Qx7FamilyProfile};
use super::*;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct X9eSpec;

impl Qx7FamilyProfile for X9eSpec {
    const MODEL_NAME: &'static str = "TARANIS-X9E";
    const FIRMWARE_ID: &'static str = "ferrox-frsky-x9e";
    const USB_PRODUCT_JOYSTICK: &'static str = "X9E Joystick";
    const USB_SERIAL_JOYSTICK: &'static str = "X9E-SIM";
    const USB_PRODUCT_SERIAL: &'static str = "X9E Serial";
    const USB_SERIAL_SERIAL: &'static str = "X9E-VCP";
    const USB_PRODUCT_COMPOSITE: &'static str = "X9E Radio";
    const USB_SERIAL_COMPOSITE: &'static str = "X9E-COMP";

    const CLI_PROMPT: &'static [u8] = b"x9e> ";
    const CLI_PAUSE_PROMPT: &'static [u8] = b"\r\n[Stream paused]\r\nx9e> ";
    const STATUS_BANNER: &'static [u8] = concat!(
        "FrSky Taranis X9E Ferrox Firmware ",
        env!("FIRMWARE_VERSION"),
        " (",
        env!("GIT_HASH"),
        ")\r\n"
    ).as_bytes();

    const DISPLAY_WIDTH: usize = 212;
    const DISPLAY_HEIGHT: usize = 64;

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
    const ADAPTIVE_CH_NAMES: &'static [&'static str] = &[
        "A", "B", "C", "D", "1", "2", "E", "F", "G", "H",
    ];
    const AUX_SOURCES: &'static [u8] = &[0, 1, 2, 3, 4, 5, 31, 32, 33, 34];
}

impl_qx7_board_profile!(X9eSpec);
