//! FlySky FS-i6X board profile and keypad matrix layout.

use crate::display::St7567;
use crate::input::{InputState, SwitchPos};
use crate::storage::{AdcInputMode, ModelConfig, RadioConfig, RadioStorage};
use super::{
    draw_switch_slot_i6x, BoardProfile, DefaultIdleState, DiagPage, HardwareExtItem, MainMenuItem,
    RadioSetupItem, RfProtocol,
};

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

    const CLI_PROMPT: &'static [u8] = b"i6x> ";
    const CLI_PAUSE_PROMPT: &'static [u8] = b"\r\n[Stream paused]\r\ni6x> ";
    const STATUS_BANNER: &'static [u8] = concat!(
        "FlySky FS-i6X Rust Firmware ",
        env!("FIRMWARE_VERSION"),
        " (",
        env!("GIT_HASH"),
        ")\r\n"
    )
    .as_bytes();
    const CRSF_UART_STATUS: &'static [u8] = b"Protocol: CRSF / ExpressLRS (USART2 PD5/PA15 active)\r\n";

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

    const MAIN_MENU_ITEMS: &'static [MainMenuItem] = &[
        MainMenuItem::ModelSelect,
        MainMenuItem::ModelSetup,
        MainMenuItem::DualRateExpo,
        MainMenuItem::ThrottleCurve,
        MainMenuItem::WingMixer,
        MainMenuItem::AuxChannels,
        MainMenuItem::ChannelReverse,
        MainMenuItem::RadioSetup,
        MainMenuItem::ChannelMonitor,
        MainMenuItem::Calibration,
        MainMenuItem::DiagAnas,
        MainMenuItem::SystemInfo,
    ];

    const RADIO_SETUP_ITEMS: &'static [RadioSetupItem] = &[
        RadioSetupItem::ThrottleTrim,
        RadioSetupItem::Beeper,
        RadioSetupItem::ToneStyle,
        RadioSetupItem::BacklightTimeout,
        RadioSetupItem::BacklightBrightness,
        RadioSetupItem::LcdContrast,
        RadioSetupItem::BatteryWarn,
        RadioSetupItem::UsbMode,
        RadioSetupItem::HardwareSetupSubmenu,
        RadioSetupItem::StickMode,
        RadioSetupItem::GimbalType,
        RadioSetupItem::InputSetupSubmenu,
    ];

    const HARDWARE_EXT_ITEMS: &'static [HardwareExtItem] = &[
        HardwareExtItem::ExtModulePwr,
        HardwareExtItem::ExtSwitches,
        HardwareExtItem::ExtAdc,
        HardwareExtItem::PpmOut,
        HardwareExtItem::PpmIn,
    ];

    const DIAG_PAGES: &'static [DiagPage] = &[
        DiagPage::Analog1To6,
        DiagPage::Analog7To11,
        DiagPage::ExtSwitches,
        DiagPage::ExtAnalog,
        DiagPage::EepromStorage,
    ];

    const EXT_PWR_LABEL: &'static str = "PC13 Pwr:";
    const EXT_ADC_LABEL: &'static str = "P7 Analog:";
    const EXT_ADC_ON_STR: &'static str = "AD12-15";
    const EXT_ADC_OFF_STR: &'static str = "OFF";
    const PPM_OUT_ON_STR: &'static str = "PF10 (ON)";
    const PPM_IN_ON_STR: &'static str = "PF9 (ON)";

    const DIAG_SW_TITLE: &'static str = "EXT SWITCHES (SE/SF)";
    const DIAG_EXT_ADC_TITLE: &'static str = "EXT ANALOG (P7)";
    const DIAG_EXT_ADC_EN_HDR: &'static str = "P7 ADC: ENABLED (15-CH)";
    const DIAG_EXT_ADC_DIS_HDR: &'static str = "P7 ADC: DISABLED (OFF)";
    const DIAG_EXT_ADC_NAMES: &'static [&'static str] = &[
        "VRC:AD12", "VRD:AD13", "VRE:AD14", "VRF:AD15",
    ];

    const AUX_NAMES: &'static [&'static str] = &[
        "SA (PA4):",
        "SB (PA5):",
        "SC (PB0):",
        "SD (PB1):",
        "VRA (PA6):",
        "VRB (PA7):",
        "VRC (PC2):",
        "VRD (PC3):",
        "VRE (PC4):",
        "VRF (PC5):",
    ];

    const ADAPTIVE_CH_NAMES: &'static [&'static str] = &[
        "A", "B", "C", "D", "1", "2", "3", "4", "5", "6",
    ];

    const AUX_SOURCES: &'static [u8] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 31, 32, 35, 36, 37, 38];

    const SUPPORTED_PROTOCOLS: &'static [RfProtocol] = &[
        #[cfg(feature = "rf-afhds2a")]
        RfProtocol::Afhds2a,
        #[cfg(feature = "rf-crsf")]
        RfProtocol::Crsf,
        #[cfg(feature = "rf-cc2500")]
        RfProtocol::Cc2500,
        #[cfg(feature = "rf-dsm")]
        RfProtocol::Dsm,
    ];
    const DEFAULT_PROTOCOL: RfProtocol = RfProtocol::Afhds2a;
    const HAS_INTERNAL_AFHDS2A: bool = true;
    const HAS_INTERNAL_CRSF: bool = false;

    #[inline]
    fn eeprom_crc16(data: &[u8]) -> u16 {
        crate::storage::crc16_arc(data)
    }

    #[inline]
    fn calculate_battery_mv(raw: u16) -> u16 {
        let vbat_10mv = ((raw as u32 * 100) / 421) + 20;
        (vbat_10mv * 10) as u16
    }

    #[inline]
    fn set_j4_mode_enabled(_enabled: bool) {}

    fn is_source_available(radio: &RadioConfig, model: &ModelConfig, src: u8) -> bool {
        match src {
            0..=4 => true, // None, Roll, Pitch, Thr, Yaw
            5 => radio.get_adc_mode(4).is_pot(), // VRA
            6 => radio.get_adc_mode(5).is_pot(), // VRB
            7 => matches!(
                radio.get_adc_mode(0),
                AdcInputMode::TwoPos | AdcInputMode::ThreePos | AdcInputMode::InstantTrim
            ), // SA
            8 => matches!(
                radio.get_adc_mode(1),
                AdcInputMode::TwoPos | AdcInputMode::ThreePos | AdcInputMode::InstantTrim
            ), // SB
            9 => matches!(
                radio.get_adc_mode(2),
                AdcInputMode::TwoPos | AdcInputMode::ThreePos | AdcInputMode::InstantTrim
            ), // SC
            10 => matches!(
                radio.get_adc_mode(3),
                AdcInputMode::TwoPos | AdcInputMode::ThreePos | AdcInputMode::InstantTrim
            ), // SD
            11 => true, // MAX constant
            12..=15 => true, // CH1..CH4 primary flight channels
            16..=29 => {
                let ch_idx = (src - 12) as usize; // 4..17
                let aux_idx = ch_idx - 4; // 0..13
                if model.wing_tail_mix == 3 && ch_idx == 5 {
                    return true;
                }
                if model.aux_channels[aux_idx] != 0 {
                    return true;
                }
                model.mixes.iter().any(|m| m.target_ch == (ch_idx + 1) as u8)
            }
            30 => true, // Thr+
            31 => radio.ext_switches != 0, // SE
            32 => radio.ext_switches != 0, // SF
            33 => radio.j4_sg_sh_en != 0, // SG
            34 => radio.j4_sg_sh_en != 0, // SH
            35 => {
                radio.ext_adc != 0
                    && matches!(
                        radio.get_adc_mode(6),
                        AdcInputMode::TwoPos | AdcInputMode::ThreePos
                    )
            } // SWI
            36 => {
                radio.ext_adc != 0
                    && matches!(
                        radio.get_adc_mode(7),
                        AdcInputMode::TwoPos | AdcInputMode::ThreePos
                    )
            } // SWJ
            37 => radio.ext_adc != 0 && radio.get_adc_mode(6).is_pot(), // VRC
            38 => radio.ext_adc != 0 && radio.get_adc_mode(7).is_pot(), // VRD
            39 => radio.ext_adc != 0 && radio.get_adc_mode(8).is_pot(), // VRE
            40 => radio.ext_adc != 0 && radio.get_adc_mode(9).is_pot(), // VRF
            41 => radio.ext_adc != 0 && radio.get_adc_mode(10).is_pot(), // VRG
            42 => {
                radio.ext_adc != 0
                    && matches!(
                        radio.get_adc_mode(8),
                        AdcInputMode::TwoPos | AdcInputMode::ThreePos
                    )
            } // SWK
            43 => {
                radio.ext_adc != 0
                    && matches!(
                        radio.get_adc_mode(9),
                        AdcInputMode::TwoPos | AdcInputMode::ThreePos
                    )
            } // SWL
            44 => {
                radio.ext_adc != 0
                    && matches!(
                        radio.get_adc_mode(10),
                        AdcInputMode::TwoPos | AdcInputMode::ThreePos
                    )
            } // SWM
            45..=49 => true, // GV1..GV5
            _ => false,
        }
    }

    fn is_switch_cond_available(radio: &RadioConfig, cond: u8) -> bool {
        match cond {
            0 => true, // Always ON
            1 | 2 => matches!(
                radio.get_adc_mode(0),
                AdcInputMode::TwoPos | AdcInputMode::ThreePos | AdcInputMode::InstantTrim
            ), // SA
            3 | 5 => matches!(
                radio.get_adc_mode(1),
                AdcInputMode::TwoPos | AdcInputMode::ThreePos | AdcInputMode::InstantTrim
            ), // SB Up, Down
            4 => radio.get_adc_mode(1) == AdcInputMode::ThreePos, // SB Mid
            6 | 8 => matches!(
                radio.get_adc_mode(2),
                AdcInputMode::TwoPos | AdcInputMode::ThreePos | AdcInputMode::InstantTrim
            ), // SC Up, Down
            7 => radio.get_adc_mode(2) == AdcInputMode::ThreePos, // SC Mid
            9 | 10 => matches!(
                radio.get_adc_mode(3),
                AdcInputMode::TwoPos | AdcInputMode::ThreePos | AdcInputMode::InstantTrim
            ), // SD
            11 | 12 => radio.ext_switches != 0, // SE
            13 | 14 => radio.ext_switches != 0, // SF
            15 | 16 => radio.j4_sg_sh_en != 0, // SG
            17 | 18 => radio.j4_sg_sh_en != 0, // SH
            19 | 20 => {
                radio.ext_adc != 0
                    && matches!(
                        radio.get_adc_mode(6),
                        AdcInputMode::TwoPos | AdcInputMode::ThreePos
                    )
            } // SWI
            21 | 22 => {
                radio.ext_adc != 0
                    && matches!(
                        radio.get_adc_mode(7),
                        AdcInputMode::TwoPos | AdcInputMode::ThreePos
                    )
            } // SWJ
            23 | 24 => {
                radio.ext_adc != 255
                    && radio.ext_adc != 0
                    && matches!(
                        radio.get_adc_mode(8),
                        AdcInputMode::TwoPos | AdcInputMode::ThreePos
                    )
            } // SWK
            25 | 26 => {
                radio.ext_adc != 0
                    && matches!(
                        radio.get_adc_mode(9),
                        AdcInputMode::TwoPos | AdcInputMode::ThreePos
                    )
            } // SWL
            27 | 28 => {
                radio.ext_adc != 0
                    && matches!(
                        radio.get_adc_mode(10),
                        AdcInputMode::TwoPos | AdcInputMode::ThreePos
                    )
            } // SWM
            _ => false,
        }
    }

    #[inline]
    fn pre_init() {}

    #[inline]
    fn init_touch() {}

    #[inline]
    fn init_board_keys(_radio: &RadioConfig) {}

    #[inline]
    fn poll_keys(_storage: &RadioStorage, _menu_active: bool) -> u16 {
        crate::boot::scan_keys()
    }

    #[inline]
    fn update_flight_switches(
        _state: &mut InputState,
        _storage: &RadioStorage,
        _menu_active: bool,
    ) {
    }

    #[inline]
    fn decode_board_switches(
        raw: &[u16; crate::adc::NUM_CHANNELS],
        ext_switches: bool,
        ext_adc: bool,
        adc_modes: &[u8],
    ) -> (
        SwitchPos,
        SwitchPos,
        SwitchPos,
        SwitchPos,
        SwitchPos,
        SwitchPos,
        SwitchPos,
        SwitchPos,
        SwitchPos,
    ) {
        let (se, sf, _, _) = if ext_switches {
            crate::boot::read_ext_switches()
        } else {
            (SwitchPos::Up, SwitchPos::Up, SwitchPos::Up, SwitchPos::Up)
        };

        let (swi, swj, swk, swl) = if ext_adc {
            let swi = match AdcInputMode::resolve(6, adc_modes[6]) {
                AdcInputMode::ThreePos => crate::input::decode_switch_3pos(raw[11]),
                AdcInputMode::TwoPos => crate::input::decode_switch_2pos(raw[11]),
                _ => SwitchPos::Up,
            };
            let swj = match AdcInputMode::resolve(7, adc_modes[7]) {
                AdcInputMode::ThreePos => crate::input::decode_switch_3pos(raw[12]),
                AdcInputMode::TwoPos => crate::input::decode_switch_2pos(raw[12]),
                _ => SwitchPos::Up,
            };
            let swk = match AdcInputMode::resolve(8, adc_modes[8]) {
                AdcInputMode::ThreePos => crate::input::decode_switch_3pos(raw[13]),
                AdcInputMode::TwoPos => crate::input::decode_switch_2pos(raw[13]),
                _ => SwitchPos::Up,
            };
            let swl = match AdcInputMode::resolve(9, adc_modes[9]) {
                AdcInputMode::ThreePos => crate::input::decode_switch_3pos(raw[14]),
                AdcInputMode::TwoPos => crate::input::decode_switch_2pos(raw[14]),
                _ => SwitchPos::Up,
            };
            (swi, swj, swk, swl)
        } else {
            (SwitchPos::Up, SwitchPos::Up, SwitchPos::Up, SwitchPos::Up)
        };

        (se, sf, SwitchPos::Up, SwitchPos::Up, swi, swj, swk, swl, SwitchPos::Up)
    }

    fn render_default_layout(lcd: &mut St7567, state: &InputState, storage: &RadioStorage) {
        let ext_sw = storage.radio.ext_switches != 0;

        if ext_sw {
            draw_switch_slot_i6x(lcd, 2, 7, "A", state.switches.sa);
            draw_switch_slot_i6x(lcd, 13, 18, "B", state.switches.sb);
            draw_switch_slot_i6x(lcd, 24, 29, "C", state.switches.sc);
            draw_switch_slot_i6x(lcd, 35, 40, "D", state.switches.sd);
            draw_switch_slot_i6x(lcd, 46, 51, "E", state.switches.se);
            draw_switch_slot_i6x(lcd, 57, 62, "F", state.switches.sf);
        } else {
            draw_switch_slot_i6x(lcd, 2, 9, "A", state.switches.sa);
            draw_switch_slot_i6x(lcd, 20, 27, "B", state.switches.sb);
            draw_switch_slot_i6x(lcd, 38, 45, "C", state.switches.sc);
            draw_switch_slot_i6x(lcd, 56, 63, "D", state.switches.sd);
        }

        lcd.draw_str_4x6(74, 47, "VR", false);
        crate::ui::widgets::draw_split_pot_bar(lcd, 84, 46, 42, state.pots.vr1, state.pots.vr2);
    }

    fn append_extra_switches(
        state: &InputState,
        storage: &RadioStorage,
        sw_list: &mut [(&'static str, SwitchPos, bool, u8); 16],
        sw_count: &mut usize,
    ) {
        if storage.radio.ext_switches != 0 {
            if *sw_count < sw_list.len() {
                sw_list[*sw_count] = ("E", state.switches.se, false, 0);
                *sw_count += 1;
            }
            if *sw_count < sw_list.len() {
                sw_list[*sw_count] = ("F", state.switches.sf, false, 0);
                *sw_count += 1;
            }
        }
    }

    type IdleState = DefaultIdleState;

    #[inline]
    fn create_idle_state() -> Self::IdleState {
        DefaultIdleState
    }
}
