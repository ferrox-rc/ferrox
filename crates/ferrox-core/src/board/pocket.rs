//! RadioMaster Pocket board specification.

use crate::storage::AdcInputMode;
use super::qx7_family::{impl_qx7_board_profile, Qx7FamilyProfile};
use super::*;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct PocketSpec;

impl Qx7FamilyProfile for PocketSpec {
    const MODEL_NAME: &'static str = "RM-Pocket";
    const FIRMWARE_ID: &'static str = "ferrox-rm-pocket";
    const USB_PRODUCT_JOYSTICK: &'static str = "Pocket Joystick";
    const USB_SERIAL_JOYSTICK: &'static str = "Pocket-SIM";
    const USB_PRODUCT_SERIAL: &'static str = "Pocket Serial";
    const USB_SERIAL_SERIAL: &'static str = "Pocket-VCP";
    const USB_PRODUCT_COMPOSITE: &'static str = "Pocket Radio";
    const USB_SERIAL_COMPOSITE: &'static str = "Pocket-COMP";

    const CLI_PROMPT: &'static [u8] = b"pocket> ";
    const CLI_PAUSE_PROMPT: &'static [u8] = b"\r\n[Stream paused]\r\npocket> ";
    const STATUS_BANNER: &'static [u8] = concat!(
        "RadioMaster Pocket Ferrox Firmware ",
        env!("FIRMWARE_VERSION"),
        " (",
        env!("GIT_HASH"),
        ")\r\n"
    ).as_bytes();

    const HAS_S2_POT: bool = false;
    const DEFAULT_SA_MODE: AdcInputMode = AdcInputMode::TwoPos;
    const DEFAULT_SB_MODE: AdcInputMode = AdcInputMode::ThreePos;
    const DEFAULT_SC_MODE: AdcInputMode = AdcInputMode::ThreePos;
    const DEFAULT_SD_MODE: AdcInputMode = AdcInputMode::TwoPos;

    const HAS_SE: bool = true;
    const HAS_SF: bool = false;
    const HAS_SG: bool = false;
    const HAS_SH: bool = false;

    const DIAG_SW_TITLE: &'static str = "SWITCHES (SA-SE)";
    const AUX_NAMES: &'static [&'static str] = &["SA:", "SB:", "SC:", "SD:", "S1:", "SE:"];
    const ADAPTIVE_CH_NAMES: &'static [&'static str] = &["A", "B", "C", "D", "1", "E"];
    const AUX_SOURCES: &'static [u8] = &[0, 7, 8, 9, 10, 5, 31];
}

impl_qx7_board_profile!(PocketSpec);

