//! RadioMaster Boxer board specification.

use crate::storage::AdcInputMode;
use super::qx7_family::{impl_qx7_board_profile, Qx7FamilyProfile};
use super::*;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct BoxerSpec;

impl Qx7FamilyProfile for BoxerSpec {
    const MODEL_NAME: &'static str = "RM-Boxer";
    const FIRMWARE_ID: &'static str = "ferrox-rm-boxer";
    const USB_PRODUCT_JOYSTICK: &'static str = "Boxer Joystick";
    const USB_SERIAL_JOYSTICK: &'static str = "Boxer-SIM";
    const USB_PRODUCT_SERIAL: &'static str = "Boxer Serial";
    const USB_SERIAL_SERIAL: &'static str = "Boxer-VCP";
    const USB_PRODUCT_COMPOSITE: &'static str = "Boxer Radio";
    const USB_SERIAL_COMPOSITE: &'static str = "Boxer-COMP";

    const CLI_PROMPT: &'static [u8] = b"boxer> ";
    const CLI_PAUSE_PROMPT: &'static [u8] = b"\r\n[Stream paused]\r\nboxer> ";
    const STATUS_BANNER: &'static [u8] = concat!(
        "RadioMaster Boxer Ferrox Firmware ",
        env!("FIRMWARE_VERSION"),
        " (",
        env!("GIT_HASH"),
        ")\r\n"
    ).as_bytes();

    const HAS_S2_POT: bool = true;
    const DEFAULT_SA_MODE: AdcInputMode = AdcInputMode::TwoPos;
    const DEFAULT_SB_MODE: AdcInputMode = AdcInputMode::ThreePos;
    const DEFAULT_SC_MODE: AdcInputMode = AdcInputMode::ThreePos;
    const DEFAULT_SD_MODE: AdcInputMode = AdcInputMode::TwoPos;

    const HAS_SE: bool = true;
    const HAS_SF: bool = true;
    const HAS_SG: bool = false;
    const HAS_SH: bool = false;

    const DIAG_SW_TITLE: &'static str = "SWITCHES (SA-SF)";
    const AUX_NAMES: &'static [&'static str] = &["SA:", "SB:", "SC:", "SD:", "S1:", "S2:", "SE:", "SF:"];
    const ADAPTIVE_CH_NAMES: &'static [&'static str] = &["A", "B", "C", "D", "1", "2", "E", "F"];
    const AUX_SOURCES: &'static [u8] = &[0, 7, 8, 9, 10, 5, 6, 31, 32];
}

impl_qx7_board_profile!(BoxerSpec);

