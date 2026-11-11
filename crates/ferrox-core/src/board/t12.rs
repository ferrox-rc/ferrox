//! Jumper T12 board specification.

use crate::storage::AdcInputMode;
use super::qx7_family::{impl_qx7_board_profile, Qx7FamilyProfile};
use super::*;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct T12Spec;

impl Qx7FamilyProfile for T12Spec {
    const MODEL_NAME: &'static str = "JUMPER-T12";
    const FIRMWARE_ID: &'static str = "ferrox-jumper-t12";
    const USB_PRODUCT_JOYSTICK: &'static str = "T12 Joystick";
    const USB_SERIAL_JOYSTICK: &'static str = "T12-SIM";
    const USB_PRODUCT_SERIAL: &'static str = "T12 Serial";
    const USB_SERIAL_SERIAL: &'static str = "T12-VCP";
    const USB_PRODUCT_COMPOSITE: &'static str = "T12 Radio";
    const USB_SERIAL_COMPOSITE: &'static str = "T12-COMP";

    const CLI_PROMPT: &'static [u8] = b"t12> ";
    const CLI_PAUSE_PROMPT: &'static [u8] = b"\r\n[Stream paused]\r\nt12> ";
    const STATUS_BANNER: &'static [u8] = concat!(
        "Jumper T12 Ferrox Firmware ",
        env!("FIRMWARE_VERSION"),
        " (",
        env!("GIT_HASH"),
        ")\r\n"
    ).as_bytes();

    const HAS_S2_POT: bool = true;
    const DEFAULT_SA_MODE: AdcInputMode = AdcInputMode::ThreePos;
    const DEFAULT_SB_MODE: AdcInputMode = AdcInputMode::ThreePos;
    const DEFAULT_SC_MODE: AdcInputMode = AdcInputMode::ThreePos;
    const DEFAULT_SD_MODE: AdcInputMode = AdcInputMode::ThreePos;

    const HAS_SE: bool = true;
    const HAS_SF: bool = true;
    const HAS_SG: bool = false;
    const HAS_SH: bool = false;

    const DIAG_SW_TITLE: &'static str = "SWITCHES (SA-SF)";
    const AUX_NAMES: &'static [&'static str] = &["SA:", "SB:", "SC:", "SD:", "S1:", "S2:", "SE:", "SF:"];
    const ADAPTIVE_CH_NAMES: &'static [&'static str] = &["A", "B", "C", "D", "1", "2", "E", "F"];
    const AUX_SOURCES: &'static [u8] = &[0, 1, 2, 3, 4, 5, 31, 32];
}

impl_qx7_board_profile!(T12Spec);
