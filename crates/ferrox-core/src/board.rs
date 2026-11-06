//! Compile-time board profile and zero-cost HAL dispatch for Ferrox targets.

use crate::buzzer::Buzzer;
use crate::display::St7567;
use crate::input::{InputState, SwitchPos};
use crate::storage::{AdcInputMode, ModelConfig, RadioConfig, RadioStorage};
use crate::ui::glyphs;

/// Modular RF protocols supported by `ferrox-core`.
/// Stored as `u8` in `ModelConfig::rf_protocol` (0 = Afhds2a, 1 = Crsf, 2 = Cc2500, 3 = Dsm).
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RfProtocol {
    /// FlySky AFHDS 2A (A7105 SPI transceiver).
    Afhds2a = 0,
    /// ExpressLRS / TBS Crossfire (CRSF UART DMA).
    Crsf = 1,
    /// CC2500 Multi-Protocol SPI transceiver (e.g., RM Pocket CC2500).
    Cc2500 = 2,
    /// Spektrum DSM2 / DSMX (MULTI-Module 100k 8E2 serial or CYRF6936).
    Dsm = 3,
}

impl RfProtocol {
    #[inline]
    pub const fn from_u8(val: u8) -> Self {
        match val {
            0 => Self::Afhds2a,
            1 => Self::Crsf,
            2 => Self::Cc2500,
            3 => Self::Dsm,
            _ => Self::Afhds2a,
        }
    }

    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Afhds2a => "AFHDS 2A",
            Self::Crsf => "CRSF",
            Self::Cc2500 => "CC2500",
            Self::Dsm => "SPEKTRUM DSM",
        }
    }

    #[inline]
    pub const fn bracketed_name(self) -> &'static str {
        match self {
            Self::Afhds2a => "[AFHDS 2A]",
            Self::Crsf => "[CRSF]",
            Self::Cc2500 => "[CC2500]",
            Self::Dsm => "[SPEKTRUM DSM]",
        }
    }
}

/// Spektrum DSM2 / DSMX air modulation and frame rate sub-protocols.
/// Stored in `ModelConfig::dsm_sub_proto` (`0..=3`).
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DsmSubProtocol {
    /// DSMX 11ms 2048-resolution (modern 6–12ch DSMX receivers, default).
    Dsmx11 = 0,
    /// DSMX 22ms 2048-resolution.
    Dsmx22 = 1,
    /// DSM2 11ms 2048-resolution.
    Dsm2_11 = 2,
    /// DSM2 22ms 1024-resolution (legacy 6–7ch DSM2 receivers).
    Dsm2_22 = 3,
}

impl DsmSubProtocol {
    #[inline]
    pub const fn from_u8(val: u8) -> Self {
        match val {
            0 => Self::Dsmx11,
            1 => Self::Dsmx22,
            2 => Self::Dsm2_11,
            3 => Self::Dsm2_22,
            _ => Self::Dsmx11,
        }
    }

    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dsmx11 => "DSMX 11ms",
            Self::Dsmx22 => "DSMX 22ms",
            Self::Dsm2_11 => "DSM2 11ms",
            Self::Dsm2_22 => "DSM2 22ms",
        }
    }

    #[inline]
    pub const fn next(self) -> Self {
        match self {
            Self::Dsmx11 => Self::Dsmx22,
            Self::Dsmx22 => Self::Dsm2_11,
            Self::Dsm2_11 => Self::Dsm2_22,
            Self::Dsm2_22 => Self::Dsmx11,
        }
    }
}

/// Top-level Main Menu entries selectable per board via [`BoardProfile::MAIN_MENU_ITEMS`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MainMenuItem {
    ModelSelect,
    ModelSetup,
    DualRateExpo,
    ThrottleCurve,
    WingMixer,
    AuxChannels,
    ChannelReverse,
    RadioSetup,
    ChannelMonitor,
    Calibration,
    DiagAnas,
    SystemInfo,
}

impl MainMenuItem {
    #[inline]
    pub const fn title(self) -> &'static str {
        match self {
            Self::ModelSelect => "Model Select",
            Self::ModelSetup => "Model Setup",
            Self::DualRateExpo => "Dual Rate/Expo",
            Self::ThrottleCurve => "Throttle Curve",
            Self::WingMixer => "Wing / Mixer",
            Self::AuxChannels => "Aux Channels",
            Self::ChannelReverse => "Channel Reverse",
            Self::RadioSetup => "Radio Setup",
            Self::ChannelMonitor => "Channel Monitor",
            Self::Calibration => "Calibration",
            Self::DiagAnas => "Analog Diag",
            Self::SystemInfo => "System Info",
        }
    }

    #[inline]
    pub const fn glyph(self) -> &'static [u8; 18] {
        match self {
            Self::ModelSelect => &glyphs::GLYPH_AIRPLANE,
            Self::ModelSetup => &glyphs::GLYPH_AIRPLANE_COG,
            Self::DualRateExpo => &glyphs::GLYPH_CHART_LINE,
            Self::ThrottleCurve => &glyphs::GLYPH_CHART_BELL_CURVE,
            Self::WingMixer => &glyphs::GLYPH_SWAP_HORIZONTAL,
            Self::AuxChannels => &glyphs::GLYPH_TOGGLE_SWITCH,
            Self::ChannelReverse => &glyphs::GLYPH_SWAP_VERTICAL,
            Self::RadioSetup => &glyphs::GLYPH_COG,
            Self::ChannelMonitor => &glyphs::GLYPH_GAUGE,
            Self::Calibration => &glyphs::GLYPH_CROSSHAIRS_GPS,
            Self::DiagAnas => &glyphs::GLYPH_PULSE,
            Self::SystemInfo => &glyphs::GLYPH_INFORMATION_OUTLINE,
        }
    }
}

/// Declarative items for the `RADIO SETUP` screen ([`BoardProfile::RADIO_SETUP_ITEMS`]).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RadioSetupItem {
    ThrottleTrim,
    Beeper,
    ToneStyle,
    SwitchBeep,
    BacklightTimeout,
    BacklightBrightness,
    LcdContrast,
    BatteryWarn,
    UsbMode,
    StickMode,
    GimbalType,
    TouchEnabled,
    RearLeftFunc,
    RearRightFunc,
    InputSetupSubmenu,
    HardwareSetupSubmenu,
}

/// Declarative items for the `HARDWARE EXT` screen ([`BoardProfile::HARDWARE_EXT_ITEMS`]).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HardwareExtItem {
    /// External RF module power MOSFET polarity (`HIGH (N)` vs `LOW (P)`).
    ExtModulePwr,
    /// Extra digital switches (`PC12+PC15` SE/SF on FS-i6X).
    ExtSwitches,
    /// J4 header mode (`SG/SH BTN` vs `SWD (DBG)` on FS-i6S).
    J4Mode,
    /// Extra analog ADC inputs (`P7 Analog` AD12-15 on FS-i6X, `Ext ADC (PC)` PC1-5 on FS-i6S).
    ExtAdc,
    /// Trainer PPM output generator (`PF10`).
    PpmOut,
    /// Trainer input mode: `OFF` -> `PPM (PF9)` -> `DSMX SAT` (Spektrum Satellite wireless trainer).
    PpmIn,
}

/// Diagnostic screen pages selectable per board via [`BoardProfile::DIAG_PAGES`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DiagPage {
    /// Page 0: Primary sticks + SA/SB (`ANALOG (1-6)`).
    Analog1To6,
    /// Page 1: Pots V1/V2 + SC/SD + VBAT (`ANALOG (7-11)`).
    Analog7To11,
    /// Page 2: Board-specific extra digital switches (`SE/SF` on i6X, `SE/SF/SG/SH` on i6S).
    ExtSwitches,
    /// Page 3: Extended analog ADC channels (`P7` AD12-15 on i6X, `PC1-5` on i6S).
    ExtAnalog,
    /// Page 4: External I2C EEPROM bus, address, and health status.
    EepromStorage,
}

/// Static board capabilities, bounds, and hardware hooks.
pub trait BoardProfile: Copy + Clone + 'static {
    // Identity & USB Strings
    const MODEL_NAME: &'static str;
    const FIRMWARE_ID: &'static str;
    const USB_PRODUCT_JOYSTICK: &'static str;
    const USB_SERIAL_JOYSTICK: &'static str;
    const USB_PRODUCT_SERIAL: &'static str;
    const USB_SERIAL_SERIAL: &'static str;
    const USB_PRODUCT_COMPOSITE: &'static str;
    const USB_SERIAL_COMPOSITE: &'static str;

    // USB Serial Interactive CLI Byte Strings
    const CLI_PROMPT: &'static [u8];
    const CLI_PAUSE_PROMPT: &'static [u8];
    const STATUS_BANNER: &'static [u8];
    const CRSF_UART_STATUS: &'static [u8];

    // Analog & Switch Topology
    const EXT_ADC_COUNT: usize;
    const TOTAL_ADC_COUNT: usize = 6 + Self::EXT_ADC_COUNT;
    const DEFAULT_SB_MODE: AdcInputMode;
    const MAX_ADC_MODE: u8 = 7;
    const MAX_MIX_SRC: u8;
    const MAX_MIX_SWITCH: u8;
    const MAX_AUX_SRC: u8;

    // UI & Peripheral Capabilities
    const HAS_JOG_WHEELS: bool;
    const HAS_ROTARY_ENCODER: bool;
    const HAS_TOUCH: bool;
    const HAS_RGB_BACKLIGHT: bool;
    const HAS_REAR_AND_J4_SWITCHES: bool;
    const HAS_SOFT_POWER_LATCH: bool;

    // RF Protocol Capabilities
    const HAS_INTERNAL_AFHDS2A: bool;
    const HAS_INTERNAL_CRSF: bool;
    const SUPPORTED_PROTOCOLS: &'static [RfProtocol];
    const DEFAULT_PROTOCOL: RfProtocol;

    // Declarative Menu & Screen Catalogs
    const MAIN_MENU_ITEMS: &'static [MainMenuItem];
    const RADIO_SETUP_ITEMS: &'static [RadioSetupItem];
    const HARDWARE_EXT_ITEMS: &'static [HardwareExtItem];
    const DIAG_PAGES: &'static [DiagPage];

    // Board-Specific Labels for Hardware Extensions & Diagnostics
    const EXT_PWR_LABEL: &'static str;
    const EXT_ADC_LABEL: &'static str;
    const EXT_ADC_ON_STR: &'static str;
    const EXT_ADC_OFF_STR: &'static str;
    const PPM_OUT_ON_STR: &'static str;
    const PPM_IN_ON_STR: &'static str;

    const DIAG_SW_TITLE: &'static str;
    const DIAG_EXT_ADC_TITLE: &'static str;
    const DIAG_EXT_ADC_EN_HDR: &'static str;
    const DIAG_EXT_ADC_DIS_HDR: &'static str;
    const DIAG_EXT_ADC_NAMES: &'static [&'static str];

    // Channel & Source Tables
    const AUX_NAMES: &'static [&'static str];
    const ADAPTIVE_CH_NAMES: &'static [&'static str];
    const AUX_SOURCES: &'static [u8];

    // CRC-16 Checksum for EEPROM
    fn eeprom_crc16(data: &[u8]) -> u16;

    /// Convert raw 12-bit battery ADC sample into millivolts.
    fn calculate_battery_mv(raw: u16) -> u16;

    /// Number of visible items in `INPUT MODES` setup screen.
    #[inline]
    fn input_setup_items(ext_adc_enabled: bool) -> usize {
        if ext_adc_enabled {
            Self::TOTAL_ADC_COUNT
        } else {
            6
        }
    }

    /// Resolve a stored `u8` protocol ID against `Self::SUPPORTED_PROTOCOLS`,
    /// falling back to `Self::DEFAULT_PROTOCOL` if the stored protocol is not compiled/supported.
    #[inline]
    fn resolve_protocol(raw_proto: u8) -> RfProtocol {
        let candidate = RfProtocol::from_u8(raw_proto);
        let mut i = 0;
        while i < Self::SUPPORTED_PROTOCOLS.len() {
            if Self::SUPPORTED_PROTOCOLS[i] as u8 == candidate as u8 {
                return candidate;
            }
            i += 1;
        }
        Self::DEFAULT_PROTOCOL
    }

    /// Cycle to the next supported RF protocol in `Self::SUPPORTED_PROTOCOLS`.
    #[inline]
    fn next_protocol(raw_proto: u8) -> RfProtocol {
        let supported = Self::SUPPORTED_PROTOCOLS;
        if supported.is_empty() {
            return Self::DEFAULT_PROTOCOL;
        }
        let cur = Self::resolve_protocol(raw_proto);
        let pos = supported.iter().position(|&p| p == cur).unwrap_or(0);
        supported[(pos + 1) % supported.len()]
    }

    /// Cycle to the previous supported RF protocol in `Self::SUPPORTED_PROTOCOLS`.
    #[inline]
    fn prev_protocol(raw_proto: u8) -> RfProtocol {
        let supported = Self::SUPPORTED_PROTOCOLS;
        if supported.is_empty() {
            return Self::DEFAULT_PROTOCOL;
        }
        let cur = Self::resolve_protocol(raw_proto);
        let pos = supported.iter().position(|&p| p == cur).unwrap_or(0);
        if pos == 0 {
            supported[supported.len() - 1]
        } else {
            supported[pos - 1]
        }
    }

    /// Hardware dispatch hook: configure J4 pins as GPIO inputs or SWD debug.
    fn set_j4_mode_enabled(enabled: bool);

    /// Check if a mix source ID is available on this board given radio and model configuration.
    fn is_source_available(radio: &RadioConfig, model: &ModelConfig, src: u8) -> bool;

    /// Check if a switch condition index is available on this board given radio configuration.
    fn is_switch_cond_available(radio: &RadioConfig, cond: u8) -> bool;

    /// Early board initialization (e.g. power latch on FS-i6S).
    fn pre_init();

    /// Initialize capacitive touchscreen controller (if supported).
    fn init_touch();

    /// Initialize board-specific switch/button pins after loading config.
    fn init_board_keys(radio: &RadioConfig);

    /// Poll navigation keys from physical buttons and touch gestures.
    fn poll_keys(storage: &RadioStorage, menu_active: bool) -> u16;

    /// Update dynamic flight switches (e.g. rear buttons or J4 buttons).
    fn update_flight_switches(state: &mut InputState, storage: &RadioStorage, menu_active: bool);

    /// Decode board-specific switches (SE..SWM) from raw ADC and GPIO states.
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
    );

    /// Render default switch and dual pot layout on flight dashboard.
    fn render_default_layout(lcd: &mut St7567, state: &InputState, storage: &RadioStorage);

    /// Append board-specific extra switches to adaptive dashboard switch list.
    fn append_extra_switches(
        state: &InputState,
        storage: &RadioStorage,
        sw_list: &mut [(&'static str, SwitchPos, bool, u8); 16],
        sw_count: &mut usize,
    );

    /// Board-specific background idle state hook type.
    type IdleState: BoardIdleHook;

    /// Create new instance of board-specific idle state.
    fn create_idle_state() -> Self::IdleState;
}

/// Board-specific background idle hook interface.
pub trait BoardIdleHook: Default + 'static {
    /// Update board-specific background tasks and return `(ok_held_for_menu, timer_reset_clicked, instant_trim_clicked)`.
    fn update(
        &mut self,
        now: u32,
        dt_ms: u16,
        keys: u16,
        menu_active: bool,
        storage: &mut RadioStorage,
        buzzer: &mut Buzzer,
    ) -> (bool, bool, bool);

    /// Clear menu-open hold state when menu is opened or released.
    fn clear_menu_open_state(&mut self);

    /// Take frame keys for display frame dispatch, resetting latched state.
    fn take_frame_keys(&mut self, current_keys: u16, menu_or_calib_active: bool) -> u16;

    /// Render board-specific overlay (e.g. shutdown progress modal).
    fn render_overlay(&self, lcd: &mut St7567);
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct I6sSpec;

impl BoardProfile for I6sSpec {
    const MODEL_NAME: &'static str = "FS-i6S";
    const FIRMWARE_ID: &'static str = "ferrox-flysky-i6s";
    const USB_PRODUCT_JOYSTICK: &'static str = "FS-i6S Joystick";
    const USB_SERIAL_JOYSTICK: &'static str = "FS-I6S-SIM";
    const USB_PRODUCT_SERIAL: &'static str = "FS-i6S Serial";
    const USB_SERIAL_SERIAL: &'static str = "FS-I6S-VCP";
    const USB_PRODUCT_COMPOSITE: &'static str = "FS-i6S Radio";
    const USB_SERIAL_COMPOSITE: &'static str = "FS-I6S-COMP";

    const CLI_PROMPT: &'static [u8] = b"i6s> ";
    const CLI_PAUSE_PROMPT: &'static [u8] = b"\r\n[Stream paused]\r\ni6s> ";
    const STATUS_BANNER: &'static [u8] = concat!(
        "FlySky FS-i6S Rust Firmware ",
        env!("FIRMWARE_VERSION"),
        " (",
        env!("GIT_HASH"),
        ")\r\n"
    )
    .as_bytes();
    const CRSF_UART_STATUS: &'static [u8] = b"Protocol: CRSF / ExpressLRS (USART1 PB6/PB7 active)\r\n";

    const EXT_ADC_COUNT: usize = 5;
    const DEFAULT_SB_MODE: AdcInputMode = AdcInputMode::ThreePos;
    const MAX_MIX_SRC: u8 = 49;
    const MAX_MIX_SWITCH: u8 = 28;
    const MAX_AUX_SRC: u8 = 14;

    const HAS_JOG_WHEELS: bool = true;
    const HAS_ROTARY_ENCODER: bool = false;
    const HAS_TOUCH: bool = true;
    const HAS_RGB_BACKLIGHT: bool = true;
    const HAS_REAR_AND_J4_SWITCHES: bool = true;
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
        RadioSetupItem::RearLeftFunc,
        RadioSetupItem::RearRightFunc,
        RadioSetupItem::TouchEnabled,
        RadioSetupItem::StickMode,
        RadioSetupItem::GimbalType,
        RadioSetupItem::SwitchBeep,
        RadioSetupItem::InputSetupSubmenu,
    ];

    const HARDWARE_EXT_ITEMS: &'static [HardwareExtItem] = &[
        HardwareExtItem::ExtModulePwr,
        HardwareExtItem::J4Mode,
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

    const EXT_PWR_LABEL: &'static str = "H1 PF6 Pwr:";
    const EXT_ADC_LABEL: &'static str = "Ext ADC (PC):";
    const EXT_ADC_ON_STR: &'static str = "PC1-5 (ON)";
    const EXT_ADC_OFF_STR: &'static str = "OFF";
    const PPM_OUT_ON_STR: &'static str = "PF10 (ON)";
    const PPM_IN_ON_STR: &'static str = "PF9 (ON)";

    const DIAG_SW_TITLE: &'static str = "SWITCHES (E/F/G/H)";
    const DIAG_EXT_ADC_TITLE: &'static str = "EXT ANALOG (PC1-5)";
    const DIAG_EXT_ADC_EN_HDR: &'static str = "PC ADC: ENABLED (16-CH)";
    const DIAG_EXT_ADC_DIS_HDR: &'static str = "PC ADC: DISABLED (OFF)";
    const DIAG_EXT_ADC_NAMES: &'static [&'static str] = &[
        "VRC:PC1 ", "VRD:PC2 ", "VRE:PC3 ", "VRF:PC4 ", "VRG:PC5 ",
    ];

    const AUX_NAMES: &'static [&'static str] = &[
        "SA (PA4):",
        "SB (PA5):",
        "SC (PB0):",
        "SD (PB1):",
        "VRA (PA6):",
        "VRB (PA7):",
        "VRC (PC1):",
        "VRD (PC2):",
        "VRE (PC3):",
        "VRF (PC4):",
        "VRG (PC5):",
    ];

    const ADAPTIVE_CH_NAMES: &'static [&'static str] = &[
        "A", "B", "C", "D", "1", "2", "I", "J", "K", "L", "M",
    ];

    const AUX_SOURCES: &'static [u8] = &[
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44,
    ];

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
        crate::storage::crc16_ccitt_false(data)
    }

    #[inline]
    fn calculate_battery_mv(raw: u16) -> u16 {
        let vbat_mv = (raw as u32 * 977064) / 409500;
        vbat_mv as u16
    }

    #[inline]
    fn set_j4_mode_enabled(enabled: bool) {
        crate::boot::init_j4_keys(enabled);
    }

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
                radio.ext_adc != 0
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
    fn pre_init() {
        crate::power::init();
    }

    #[inline]
    fn init_touch() {
        crate::touch::ft6236::init();
    }

    #[inline]
    fn init_board_keys(radio: &RadioConfig) {
        crate::boot::init_j4_keys(radio.j4_sg_sh_en != 0);
    }

    #[inline]
    fn poll_keys(storage: &RadioStorage, menu_active: bool) -> u16 {
        let touch_sample =
            if storage.radio.touch_disabled == 0 && crate::touch::ft6236::is_touch_int_asserted() {
                crate::touch::ft6236::read_touch()
            } else {
                None
            };
        let current_sticks = crate::input::read().gimbals;
        crate::touch::nav::update_inputs(
            touch_sample.as_ref(),
            &current_sticks,
            menu_active,
            storage.radio.rear_left_func,
            storage.radio.rear_right_func,
            storage.radio.throttle_trim != 0,
        )
    }

    #[inline]
    fn update_flight_switches(state: &mut InputState, storage: &RadioStorage, menu_active: bool) {
        let (left_btn, right_btn) = crate::boot::read_front_buttons();
        if !menu_active && storage.radio.rear_left_func == 0 && left_btn {
            state.switches.se = SwitchPos::Down;
        } else {
            state.switches.se = SwitchPos::Up;
        }
        if !menu_active && storage.radio.rear_right_func == 0 && right_btn {
            state.switches.sf = SwitchPos::Down;
        } else {
            state.switches.sf = SwitchPos::Up;
        }

        if storage.radio.j4_sg_sh_en != 0 {
            let (sg_pressed, sh_pressed) = crate::boot::read_j4_buttons();
            state.switches.sg = if sg_pressed {
                SwitchPos::Down
            } else {
                SwitchPos::Up
            };
            state.switches.sh = if sh_pressed {
                SwitchPos::Down
            } else {
                SwitchPos::Up
            };
        } else {
            state.switches.sg = SwitchPos::Up;
            state.switches.sh = SwitchPos::Up;
        }
    }

    #[inline]
    fn decode_board_switches(
        raw: &[u16; crate::adc::NUM_CHANNELS],
        _ext_switches: bool,
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
        let (swi, swj, swk, swl, swm) = if ext_adc {
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
            let swm = match AdcInputMode::resolve(10, adc_modes[10]) {
                AdcInputMode::ThreePos => crate::input::decode_switch_3pos(raw[15]),
                AdcInputMode::TwoPos => crate::input::decode_switch_2pos(raw[15]),
                _ => SwitchPos::Up,
            };
            (swi, swj, swk, swl, swm)
        } else {
            (
                SwitchPos::Up,
                SwitchPos::Up,
                SwitchPos::Up,
                SwitchPos::Up,
                SwitchPos::Up,
            )
        };

        (
            SwitchPos::Up,
            SwitchPos::Up,
            SwitchPos::Up,
            SwitchPos::Up,
            swi,
            swj,
            swk,
            swl,
            swm,
        )
    }

    fn render_default_layout(lcd: &mut St7567, state: &InputState, storage: &RadioStorage) {
        let sg_sh_active = storage.radio.j4_sg_sh_en != 0;

        if sg_sh_active {
            let sw_list = [
                ("A", state.switches.sa),
                ("B", state.switches.sb),
                ("C", state.switches.sc),
                ("D", state.switches.sd),
                ("E", state.switches.se),
                ("F", state.switches.sf),
                ("G", state.switches.sg),
                ("H", state.switches.sh),
            ];
            let mut x = 2;
            for (lbl, pos) in sw_list.iter() {
                lcd.draw_str_4x6(x, 47, lbl, false);
                crate::ui::glyphs::draw_switch_arrow(lcd, x + 5, 46, *pos);
                x += 12;
            }

            crate::ui::widgets::draw_split_pot_bar(lcd, 100, 46, 26, state.pots.vr1, state.pots.vr2);
        } else {
            let sw_list = [
                ("A", state.switches.sa),
                ("B", state.switches.sb),
                ("C", state.switches.sc),
                ("D", state.switches.sd),
                ("E", state.switches.se),
                ("F", state.switches.sf),
            ];
            let mut x = 2;
            for (lbl, pos) in sw_list.iter() {
                lcd.draw_str_4x6(x, 47, lbl, false);
                crate::ui::glyphs::draw_switch_arrow(lcd, x + 6, 46, *pos);
                x += 14;
            }

            lcd.draw_str_4x6(86, 47, "VR", false);
            crate::ui::widgets::draw_split_pot_bar(lcd, 96, 46, 30, state.pots.vr1, state.pots.vr2);
        }
    }

    fn append_extra_switches(
        state: &InputState,
        storage: &RadioStorage,
        sw_list: &mut [(&'static str, SwitchPos, bool, u8); 16],
        sw_count: &mut usize,
    ) {
        if *sw_count < sw_list.len() {
            sw_list[*sw_count] = ("E", state.switches.se, false, 0);
            *sw_count += 1;
        }
        if *sw_count < sw_list.len() {
            sw_list[*sw_count] = ("F", state.switches.sf, false, 0);
            *sw_count += 1;
        }
        if storage.radio.j4_sg_sh_en != 0 {
            if *sw_count < sw_list.len() {
                sw_list[*sw_count] = ("G", state.switches.sg, false, 0);
                *sw_count += 1;
            }
            if *sw_count < sw_list.len() {
                sw_list[*sw_count] = ("H", state.switches.sh, false, 0);
                *sw_count += 1;
            }
        }
    }

    type IdleState = I6sIdleState;

    #[inline]
    fn create_idle_state() -> Self::IdleState {
        I6sIdleState::new()
    }
}

/// Board-specific background idle state for FS-i6S.
pub struct I6sIdleState {
    pub power_manager: crate::power::PowerManager,
    prev_left_btn: bool,
    prev_right_btn: bool,
    latched_keys: u16,
    last_center_touch_ms: u32,
    menu_led_phase: bool,
    menu_led_timer_ms: u16,
}

impl Default for I6sIdleState {
    fn default() -> Self {
        Self::new()
    }
}

impl I6sIdleState {
    pub const fn new() -> Self {
        Self {
            power_manager: crate::power::PowerManager::new(),
            prev_left_btn: false,
            prev_right_btn: false,
            latched_keys: 0,
            last_center_touch_ms: 0,
            menu_led_phase: true,
            menu_led_timer_ms: 0,
        }
    }
}

impl BoardIdleHook for I6sIdleState {
    fn update(
        &mut self,
        now: u32,
        dt_ms: u16,
        keys: u16,
        menu_active: bool,
        storage: &mut RadioStorage,
        buzzer: &mut Buzzer,
    ) -> (bool, bool, bool) {
        self.power_manager.update(dt_ms, storage, buzzer);
        self.latched_keys |= keys;

        // Front Button LED Backlight Handling
        if menu_active {
            self.menu_led_timer_ms = self.menu_led_timer_ms.saturating_add(dt_ms);
            if self.menu_led_timer_ms >= 250 {
                self.menu_led_timer_ms = 0;
                self.menu_led_phase = !self.menu_led_phase;
                crate::power::set_leds(self.menu_led_phase);
            }
        } else if !self.menu_led_phase {
            self.menu_led_phase = true;
            self.menu_led_timer_ms = 0;
            crate::power::set_leds(true);
        }

        let (left_btn, right_btn) = crate::boot::read_front_buttons();
        let left_clicked = left_btn && !self.prev_left_btn;
        let right_clicked = right_btn && !self.prev_right_btn;
        self.prev_left_btn = left_btn;
        self.prev_right_btn = right_btn;

        let ok_held = if !menu_active {
            let center_touch = (keys & crate::touch::nav::KEY_MENU_OPEN) != 0;
            if center_touch {
                self.last_center_touch_ms = now;
            }
            let touch_active =
                self.last_center_touch_ms != 0 && now.wrapping_sub(self.last_center_touch_ms) < 150;
            touch_active || right_btn
        } else {
            self.last_center_touch_ms = 0;
            false
        };

        let timer_reset_clicked = !menu_active && left_clicked && storage.radio.rear_left_func == 1;
        let instant_trim_clicked =
            !menu_active && right_clicked && storage.radio.rear_right_func == 1;

        (ok_held, timer_reset_clicked, instant_trim_clicked)
    }

    #[inline]
    fn clear_menu_open_state(&mut self) {
        self.last_center_touch_ms = 0;
    }

    #[inline]
    fn take_frame_keys(&mut self, _current_keys: u16, menu_or_calib_active: bool) -> u16 {
        let mut frame_keys = self.latched_keys;
        self.latched_keys = 0;
        if menu_or_calib_active && (frame_keys & ((1 << 8) | (1 << 9))) != 0 {
            frame_keys &= !(1 << 10);
        }
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
        let (se, sf) = if ext_switches {
            crate::boot::read_ext_switches()
        } else {
            (SwitchPos::Up, SwitchPos::Up)
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

#[inline]
fn draw_switch_slot_i6x(lcd: &mut St7567, label_x: i32, val_x: i32, label: &str, pos: SwitchPos) {
    lcd.draw_str_4x6(label_x, 47, label, false);
    crate::ui::glyphs::draw_switch_arrow(lcd, val_x, 46, pos);
}

/// Default/standard background idle state (FS-i6X and RM-Pocket).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct DefaultIdleState;

impl BoardIdleHook for DefaultIdleState {
    #[inline]
    fn update(
        &mut self,
        _now: u32,
        _dt_ms: u16,
        keys: u16,
        menu_active: bool,
        _storage: &mut RadioStorage,
        _buzzer: &mut Buzzer,
    ) -> (bool, bool, bool) {
        let ok_held = !menu_active && (keys & (1 << 10)) != 0;
        (ok_held, false, false)
    }

    #[inline]
    fn clear_menu_open_state(&mut self) {}

    #[inline]
    fn take_frame_keys(&mut self, current_keys: u16, _menu_or_calib_active: bool) -> u16 {
        current_keys
    }

    #[inline]
    fn render_overlay(&self, _lcd: &mut St7567) {}
}

/// RadioMaster Pocket specification: STM32F407, ST7567 128x64 mono, rotary encoder, native ELRS.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct PocketSpec;

impl BoardProfile for PocketSpec {
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
    )
    .as_bytes();
    const CRSF_UART_STATUS: &'static [u8] = b"Protocol: CRSF / ExpressLRS (Internal UART active)\r\n";

    const EXT_ADC_COUNT: usize = 0;
    const DEFAULT_SB_MODE: AdcInputMode = AdcInputMode::ThreePos;
    const MAX_MIX_SRC: u8 = 45;
    const MAX_MIX_SWITCH: u8 = 22;
    const MAX_AUX_SRC: u8 = 12;

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

    const HARDWARE_EXT_ITEMS: &'static [HardwareExtItem] = &[
        HardwareExtItem::ExtModulePwr,
    ];

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

    const DIAG_SW_TITLE: &'static str = "SWITCHES (SA-SD)";
    const DIAG_EXT_ADC_TITLE: &'static str = "EXT ANALOG";
    const DIAG_EXT_ADC_EN_HDR: &'static str = "EXT ADC: ENABLED";
    const DIAG_EXT_ADC_DIS_HDR: &'static str = "EXT ADC: DISABLED";
    const DIAG_EXT_ADC_NAMES: &'static [&'static str] = &[];

    const AUX_NAMES: &'static [&'static str] = &[
        "SA:", "SB:", "SC:", "SD:", "S1:", "S2:",
    ];

    const ADAPTIVE_CH_NAMES: &'static [&'static str] = &[
        "A", "B", "C", "D", "1", "2",
    ];

    const AUX_SOURCES: &'static [u8] = &[0, 1, 2, 3, 4, 5, 31, 32, 35, 36];

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
        // Pocket 1S/2S divider
        (raw as u32 * 3300 * 2 / 4095) as u16
    }

    #[inline]
    fn set_j4_mode_enabled(_enabled: bool) {}

    fn is_source_available(radio: &RadioConfig, model: &ModelConfig, src: u8) -> bool {
        match src {
            0..=4 => true, // None, Roll, Pitch, Thr, Yaw
            5 => radio.get_adc_mode(4).is_pot(), // S1
            6 => radio.get_adc_mode(5).is_pot(), // S2
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
        (
            SwitchPos::Up,
            SwitchPos::Up,
            SwitchPos::Up,
            SwitchPos::Up,
            SwitchPos::Up,
            SwitchPos::Up,
            SwitchPos::Up,
            SwitchPos::Up,
            SwitchPos::Up,
        )
    }

    fn render_default_layout(lcd: &mut St7567, state: &InputState, _storage: &RadioStorage) {
        draw_switch_slot_i6x(lcd, 2, 9, "A", state.switches.sa);
        draw_switch_slot_i6x(lcd, 20, 27, "B", state.switches.sb);
        draw_switch_slot_i6x(lcd, 38, 45, "C", state.switches.sc);
        draw_switch_slot_i6x(lcd, 56, 63, "D", state.switches.sd);
        lcd.draw_str_4x6(74, 47, "VR", false);
        crate::ui::widgets::draw_split_pot_bar(lcd, 84, 46, 42, state.pots.vr1, state.pots.vr2);
    }

    fn append_extra_switches(
        _state: &InputState,
        _storage: &RadioStorage,
        _sw_list: &mut [(&'static str, SwitchPos, bool, u8); 16],
        _sw_count: &mut usize,
    ) {
    }

    type IdleState = DefaultIdleState;

    #[inline]
    fn create_idle_state() -> Self::IdleState {
        DefaultIdleState
    }
}

#[cfg(feature = "board-i6s")]
pub type ActiveBoard = I6sSpec;

#[cfg(feature = "board-i6x")]
pub type ActiveBoard = I6xSpec;

#[cfg(feature = "board-pocket")]
pub type ActiveBoard = PocketSpec;

#[cfg(not(any(feature = "board-i6s", feature = "board-i6x", feature = "board-pocket")))]
pub type ActiveBoard = I6xSpec;
