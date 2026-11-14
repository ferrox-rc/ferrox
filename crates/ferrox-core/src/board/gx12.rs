//! RadioMaster GX12 board specification.

use super::qx7_family::{impl_qx7_board_profile, Qx7FamilyProfile};
use super::*;
use crate::storage::AdcInputMode;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Gx12Spec;

impl Qx7FamilyProfile for Gx12Spec {
    const MODEL_NAME: &'static str = "RM-GX12";
    const FIRMWARE_ID: &'static str = "ferrox-rm-gx12";
    const USB_PRODUCT_JOYSTICK: &'static str = "GX12 Joystick";
    const USB_SERIAL_JOYSTICK: &'static str = "GX12-SIM";
    const USB_PRODUCT_SERIAL: &'static str = "GX12 Serial";
    const USB_SERIAL_SERIAL: &'static str = "GX12-VCP";
    const USB_PRODUCT_COMPOSITE: &'static str = "GX12 Radio";
    const USB_SERIAL_COMPOSITE: &'static str = "GX12-COMP";

    const CLI_PROMPT: &'static [u8] = b"gx12> ";
    const CLI_PAUSE_PROMPT: &'static [u8] = b"\r\n[Stream paused]\r\ngx12> ";
    const STATUS_BANNER: &'static [u8] = concat!(
        "RadioMaster GX12 Ferrox Firmware ",
        env!("FIRMWARE_VERSION"),
        " (",
        env!("GIT_HASH"),
        ")\r\n"
    )
    .as_bytes();

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
    const AUX_NAMES: &'static [&'static str] = &[
        "SA:", "SB:", "SC:", "SD:", "S1:", "S2:", "SE:", "SF:", "SG:", "SH:",
    ];
    const ADAPTIVE_CH_NAMES: &'static [&'static str] =
        &["A", "B", "C", "D", "1", "2", "E", "F", "G", "H"];
    const AUX_SOURCES: &'static [u8] = &[0, 7, 8, 9, 10, 5, 6, 31, 32, 33, 34];
}

impl_qx7_board_profile!(Gx12Spec);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gx12_spec_properties() {
        assert_eq!(<Gx12Spec as BoardProfile>::MODEL_NAME, "RM-GX12");
        assert_eq!(<Gx12Spec as BoardProfile>::FIRMWARE_ID, "ferrox-rm-gx12");
        assert!(<Gx12Spec as Qx7FamilyProfile>::HAS_S2_POT);
        assert!(<Gx12Spec as Qx7FamilyProfile>::HAS_SE);
        assert!(<Gx12Spec as Qx7FamilyProfile>::HAS_SF);
        assert!(<Gx12Spec as Qx7FamilyProfile>::HAS_SG);
        assert!(<Gx12Spec as Qx7FamilyProfile>::HAS_SH);
        assert_eq!(<Gx12Spec as BoardProfile>::AUX_NAMES.len(), 10);
        assert_eq!(<Gx12Spec as BoardProfile>::AUX_SOURCES.len(), 11);
        assert_eq!(<Gx12Spec as BoardProfile>::DISPLAY_WIDTH, 128);
        assert_eq!(<Gx12Spec as BoardProfile>::DISPLAY_HEIGHT, 64);
        assert!(<Gx12Spec as BoardProfile>::HAS_ROTARY_ENCODER);
        assert!(!<Gx12Spec as BoardProfile>::HAS_TOUCH);
    }
}
