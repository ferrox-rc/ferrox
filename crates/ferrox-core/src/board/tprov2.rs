//! Jumper T-Pro V2 board specification.

use crate::storage::AdcInputMode;
use super::qx7_family::{impl_qx7_board_profile, Qx7FamilyProfile};
use super::*;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct TProV2Spec;

impl Qx7FamilyProfile for TProV2Spec {
    const MODEL_NAME: &'static str = "Jumper-TProV2";
    const FIRMWARE_ID: &'static str = "ferrox-jumper-tprov2";
    const USB_PRODUCT_JOYSTICK: &'static str = "T-Pro Joystick";
    const USB_SERIAL_JOYSTICK: &'static str = "TPro-SIM";
    const USB_PRODUCT_SERIAL: &'static str = "T-Pro Serial";
    const USB_SERIAL_SERIAL: &'static str = "TPro-VCP";
    const USB_PRODUCT_COMPOSITE: &'static str = "T-Pro Radio";
    const USB_SERIAL_COMPOSITE: &'static str = "TPro-COMP";

    const CLI_PROMPT: &'static [u8] = b"tpro> ";
    const CLI_PAUSE_PROMPT: &'static [u8] = b"\r\n[Stream paused]\r\ntpro> ";
    const STATUS_BANNER: &'static [u8] = concat!(
        "Jumper T-Pro V2 Ferrox Firmware ",
        env!("FIRMWARE_VERSION"),
        " (",
        env!("GIT_HASH"),
        ")\r\n"
    ).as_bytes();

    const HAS_S2_POT: bool = true;
    const DEFAULT_SA_MODE: AdcInputMode = AdcInputMode::ThreePos;
    const DEFAULT_SB_MODE: AdcInputMode = AdcInputMode::ThreePos;
    const DEFAULT_SC_MODE: AdcInputMode = AdcInputMode::TwoPos;
    const DEFAULT_SD_MODE: AdcInputMode = AdcInputMode::TwoPos;

    const HAS_SE: bool = true;
    const HAS_SF: bool = true;
    const HAS_SG: bool = true;
    const HAS_SH: bool = true;

    const DIAG_SW_TITLE: &'static str = "SWITCHES (SA-SH)";
    const AUX_NAMES: &'static [&'static str] = &["SA:", "SB:", "SC:", "SD:", "S1:", "S2:", "SE:", "SF:", "SG:", "SH:"];
    const ADAPTIVE_CH_NAMES: &'static [&'static str] = &["A", "B", "C", "D", "1", "2", "E", "F", "G", "H"];
    const AUX_SOURCES: &'static [u8] = &[0, 1, 2, 3, 4, 5, 31, 32, 33, 34];
}

impl_qx7_board_profile!(TProV2Spec);

