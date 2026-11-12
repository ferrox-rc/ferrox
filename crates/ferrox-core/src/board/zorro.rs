//! RadioMaster Zorro board specification.

use crate::storage::AdcInputMode;
use super::qx7_family::{impl_qx7_board_profile, Qx7FamilyProfile};
use super::*;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct ZorroSpec;

impl Qx7FamilyProfile for ZorroSpec {
    const MODEL_NAME: &'static str = "RM-Zorro";
    const FIRMWARE_ID: &'static str = "ferrox-rm-zorro";
    const USB_PRODUCT_JOYSTICK: &'static str = "Zorro Joystick";
    const USB_SERIAL_JOYSTICK: &'static str = "Zorro-SIM";
    const USB_PRODUCT_SERIAL: &'static str = "Zorro Serial";
    const USB_SERIAL_SERIAL: &'static str = "Zorro-VCP";
    const USB_PRODUCT_COMPOSITE: &'static str = "Zorro Radio";
    const USB_SERIAL_COMPOSITE: &'static str = "Zorro-COMP";

    const CLI_PROMPT: &'static [u8] = b"zorro> ";
    const CLI_PAUSE_PROMPT: &'static [u8] = b"\r\n[Stream paused]\r\nzorro> ";
    const STATUS_BANNER: &'static [u8] = concat!(
        "RadioMaster Zorro Ferrox Firmware ",
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
    const HAS_SG: bool = true;
    const HAS_SH: bool = true;

    const DIAG_SW_TITLE: &'static str = "SWITCHES (SA-SH)";
    const AUX_NAMES: &'static [&'static str] = &["SA:", "SB:", "SC:", "SD:", "S1:", "S2:", "SE:", "SF:", "SG:", "SH:"];
    const ADAPTIVE_CH_NAMES: &'static [&'static str] = &["A", "B", "C", "D", "1", "2", "E", "F", "G", "H"];
    const AUX_SOURCES: &'static [u8] = &[0, 7, 8, 9, 10, 5, 6, 31, 32, 33, 34];
}

impl_qx7_board_profile!(ZorroSpec);

