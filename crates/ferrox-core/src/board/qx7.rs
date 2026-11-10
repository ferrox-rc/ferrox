//! FrSky Taranis Q X7 / X7S ACCESS board specification.

use crate::storage::AdcInputMode;
use super::qx7_family::{impl_qx7_board_profile, Qx7FamilyProfile};
use super::*;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Qx7Spec;

impl Qx7FamilyProfile for Qx7Spec {
    const MODEL_NAME: &'static str = "FrSky-QX7";
    const FIRMWARE_ID: &'static str = "ferrox-frsky-qx7";
    const USB_PRODUCT_JOYSTICK: &'static str = "QX7 Joystick";
    const USB_SERIAL_JOYSTICK: &'static str = "QX7-SIM";
    const USB_PRODUCT_SERIAL: &'static str = "QX7 Serial";
    const USB_SERIAL_SERIAL: &'static str = "QX7-VCP";
    const USB_PRODUCT_COMPOSITE: &'static str = "QX7 Radio";
    const USB_SERIAL_COMPOSITE: &'static str = "QX7-COMP";

    const CLI_PROMPT: &'static [u8] = b"qx7> ";
    const CLI_PAUSE_PROMPT: &'static [u8] = b"\r\n[Stream paused]\r\nqx7> ";
    const STATUS_BANNER: &'static [u8] = concat!(
        "FrSky Taranis Q X7 Ferrox Firmware ",
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

    const HAS_SE: bool = false;
    const HAS_SF: bool = true;
    const HAS_SG: bool = false;
    const HAS_SH: bool = true;

    const DIAG_SW_TITLE: &'static str = "SWITCHES (SA-SH)";
    const AUX_NAMES: &'static [&'static str] = &["SA:", "SB:", "SC:", "SD:", "S1:", "S2:", "SF:", "SH:"];
    const ADAPTIVE_CH_NAMES: &'static [&'static str] = &["A", "B", "C", "D", "1", "2", "F", "H"];
    const AUX_SOURCES: &'static [u8] = &[0, 1, 2, 3, 4, 5, 32, 34];
}

impl_qx7_board_profile!(Qx7Spec);

