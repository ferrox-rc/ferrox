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
    const DISPLAY_WIDTH: usize = 128;
    const DISPLAY_HEIGHT: usize = 64;
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


#[inline]
pub fn draw_switch_slot_i6x(lcd: &mut St7567, label_x: i32, val_x: i32, label: &str, pos: SwitchPos) {
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



pub mod at9spro;
pub mod boxer;
pub mod i6s;
pub mod i6x;
pub mod pocket;
pub mod qx7;
pub mod qx7_family;
pub mod t12;
pub mod tprov2;
pub mod tx12;
pub mod x9d;
pub mod x9e;
pub mod zorro;

pub use at9spro::At9sProSpec;
pub use boxer::BoxerSpec;
pub use i6s::I6sSpec;
pub use i6x::I6xSpec;
pub use pocket::PocketSpec;
pub use qx7::Qx7Spec;
pub use qx7_family::{Qx7FamilyProfile, Qx7IdleState};
pub use t12::T12Spec;
pub use tprov2::TProV2Spec;
pub use tx12::Tx12Spec;
pub use x9d::X9dSpec;
pub use x9e::X9eSpec;
pub use zorro::ZorroSpec;

// ============================================================================
// Active Board Type Resolution
// ============================================================================

#[cfg(feature = "board-i6s")]
pub type ActiveBoard = I6sSpec;

#[cfg(feature = "board-i6x")]
pub type ActiveBoard = I6xSpec;

#[cfg(feature = "board-pocket")]
pub type ActiveBoard = PocketSpec;

#[cfg(feature = "board-boxer")]
pub type ActiveBoard = BoxerSpec;

#[cfg(feature = "board-zorro")]
pub type ActiveBoard = ZorroSpec;

#[cfg(feature = "board-tx12")]
pub type ActiveBoard = Tx12Spec;

#[cfg(feature = "board-qx7")]
pub type ActiveBoard = Qx7Spec;

#[cfg(feature = "board-tprov2")]
pub type ActiveBoard = TProV2Spec;

#[cfg(feature = "board-t12")]
pub type ActiveBoard = T12Spec;

#[cfg(feature = "board-x9d")]
pub type ActiveBoard = X9dSpec;

#[cfg(feature = "board-x9e")]
pub type ActiveBoard = X9eSpec;

#[cfg(feature = "board-at9spro")]
pub type ActiveBoard = At9sProSpec;

#[cfg(not(any(
    feature = "board-i6s",
    feature = "board-i6x",
    feature = "board-pocket",
    feature = "board-boxer",
    feature = "board-zorro",
    feature = "board-tx12",
    feature = "board-qx7",
    feature = "board-tprov2",
    feature = "board-t12",
    feature = "board-x9d",
    feature = "board-x9e",
    feature = "board-at9spro"
)))]
pub type ActiveBoard = I6xSpec;

