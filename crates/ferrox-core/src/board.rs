//! Compile-time board profile and zero-cost HAL dispatch for Ferrox targets.

use crate::storage::AdcInputMode;
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
        #[cfg(feature = "board-i6s")]
        crate::hw::i6s::set_j4_mode_enabled(enabled);
        #[cfg(not(feature = "board-i6s"))]
        let _ = enabled;
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
}

#[cfg(feature = "board-i6s")]
pub type ActiveBoard = I6sSpec;

#[cfg(all(not(feature = "board-i6s"), not(feature = "board-pocket")))]
pub type ActiveBoard = I6xSpec;

#[cfg(feature = "board-pocket")]
pub type ActiveBoard = PocketSpec;
