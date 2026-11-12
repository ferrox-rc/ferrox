//! Shared profile trait, idle hook, and board implementation macro for Q X7 derivatives.

#![allow(unused_imports)]

use super::{
    draw_switch_slot_i6x, BoardIdleHook, BoardProfile, DiagPage, HardwareExtItem, MainMenuItem,
    RadioSetupItem, RfProtocol,
};
use crate::buzzer::Buzzer;
use crate::display::St7567;
use crate::input::{InputState, SwitchPos};
use crate::storage::{AdcInputMode, ModelConfig, RadioConfig, RadioStorage};

/// Shared background idle state for Q X7 family targets.
/// Evaluates soft-power latch (`PD0`/`PD1`) shutdown timer and renders shutdown overlay.
pub struct Qx7IdleState {
    pub power_manager: crate::power::PowerManager,
    latched_keys: u16,
}

impl Default for Qx7IdleState {
    fn default() -> Self {
        Self::new()
    }
}

impl Qx7IdleState {
    pub const fn new() -> Self {
        Self {
            power_manager: crate::power::PowerManager::new(),
            latched_keys: 0,
        }
    }
}

impl BoardIdleHook for Qx7IdleState {
    fn update(
        &mut self,
        _now: u32,
        dt_ms: u16,
        keys: u16,
        _menu_active: bool,
        storage: &mut RadioStorage,
        buzzer: &mut Buzzer,
    ) -> (bool, bool, bool) {
        self.power_manager.update(dt_ms, storage, buzzer);
        self.latched_keys |= keys;
        (false, false, false)
    }

    #[inline]
    fn clear_menu_open_state(&mut self) {}

    #[inline]
    fn take_frame_keys(&mut self, current_keys: u16, _menu_or_calib_active: bool) -> u16 {
        let frame_keys = self.latched_keys | current_keys;
        self.latched_keys = 0;
        frame_keys
    }

    fn render_overlay(&self, lcd: &mut St7567) {
        let progress_pct = self.power_manager.shutdown_progress_pct();
        if progress_pct > 0 {
            lcd.fill_rect(18, 19, 92, 26, false);
            lcd.draw_rect(18, 19, 92, 26, true);
            lcd.draw_str_6x10(28, 21, "SHUTDOWN...", false);
            lcd.draw_rect(27, 33, 74, 6, true);
            let fill_w = ((progress_pct as u32 * 70) / 100).min(70);
            if fill_w > 0 {
                lcd.fill_rect(29, 35, fill_w, 2, true);
            }
        }
    }
}

/// Declarative profile trait for radios derived from the FrSky Taranis Q X7 platform.
pub trait Qx7FamilyProfile: Copy + Clone + 'static {
    const MODEL_NAME: &'static str;
    const FIRMWARE_ID: &'static str;
    const USB_PRODUCT_JOYSTICK: &'static str;
    const USB_SERIAL_JOYSTICK: &'static str;
    const USB_PRODUCT_SERIAL: &'static str;
    const USB_SERIAL_SERIAL: &'static str;
    const USB_PRODUCT_COMPOSITE: &'static str;
    const USB_SERIAL_COMPOSITE: &'static str;

    const CLI_PROMPT: &'static [u8];
    const CLI_PAUSE_PROMPT: &'static [u8];
    const STATUS_BANNER: &'static [u8];

    const HAS_S2_POT: bool;
    const DEFAULT_SA_MODE: AdcInputMode;
    const DEFAULT_SB_MODE: AdcInputMode;
    const DEFAULT_SC_MODE: AdcInputMode;
    const DEFAULT_SD_MODE: AdcInputMode;

    const HAS_SE: bool;
    const HAS_SF: bool;
    const HAS_SG: bool;
    const HAS_SH: bool;

    const DIAG_SW_TITLE: &'static str;
    const AUX_NAMES: &'static [&'static str];
    const ADAPTIVE_CH_NAMES: &'static [&'static str];
    const AUX_SOURCES: &'static [u8];

    const DISPLAY_WIDTH: usize = 128;
    const DISPLAY_HEIGHT: usize = 64;
}

macro_rules! impl_qx7_board_profile {
    ($spec:ident) => {
        impl BoardProfile for $spec {
            const MODEL_NAME: &'static str = <$spec as Qx7FamilyProfile>::MODEL_NAME;
            const FIRMWARE_ID: &'static str = <$spec as Qx7FamilyProfile>::FIRMWARE_ID;
            const USB_PRODUCT_JOYSTICK: &'static str =
                <$spec as Qx7FamilyProfile>::USB_PRODUCT_JOYSTICK;
            const USB_SERIAL_JOYSTICK: &'static str =
                <$spec as Qx7FamilyProfile>::USB_SERIAL_JOYSTICK;
            const USB_PRODUCT_SERIAL: &'static str =
                <$spec as Qx7FamilyProfile>::USB_PRODUCT_SERIAL;
            const USB_SERIAL_SERIAL: &'static str = <$spec as Qx7FamilyProfile>::USB_SERIAL_SERIAL;
            const USB_PRODUCT_COMPOSITE: &'static str =
                <$spec as Qx7FamilyProfile>::USB_PRODUCT_COMPOSITE;
            const USB_SERIAL_COMPOSITE: &'static str =
                <$spec as Qx7FamilyProfile>::USB_SERIAL_COMPOSITE;

            const CLI_PROMPT: &'static [u8] = <$spec as Qx7FamilyProfile>::CLI_PROMPT;
            const CLI_PAUSE_PROMPT: &'static [u8] = <$spec as Qx7FamilyProfile>::CLI_PAUSE_PROMPT;
            const STATUS_BANNER: &'static [u8] = <$spec as Qx7FamilyProfile>::STATUS_BANNER;
            const CRSF_UART_STATUS: &'static [u8] =
                b"Protocol: CRSF / ExpressLRS (Internal UART active)\r\n";

            const DISPLAY_WIDTH: usize = <$spec as Qx7FamilyProfile>::DISPLAY_WIDTH;
            const DISPLAY_HEIGHT: usize = <$spec as Qx7FamilyProfile>::DISPLAY_HEIGHT;

            const EXT_ADC_COUNT: usize = 0;
            const DEFAULT_SB_MODE: AdcInputMode = <$spec as Qx7FamilyProfile>::DEFAULT_SB_MODE;
            const MAX_MIX_SRC: u8 = 45;
            const MAX_MIX_SWITCH: u8 = 22;
            const MAX_AUX_SRC: u8 = 14;

            const HAS_JOG_WHEELS: bool = false;
            const HAS_ROTARY_ENCODER: bool = true;
            const HAS_TOUCH: bool = false;
            const HAS_RGB_BACKLIGHT: bool = true;
            const HAS_REAR_AND_J4_SWITCHES: bool = false;
            const HAS_SOFT_POWER_LATCH: bool = true;

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

            const HARDWARE_EXT_ITEMS: &'static [HardwareExtItem] = &[HardwareExtItem::ExtModulePwr];

            const DIAG_PAGES: &'static [DiagPage] = &[
                DiagPage::Analog1To6,
                DiagPage::Analog7To11,
                DiagPage::ExtSwitches,
                DiagPage::EepromStorage,
            ];

            const EXT_PWR_LABEL: &'static str = "Ext Mod Pwr:";
            const EXT_ADC_LABEL: &'static str = "Ext ADC:";
            const EXT_ADC_ON_STR: &'static str = "ON";
            const EXT_ADC_OFF_STR: &'static str = "OFF";
            const PPM_OUT_ON_STR: &'static str = "PPM (ON)";
            const PPM_IN_ON_STR: &'static str = "PPM (ON)";

            const DIAG_SW_TITLE: &'static str = <$spec as Qx7FamilyProfile>::DIAG_SW_TITLE;
            const DIAG_EXT_ADC_TITLE: &'static str = "EXT ANALOG";
            const DIAG_EXT_ADC_EN_HDR: &'static str = "EXT ADC: ENABLED";
            const DIAG_EXT_ADC_DIS_HDR: &'static str = "EXT ADC: DISABLED";
            const DIAG_EXT_ADC_NAMES: &'static [&'static str] = &[];

            const AUX_NAMES: &'static [&'static str] = <$spec as Qx7FamilyProfile>::AUX_NAMES;
            const ADAPTIVE_CH_NAMES: &'static [&'static str] =
                <$spec as Qx7FamilyProfile>::ADAPTIVE_CH_NAMES;
            const AUX_SOURCES: &'static [u8] = <$spec as Qx7FamilyProfile>::AUX_SOURCES;

            const SUPPORTED_PROTOCOLS: &'static [RfProtocol] = &[
                #[cfg(feature = "rf-crsf")]
                RfProtocol::Crsf,
                #[cfg(feature = "rf-cc2500")]
                RfProtocol::Cc2500,
                #[cfg(feature = "rf-dsm")]
                RfProtocol::Dsm,
            ];
            const DEFAULT_PROTOCOL: RfProtocol = RfProtocol::Crsf;
            const HAS_INTERNAL_AFHDS2A: bool = false;
            const HAS_INTERNAL_CRSF: bool = true;

            #[inline]
            fn eeprom_crc16(data: &[u8]) -> u16 {
                crate::storage::crc16_ccitt_false(data)
            }

            #[inline]
            fn calculate_battery_mv(raw: u16) -> u16 {
                (raw as u32 * 9900 / 4095) as u16
            }

            #[inline]
            fn set_j4_mode_enabled(_enabled: bool) {}

            fn is_source_available(radio: &RadioConfig, model: &ModelConfig, src: u8) -> bool {
                match src {
                    0..=4 => true,                       // None, Roll, Pitch, Thr, Yaw
                    5 => radio.get_adc_mode(4).is_pot(), // S1
                    6 => <$spec as Qx7FamilyProfile>::HAS_S2_POT && radio.get_adc_mode(5).is_pot(), // S2
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
                    11 => true,      // MAX constant
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
                        model
                            .mixes
                            .iter()
                            .any(|m| m.target_ch == (ch_idx + 1) as u8)
                    }
                    30 => true, // Thr+
                    31 => <$spec as Qx7FamilyProfile>::HAS_SE,
                    32 => <$spec as Qx7FamilyProfile>::HAS_SF,
                    33 => <$spec as Qx7FamilyProfile>::HAS_SG,
                    34 => <$spec as Qx7FamilyProfile>::HAS_SH,
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
                    11 | 12 => <$spec as Qx7FamilyProfile>::HAS_SE, // SE^, SEv
                    13 | 14 => <$spec as Qx7FamilyProfile>::HAS_SF, // SF^, SFv
                    15 | 16 => <$spec as Qx7FamilyProfile>::HAS_SG, // SG^, SGv
                    17 | 18 => <$spec as Qx7FamilyProfile>::HAS_SH, // SH^, SHv
                    _ => false,
                }
            }

            #[inline]
            fn pre_init() {
                crate::power::init();
            }

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
                state: &mut InputState,
                _storage: &RadioStorage,
                _menu_active: bool,
            ) {
                let (se_raw, sf_raw, sg_raw, sh_raw) = crate::boot::read_ext_switches();
                if <$spec as Qx7FamilyProfile>::HAS_SE {
                    state.switches.se = se_raw;
                }
                if <$spec as Qx7FamilyProfile>::HAS_SF {
                    state.switches.sf = sf_raw;
                }
                if <$spec as Qx7FamilyProfile>::HAS_SG {
                    state.switches.sg = sg_raw;
                }
                if <$spec as Qx7FamilyProfile>::HAS_SH {
                    state.switches.sh = sh_raw;
                }
            }

            #[inline]
            fn decode_board_switches(
                _raw: &[u16; crate::adc::NUM_CHANNELS],
                _ext_switches: bool,
                _ext_adc: bool,
                _adc_modes: &[u8],
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
                let (se_raw, sf_raw, sg_raw, sh_raw) = crate::boot::read_ext_switches();
                let se = if <$spec as Qx7FamilyProfile>::HAS_SE {
                    se_raw
                } else {
                    SwitchPos::Up
                };
                let sf = if <$spec as Qx7FamilyProfile>::HAS_SF {
                    sf_raw
                } else {
                    SwitchPos::Up
                };
                let sg = if <$spec as Qx7FamilyProfile>::HAS_SG {
                    sg_raw
                } else {
                    SwitchPos::Up
                };
                let sh = if <$spec as Qx7FamilyProfile>::HAS_SH {
                    sh_raw
                } else {
                    SwitchPos::Up
                };
                (
                    se,
                    sf,
                    sg,
                    sh,
                    SwitchPos::Up,
                    SwitchPos::Up,
                    SwitchPos::Up,
                    SwitchPos::Up,
                    SwitchPos::Up,
                )
            }

            fn render_default_layout(
                lcd: &mut St7567,
                state: &InputState,
                _storage: &RadioStorage,
            ) {
                draw_switch_slot_i6x(lcd, 2, 9, "A", state.switches.sa);
                draw_switch_slot_i6x(lcd, 20, 27, "B", state.switches.sb);
                draw_switch_slot_i6x(lcd, 38, 45, "C", state.switches.sc);
                draw_switch_slot_i6x(lcd, 56, 63, "D", state.switches.sd);
                lcd.draw_str_4x6(74, 47, "VR", false);
                crate::ui::widgets::draw_split_pot_bar(
                    lcd,
                    84,
                    46,
                    42,
                    state.pots.vr1,
                    state.pots.vr2,
                );
            }

            fn append_extra_switches(
                state: &InputState,
                _storage: &RadioStorage,
                sw_list: &mut [(&'static str, SwitchPos, bool, u8); 16],
                sw_count: &mut usize,
            ) {
                if <$spec as Qx7FamilyProfile>::HAS_SE && *sw_count < sw_list.len() {
                    sw_list[*sw_count] = ("E", state.switches.se, false, 0);
                    *sw_count += 1;
                }
                if <$spec as Qx7FamilyProfile>::HAS_SF && *sw_count < sw_list.len() {
                    sw_list[*sw_count] = ("F", state.switches.sf, false, 0);
                    *sw_count += 1;
                }
                if <$spec as Qx7FamilyProfile>::HAS_SG && *sw_count < sw_list.len() {
                    sw_list[*sw_count] = ("G", state.switches.sg, false, 0);
                    *sw_count += 1;
                }
                if <$spec as Qx7FamilyProfile>::HAS_SH && *sw_count < sw_list.len() {
                    sw_list[*sw_count] = ("H", state.switches.sh, false, 0);
                    *sw_count += 1;
                }
            }

            type IdleState = Qx7IdleState;

            #[inline]
            fn create_idle_state() -> Self::IdleState {
                Qx7IdleState::new()
            }
        }
    };
}

pub(crate) use impl_qx7_board_profile;
