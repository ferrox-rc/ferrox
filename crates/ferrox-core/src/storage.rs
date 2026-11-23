//! I2C EEPROM storage (FT24C128A 16 KB) for non-volatile configuration and 20-model memory system.
//!
//! Supports adaptive bus & address auto-detection:
//! - Primary: I2C2 on PB10 (SCL) and PB11 (SDA) across addresses 0x50..0x57
//! - Fallback: I2C1 on PB8 (SCL) and PB9 (SDA) across addresses 0x50..0x57
//!
//! Physical capacity: 16,384 bytes (256 pages x 64 bytes).
//! Slot layout:
//! - 0x0000..0x0100 (256 B / 4 pages): Primary Radio Config (Slot A)
//! - 0x0100..0x0200 (256 B / 4 pages): Backup Radio Config (Slot B - A/B safe save)
//! - 0x0200..0x3E00 (15,360 B / 240 pages): 20 Models x 768 B (12 pages each)
//! - 0x3E00..0x4000 (512 B / 8 pages): Reserved / scratchpad

use crate::board::BoardProfile;

use crate::hal::StorageBus;

static mut STORAGE_BUS: Option<&'static mut dyn StorageBus> = None;

/// Register the target MCU's non-volatile storage driver.
pub fn register_storage_bus(bus: &'static mut dyn StorageBus) {
    unsafe {
        STORAGE_BUS = Some(bus);
    }
}

/// Initialize the SD card for read-only FAT access (e.g. `/SOUNDS/` playback), if present.
pub fn init_sd_readonly() -> bool {
    unsafe {
        match STORAGE_BUS {
            Some(ref mut bus) => bus.init_sd_readonly(),
            None => false,
        }
    }
}

/// Read a raw 512-byte sector at `lba` from the SD card, if present.
#[allow(clippy::result_unit_err)]
pub fn read_raw_sd_sector(lba: u32, buf: &mut [u8; 512]) -> Result<(), ()> {
    unsafe {
        match STORAGE_BUS {
            Some(ref mut bus) => bus.read_raw_sector(lba, buf),
            None => Err(()),
        }
    }
}

/// Execute a closure with mutable access to the registered `StorageBus`, if present.
pub fn with_storage_bus<R>(f: impl FnOnce(&mut dyn StorageBus) -> R) -> Option<R> {
    unsafe { STORAGE_BUS.as_mut().map(|bus| f(*bus)) }
}

pub const FLASH_MAGIC: u32 = 0x4653_4B59; // "FSKY"
pub const CONFIG_VERSION: u32 = 5;
pub const NUM_MODELS: usize = 20;

/// 8-byte framing envelope magic ("FX" in little-endian = 0x58, 0x46 -> 0x4658)
pub const SLOT_MAGIC: u16 = 0x4658;
pub const SCHEMA_VERSION: u8 = 2;
pub const HEADER_SIZE: usize = 8;

pub const PAGE_SIZE: usize = 64;
pub const EEPROM_TOTAL_SIZE: usize = 16_384;

pub const RADIO_SLOT_A_ADDR: u16 = 0x0000;
pub const RADIO_SLOT_B_ADDR: u16 = 0x0100;
pub const RADIO_SLOT_SIZE: usize = 256;

pub const MODEL_BASE_ADDR: u16 = 0x0200;
pub const MODEL_SLOT_SIZE: usize = 768;

#[inline(always)]
pub const fn model_slot_addr(index: usize) -> Option<u16> {
    if index >= NUM_MODELS {
        None
    } else {
        Some(MODEL_BASE_ADDR + (index as u16) * (MODEL_SLOT_SIZE as u16))
    }
}

/// Split an arbitrary EEPROM write buffer across `PAGE_SIZE` (64-byte) page boundaries
/// with overflow validation, invoking `write_page(page_addr, chunk)` for each page chunk.
#[allow(clippy::result_unit_err)]
pub fn for_each_eeprom_page(
    addr: u16,
    buf: &[u8],
    mut write_page: impl FnMut(u16, &[u8]) -> Result<(), ()>,
) -> Result<(), ()> {
    let end = (addr as usize).checked_add(buf.len()).ok_or(())?;
    if end > (u16::MAX as usize) + 1 {
        return Err(());
    }
    let mut cur_addr = addr;
    let mut offset = 0;
    let total = buf.len();

    while offset < total {
        let page_offset = (cur_addr as usize) % PAGE_SIZE;
        let bytes_in_page = PAGE_SIZE - page_offset;
        let chunk_size = bytes_in_page.min(total - offset).min(PAGE_SIZE);

        write_page(cur_addr, &buf[offset..offset + chunk_size])?;

        offset += chunk_size;
        if offset < total {
            cur_addr = cur_addr.checked_add(chunk_size as u16).ok_or(())?;
        }
    }
    Ok(())
}

/// Calibration parameters for a single analog channel (8 bytes).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ChannelCalib {
    pub min: u16,
    pub center: u16,
    pub max: u16,
    pub _pad: u16,
}

impl ChannelCalib {
    pub const fn new(min: u16, center: u16, max: u16) -> Self {
        Self {
            min,
            center,
            max,
            _pad: 0,
        }
    }
}

/// Compact calibration parameters for an auxiliary potentiometer/slider (6 bytes).
/// Supports both standard pots (with calculated midpoint) and detented pots (with fixed detent center).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PotCalib {
    pub min: u16,
    pub center: u16,
    pub max: u16,
}

impl PotCalib {
    pub const fn new(min: u16, center: u16, max: u16) -> Self {
        Self { min, center, max }
    }
}

/// Universal input mode for auxiliary analog channels (SA..SD, VRA..VRB, VRC..VRF).
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum AdcInputMode {
    #[default]
    Default = 0,
    Off = 1,
    TwoPos = 2,
    ThreePos = 3,
    SixPos = 4,
    Pot = 5,
    PotDetent = 6,
    InstantTrim = 7,
}

impl AdcInputMode {
    pub fn from_u8(val: u8) -> Self {
        match val {
            1 => Self::Off,
            2 => Self::TwoPos,
            3 => Self::ThreePos,
            4 => Self::SixPos,
            5 => Self::Pot,
            6 => Self::PotDetent,
            7 => Self::InstantTrim,
            _ => Self::Default,
        }
    }

    /// Check if this mode represents any analog potentiometer (continuous or center-detent).
    pub fn is_pot(&self) -> bool {
        matches!(self, Self::Pot | Self::PotDetent)
    }

    /// Check if this mode utilizes a physical center detent.
    pub fn has_detent(&self) -> bool {
        matches!(self, Self::PotDetent)
    }

    /// Check if this input is disabled / turned off.
    pub fn is_off(&self) -> bool {
        matches!(self, Self::Off)
    }

    /// Resolve effective mode for a given channel index (0..3: SA..SD, 4..5: VRA..VRB, 6..10: VRC..VRG)
    pub fn resolve(channel_idx: usize, mode_val: u8) -> Self {
        let mode = Self::from_u8(mode_val);
        if mode != Self::Default {
            return mode;
        }
        // Default hardware mapping:
        match channel_idx {
            0 => Self::TwoPos,                               // SA (2-pos stock)
            1 => crate::board::ActiveBoard::DEFAULT_SB_MODE, // SB (2-pos stock on i6X, 3-pos stock on i6S)
            2 => Self::ThreePos,                             // SC (3-pos stock)
            3 => Self::TwoPos,                               // SD (2-pos stock)
            _ => Self::Pot,                                  // VRA..VRG (Pots stock)
        }
    }
}

/// Persistent system/radio-level configuration (SCHEMA_VERSION = 2, 160 bytes, 4-byte aligned).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RadioConfig {
    pub magic: u32,                // 0..4 ("FSKY")
    pub version: u32,              // 4..8 (CONFIG_VERSION)
    pub sticks: [ChannelCalib; 4], // 8..40 (32 bytes: Roll, Pitch, Throttle, Yaw)
    pub aux_pots: [PotCalib; 11],  // 40..106 (66 bytes: SA..SD, VRA..VRB, VRC..VRG)
    pub servo_rate_hz: u16,        // 106..108 (50..400 Hz, default 50 Hz)
    pub active_model: u8,          // 108 (0..19, active model index)
    pub audio_enabled: u8,         // 109 (0: Muted, 1: Enabled)
    pub backlight_timeout: u8,     // 110 (0: Always On, 1: 15s, 2: 30s, 3: 60s)
    pub backlight_brightness: u8,  // 111 (1..10)
    pub vbat_warn_deci: u8,        // 112 (40..50 = 4.0V..5.0V)
    pub lcd_contrast: u8,          // 113 (15..55, default 37)
    pub usb_mode: u8,              // 114 (0: Off, 1: Joystick, 2: Serial, 3: Composite)
    pub ext_module_pwr: u8,        // 115 (0: Active HIGH / N-type, 1: Active LOW / P-type)
    pub tone_style: u8,            // 116 (0: Simple, 1: Rich)
    pub rx_out_mode: u8,           // 117 (0: PWM, 1: PPM)
    pub rx_serial_proto: u8,       // 118 (0: i-BUS, 1: S.BUS)
    pub ext_switches: u8,          // 119 (0: Disabled, 1: Enabled)
    pub ext_adc: u8,               // 120 (0: Disabled, 1: Enabled)
    pub adc_modes: [u8; 11],       // 121..132: Input modes for the 11 auxiliary analog channels
    pub stick_mode: u8,            // 132 (0: Mode 1, 1: Mode 2, 2: Mode 3, 3: Mode 4)
    pub trainer_config: u8,        // 133: Bit 0: PPM Out, Bit 1: PPM In, Bit 2: DSM SAT In
    pub rear_left_func: u8,        // 134: 0: RC Switch (SWE), 1: Timer Reset, 2: Trims Modifier
    pub rear_right_func: u8,       // 135: 0: RC Switch (SWF), 1: Instant Trim, 2: Trims Modifier
    pub touch_disabled: u8,        // 136: 0: Touch Enabled, 1: Touch Disabled
    pub j4_sg_sh_en: u8,           // 137: 0: SWD Debug, 1: SG/SH Buttons
    pub switch_beep: u8,           // 138: 0: Muted, 1: Beep on switch toggle
    pub gimbal_type: u8,           // 139: 0: Potentiometer, 1: Hall Effect
    pub stick_invert_mask: u8,     // 140: 4 bits (bit 0: RH, 1: RV, 2: LV, 3: LH), default 0x03
    pub voice_cfg: u8,             // 141: Low nibble = volume step, High nibble = language index
    pub voice_interval: u8,        // 142: Periodic voice telemetry callout interval index (0..=4)
    pub _reserved: [u8; 17],       // 143..160: Zero-initialized tail for v1.x extensions
}

const _: () = assert!(core::mem::size_of::<RadioConfig>() == 160);
const _: () = assert!(HEADER_SIZE + core::mem::size_of::<RadioConfig>() <= RADIO_SLOT_SIZE);

impl RadioConfig {
    pub const fn default_factory() -> Self {
        Self {
            magic: FLASH_MAGIC,
            version: CONFIG_VERSION,
            sticks: [
                ChannelCalib::new(
                    crate::adc::ADC_MIN,
                    crate::adc::ADC_CENTER,
                    crate::adc::ADC_MAX,
                ), // Roll (Horizontal)
                ChannelCalib::new(
                    crate::adc::ADC_MIN,
                    crate::adc::ADC_CENTER,
                    crate::adc::ADC_MAX,
                ), // Pitch (Vertical)
                ChannelCalib::new(
                    crate::adc::ADC_MIN,
                    crate::adc::ADC_CENTER,
                    crate::adc::ADC_MAX,
                ), // Throttle (Vertical)
                ChannelCalib::new(
                    crate::adc::ADC_MIN,
                    crate::adc::ADC_CENTER,
                    crate::adc::ADC_MAX,
                ), // Yaw (Horizontal)
            ],
            aux_pots: [
                PotCalib::new(
                    crate::adc::ADC_MIN,
                    crate::adc::ADC_CENTER,
                    crate::adc::ADC_MAX,
                ), // SA
                PotCalib::new(
                    crate::adc::ADC_MIN,
                    crate::adc::ADC_CENTER,
                    crate::adc::ADC_MAX,
                ), // SB
                PotCalib::new(
                    crate::adc::ADC_MIN,
                    crate::adc::ADC_CENTER,
                    crate::adc::ADC_MAX,
                ), // SC
                PotCalib::new(
                    crate::adc::ADC_MIN,
                    crate::adc::ADC_CENTER,
                    crate::adc::ADC_MAX,
                ), // SD
                PotCalib::new(
                    crate::adc::ADC_MIN,
                    crate::adc::ADC_CENTER,
                    crate::adc::ADC_MAX,
                ), // VRA
                PotCalib::new(
                    crate::adc::ADC_MIN,
                    crate::adc::ADC_CENTER,
                    crate::adc::ADC_MAX,
                ), // VRB
                PotCalib::new(
                    crate::adc::ADC_MIN,
                    crate::adc::ADC_CENTER,
                    crate::adc::ADC_MAX,
                ), // VRC
                PotCalib::new(
                    crate::adc::ADC_MIN,
                    crate::adc::ADC_CENTER,
                    crate::adc::ADC_MAX,
                ), // VRD
                PotCalib::new(
                    crate::adc::ADC_MIN,
                    crate::adc::ADC_CENTER,
                    crate::adc::ADC_MAX,
                ), // VRE
                PotCalib::new(
                    crate::adc::ADC_MIN,
                    crate::adc::ADC_CENTER,
                    crate::adc::ADC_MAX,
                ), // VRF
                PotCalib::new(
                    crate::adc::ADC_MIN,
                    crate::adc::ADC_CENTER,
                    crate::adc::ADC_MAX,
                ), // VRG
            ],
            servo_rate_hz: 50,
            active_model: 0,
            audio_enabled: 1,
            backlight_timeout: 0,
            backlight_brightness: 10,
            vbat_warn_deci: crate::board::ActiveBoard::VBAT_WARN_DEFAULT_DECI,
            lcd_contrast: 37,
            usb_mode: 0,
            ext_module_pwr: 0,
            tone_style: 1,
            rx_out_mode: 0,
            rx_serial_proto: 0,
            ext_switches: 0,
            ext_adc: 0,
            adc_modes: [0; 11],
            stick_mode: 1, // Mode 2 default
            trainer_config: 0,
            rear_left_func: 0,
            rear_right_func: 0,
            touch_disabled: 0,
            j4_sg_sh_en: 0,
            switch_beep: 1,
            gimbal_type: 0,
            stick_invert_mask: 0x03, // RH (bit 0) and RV (bit 1) inverted by default for factory pot gimbals
            voice_cfg: 0,
            voice_interval: 0,
            _reserved: [0; 17],
        }
    }

    /// Check if aftermarket Hall effect gimbals mode is enabled.
    #[inline(always)]
    pub fn is_hall_gimbals(&self) -> bool {
        self.gimbal_type == 1
    }

    /// Supported EdgeTX 2-letter voice pack language folder codes (`/SOUNDS/<lang>/`).
    pub const VOICE_LANGS: [&'static str; 12] = [
        "en", "de", "fr", "nl", "es", "it", "pt", "cs", "pl", "ru", "zh", "ja",
    ];

    /// Return WAV/PCM voice playback volume (`0..=100%`, default `80%`).
    #[inline(always)]
    pub const fn voice_volume_pct(&self) -> u8 {
        match self.voice_cfg & 0x0F {
            0 => 80,
            step @ 1..=10 => step * 10,
            _ => 0,
        }
    }

    /// Cycle WAV/PCM voice volume in 20% steps (`0% -> 20% -> 40% -> 60% -> 80% -> 100% -> 0%`).
    #[inline]
    pub fn cycle_voice_volume(&mut self) {
        let cur = self.voice_volume_pct();
        let next = if cur >= 100 { 0 } else { (cur + 20).min(100) };
        let vol_nib = if next == 0 { 11 } else { next / 10 };
        self.voice_cfg = (self.voice_cfg & 0xF0) | (vol_nib & 0x0F);
    }

    /// Return active 2-letter EdgeTX voice pack language code (default `"en"`).
    #[inline(always)]
    pub const fn voice_lang(&self) -> &'static str {
        let idx = ((self.voice_cfg >> 4) & 0x0F) as usize;
        if idx < Self::VOICE_LANGS.len() {
            Self::VOICE_LANGS[idx]
        } else {
            "en"
        }
    }

    /// Cycle active voice pack language code through [`Self::VOICE_LANGS`].
    #[inline]
    pub fn cycle_voice_lang(&mut self) {
        let cur = ((self.voice_cfg >> 4) & 0x0F) as usize;
        let next = if cur + 1 >= Self::VOICE_LANGS.len() {
            0
        } else {
            cur + 1
        };
        self.voice_cfg = (self.voice_cfg & 0x0F) | ((next as u8) << 4);
    }

    /// Return periodic voice telemetry callout interval in seconds (`0` = OFF, default `15s`).
    #[inline(always)]
    pub const fn voice_interval_secs(&self) -> u8 {
        match self.voice_interval {
            0 => 15,
            1 => 30,
            2 => 60,
            3 => 0,
            4 => 10,
            _ => 15,
        }
    }

    /// Return UI label for the periodic voice callout interval.
    #[inline(always)]
    pub const fn voice_interval_label(&self) -> &'static str {
        match self.voice_interval_secs() {
            0 => "OFF",
            10 => "10 SEC",
            15 => "15 SEC",
            30 => "30 SEC",
            _ => "60 SEC",
        }
    }

    /// Cycle periodic voice telemetry callout interval (`15s -> 30s -> 60s -> OFF -> 10s -> 15s`).
    #[inline]
    pub fn cycle_voice_interval(&mut self) {
        self.voice_interval = (self.voice_interval + 1) % 5;
    }

    /// Check if a specific stick axis is inverted (0: RH, 1: RV, 2: LV, 3: LH).
    #[inline(always)]
    pub fn is_stick_inverted(&self, stick_idx: usize) -> bool {
        (self.stick_invert_mask & (1 << (stick_idx & 3))) != 0
    }

    /// Set stick axis inversion bit (0: RH, 1: RV, 2: LV, 3: LH).
    #[inline(always)]
    pub fn set_stick_inverted(&mut self, stick_idx: usize, inverted: bool) {
        let bit = 1 << (stick_idx & 3);
        if inverted {
            self.stick_invert_mask |= bit;
        } else {
            self.stick_invert_mask &= !bit;
        }
    }

    /// Check if PPM Output on PF10 is enabled.
    #[inline(always)]
    pub fn is_ppm_out_enabled(&self) -> bool {
        (self.trainer_config & 0x01) != 0
    }

    /// Check if PPM Input on PF9 is enabled.
    #[inline(always)]
    pub fn is_ppm_in_enabled(&self) -> bool {
        (self.trainer_config & 0x02) != 0
    }

    /// Set PPM Output on PF10 enabled state.
    #[inline(always)]
    pub fn set_ppm_out_enabled(&mut self, enabled: bool) {
        if enabled {
            self.trainer_config |= 0x01;
        } else {
            self.trainer_config &= !0x01;
        }
    }

    /// Set PPM Input on PF9 enabled state.
    #[inline(always)]
    pub fn set_ppm_in_enabled(&mut self, enabled: bool) {
        if enabled {
            self.trainer_config = (self.trainer_config & !0x04) | 0x02;
        } else {
            self.trainer_config &= !0x02;
        }
    }

    /// Check if Spektrum Satellite wireless trainer input on UART RX is enabled (bit 2 = 0x04).
    #[inline(always)]
    pub fn is_dsm_sat_in_enabled(&self) -> bool {
        (self.trainer_config & 0x04) != 0
    }

    /// Set Spektrum Satellite wireless trainer input enabled state.
    #[inline(always)]
    pub fn set_dsm_sat_in_enabled(&mut self, enabled: bool) {
        if enabled {
            self.trainer_config = (self.trainer_config & !0x02) | 0x04;
        } else {
            self.trainer_config &= !0x04;
        }
    }

    /// Cycle trainer input mode: OFF (0x00) -> PPM In (0x02) -> DSMX SAT (0x04) -> OFF.
    #[inline(always)]
    pub fn cycle_trainer_in_mode(&mut self) {
        if self.is_ppm_in_enabled() {
            self.trainer_config = (self.trainer_config & !0x06) | 0x04;
        } else if self.is_dsm_sat_in_enabled() {
            self.trainer_config &= !0x06;
        } else {
            self.trainer_config = (self.trainer_config & !0x06) | 0x02;
        }
    }

    /// Get raw configured ADC mode index (0..=6) for auxiliary input `ch` (`0..10`).
    #[inline(always)]
    pub const fn get_adc_mode_val(&self, ch: usize) -> u8 {
        if ch < 11 {
            self.adc_modes[ch]
        } else {
            0
        }
    }

    /// Set raw configured ADC mode index (0..=7) for auxiliary input `ch` (`0..10`).
    #[inline(always)]
    pub fn set_adc_mode_val(&mut self, ch: usize, val: u8) {
        if ch < 11 {
            self.adc_modes[ch] = val.min(7);
        }
    }

    /// Get the effective AdcInputMode for an auxiliary channel (0..10: SA..SD, VRA..VRB, VRC..VRG).
    pub fn get_adc_mode(&self, ch: usize) -> AdcInputMode {
        if ch < 11 {
            AdcInputMode::resolve(ch, self.adc_modes[ch])
        } else {
            AdcInputMode::Default
        }
    }

    /// Get the configured raw AdcInputMode (including Default).
    pub fn get_raw_adc_mode(&self, ch: usize) -> AdcInputMode {
        if ch < 11 {
            AdcInputMode::from_u8(self.adc_modes[ch])
        } else {
            AdcInputMode::Default
        }
    }
}

pub const NUM_MIX_LINES: usize = 32;

/// A single freeform mix rule in the matrix mixer (8 bytes).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MixLine {
    pub target_ch: u8,  // 0: Disabled, 1..18: Target Channel CH1..CH18
    pub source: u8, // 0: None, 1: Roll, 2: Pitch, 3: Thr, 4: Yaw, 5: VRA, 6: VRB, 7: SA, 8: SB, 9: SC, 10: SD, 11: MAX, 12..29: CH1..CH18, 30: ThrUnipolar, 31..40: SE..VRF
    pub weight_src: u8, // 0: Fixed Weight, 1..18: Modulate with CH1..CH18 (Bipolar), 19..36: Modulate (Unipolar 0..100%)
    pub weight: i8,     // -100% .. +100%
    pub offset: i8,     // -100% .. +100%
    pub switch: u8,     // 0: Always On, 1..22: SA_UP..SWJ_DN
    pub mode: u8,       // 0: Add (+), 1: Multiply (*), 2: Replace (:=)
    pub _reserved: u8,  // 1-byte padding for 8-byte alignment
}

impl MixLine {
    pub const fn disabled() -> Self {
        Self {
            target_ch: 0,
            source: 0,
            weight_src: 0,
            weight: 100,
            offset: 0,
            switch: 0,
            mode: 0,
            _reserved: 0,
        }
    }
}

pub const NUM_LOGICAL_SWITCHES: usize = 8;

/// Programmable boolean Logical Switch (`L1..L8`, 6 bytes).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct LogicalSwitch {
    /// Function:
    /// 0: OFF, 1: a>x, 2: a<x, 3: |a|>x, 4: |a|<x,
    /// 5: AND, 6: OR, 7: XOR, 8: a=b, 9: a>b, 10: a<b
    pub func: u8,
    /// Primary source index (`0..=57` for value functions) or switch condition (`0..=44` for boolean functions)
    pub v1: u8,
    /// Threshold (`-100..=100%` for `1..=4`) or secondary source/switch index (`0..=57`/`0..=44` for `5..=10`)
    pub v2: i8,
    /// Optional secondary AND switch condition (`0` = None, `1..=44` = switch condition)
    pub and_sw: u8,
    /// Activation delay in `0.1s` steps (`0..=100` = `0.0..10.0s`)
    pub delay_tenths: u8,
    /// Pulse duration in `0.1s` steps (`0` = continuous while condition holds, `1..=100` = `0.1..10.0s` pulse)
    pub duration_tenths: u8,
}

impl LogicalSwitch {
    pub const fn disabled() -> Self {
        Self {
            func: 0,
            v1: 0,
            v2: 0,
            and_sw: 0,
            delay_tenths: 0,
            duration_tenths: 0,
        }
    }

    #[inline(always)]
    pub const fn is_boolean_func(&self) -> bool {
        matches!(self.func, 5..=7)
    }

    #[inline(always)]
    pub const fn is_dual_source_func(&self) -> bool {
        matches!(self.func, 8..=10)
    }
}

pub const NUM_MULTISWITCH_SLOTS: usize = 4;
pub const NUM_MULTISWITCH_SWITCHES: usize = 8;
pub const MULTISWITCH_LABEL_LEN: usize = 4;

/// Default 4-char ASCII switch labels for a MultiSwitch slot (`"SW1 "`..`"SW8 "`).
pub const fn default_multiswitch_labels() -> [[[u8; MULTISWITCH_LABEL_LEN]; NUM_MULTISWITCH_SWITCHES]; NUM_MULTISWITCH_SLOTS] {
    let slot = [
        *b"SW1 ", *b"SW2 ", *b"SW3 ", *b"SW4 ", *b"SW5 ", *b"SW6 ", *b"SW7 ", *b"SW8 ",
    ];
    [slot; NUM_MULTISWITCH_SLOTS]
}

/// Complete profile for an individual aircraft/model (SCHEMA_VERSION = 2, exactly 760 bytes).
/// Fits cleanly inside `MODEL_SLOT_SIZE` (`768 B` = `8 B` header + `760 B` payload) with
/// natural alignment and a 12-byte `_reserved` tail for v1.x extensions.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ModelConfig {
    // --- 4-byte aligned group (16 B) ---
    pub rx_id: u32,           // 0..4: Model Match bound receiver ID
    pub channel_reverse: u32, // 4..8: 18-bit channel reversing mask
    pub motor_mask: u32,      // 8..12: 18-bit motor safety mask (default bit 2 = CH3)
    pub bidir_mask: u32,      // 12..16: 18-bit bidirectional motor mask (neutral = 1500 µs)

    // --- 2-byte aligned group (42 B) ---
    pub gvars: [i16; 5],                                    // 16..26: Global Variables GV1..GV5 (-1000..+1000)
    pub multiswitch_addresses: [i16; NUM_MULTISWITCH_SLOTS], // 26..34: CRSF MultiSwitch slot addresses (-1 = disabled)
    pub timer_secs: u16,                                    // 34..36: Timer 1 target in seconds
    pub failsafe_thr: u16,                                  // 36..38: Failsafe throttle pulse in µs
    pub servo_rate_hz: u16,                                 // 38..40: Receiver servo refresh rate (50..400 Hz)
    pub timers_ext: [crate::timer::TimerConfig; 3],         // 40..58: Independent Timers 2..4 (3 * 6 = 18 B)

    // --- Identity, RF, Safety, Timers, Vario, Trims, Curves, Soaring (70 B) ---
    pub name: [u8; 10],           // 58..68: 10-char ASCII model name
    pub model_type: u8,           // 68: 0: Airplane, 1: Glider, 2: Heli, 3: Quad, 4: General, 5: Baitboat
    pub rf_protocol: u8,          // 69: 0: AFHDS 2A, 1: CRSF, 2: CC2500, 3: DSM, 4: MPM
    pub rf_module_target: u8,     // 70: 0: Internal Module, 1: External Bay
    pub rf_power: u8,             // 71: AFHDS 2A RF power (0: 100mW, 1: 10mW, 2: 1mW, 3: 100uW)
    pub crsf_baud: u8,            // 72: 0: 115.2k, 1: 416.6k, 2: 420k, 3: 921.6k, 4: 1.875M
    pub crsf_half_duplex: u8,     // 73: 0: Full-Duplex, 1: Half-Duplex (S.Port Pin 5)
    pub crsf_arm_mode: u8,        // 74: 0: CH5, 1: Switch (ELRS 4.0)
    pub dsm_sub_proto: u8,        // 75: DSM sub-protocol (0..3)
    pub mpm_rx_num: u8,           // 76: MPM/CC2500 Receiver Number (0..15)
    pub mpm_range_check: u8,      // 77: MPM/CC2500 Range Check active (transient, cleared on boot)
    pub mpm_freq_tune: i8,        // 78: MPM/CC2500 RF frequency fine-tuning (-127..+127)
    pub rx_out_mode: u8,          // 79: AFHDS2A PWM/PPM or MPM/CC2500 sub-protocol index
    pub rx_serial_proto: u8,      // 80: AFHDS2A i-BUS/S.BUS or MPM/CC2500 sub-type index
    pub failsafe_mode: u8,        // 81: 0: Hold last, 1: Custom pulses
    pub failsafe_timeout: u8,     // 82: 10..50 (1.0s..5.0s)
    pub arm_switch: u8,           // 83: 0: None, 1..=44: SA^..SWMv, L1..!L8
    pub sd_log_mode: u8,          // 84: SD telemetry logging (0: Off, 1: Armed, 2..=45: switch)
    pub timer_source: u8,         // 85: Timer 1 start source
    pub timer1_stop_src: u8,      // 86: Timer 1 stop switch
    pub timer1_reset_src: u8,     // 87: Timer 1 reset switch
    pub timer1_flags: u8,         // 88: Timer 1 bitflags
    pub vario_switch: u8,         // 89: 0: Off, 1..=45: switch/mode
    pub vario_climb_deadband: u8, // 90: Climb deadband in 0.1 m/s
    pub vario_sink_deadband: u8,  // 91: Sink deadband in 0.1 m/s
    pub vario_sink_mode: u8,      // 92: 0: Mute, 1: Pulse, 2: Continuous
    pub vario_motor_mute: u8,     // 93: 0: Disabled, 1: Enabled
    pub telem_slots: [u8; 4],     // 94..98: Custom telemetry dashboard 4 tiles
    pub trims: [i8; 4],           // 98..102: Roll, Pitch, Throttle, Yaw (-25..+25)
    pub trim_map: [u8; 4],        // 102..106: Target input axis for each physical trim (1..4, 0=off)
    pub trim_mode: u8,            // 106: Throttle trim mode (0: Off/Lock, 1: Idle T-Trim, 2: Linear)
    pub thr_curve_pts: u8,        // 107: 5, 7, or 9 points
    pub thr_curve_smooth: u8,     // 108: 0: Linear, 1: Catmull-Rom Spline
    pub thr_curve: [u8; 9],       // 109..118: Curve points (0..100%)
    pub soaring: crate::soaring::SoaringConfig, // 118..130 (Wait: let's check exact offset!)

    // --- Mixer v2 arrays (488 B) ---
    pub channels: [crate::mixer::ChannelCfg; crate::mixer::NUM_CHANNELS], // 72 B
    pub input_mods: [crate::mixer::InputMod; crate::mixer::NUM_INPUT_MODS], // 64 B
    pub mixers: [crate::mixer::MixerInst; crate::mixer::NUM_PREDEF_MIXERS], // 32 B
    pub chan_mods: [crate::mixer::ChanMod; crate::mixer::NUM_CHAN_MODS],    // 16 B
    pub mixes: [MixLine; NUM_MIX_LINES],                                    // 256 B
    pub logical_switches: [LogicalSwitch; NUM_LOGICAL_SWITCHES],            // 48 B

    // --- PR #37 MultiSwitch (132 B) ---
    pub multiswitch_momentary: [u8; NUM_MULTISWITCH_SLOTS], // 4 B
    pub multiswitch_labels: [[[u8; MULTISWITCH_LABEL_LEN]; NUM_MULTISWITCH_SWITCHES]; NUM_MULTISWITCH_SLOTS], // 128 B

    // --- Reserved v1.x extension tail (10 B) ---
    pub _reserved: [u8; 10],
}

const _: () = assert!(core::mem::size_of::<ModelConfig>() == 760);
const _: () = assert!(HEADER_SIZE + core::mem::size_of::<ModelConfig>() <= MODEL_SLOT_SIZE);

impl ModelConfig {
    pub const fn default_for_index(idx: usize) -> Self {
        let num = (idx + 1) as u8;
        let digit1 = b'0' + (num / 10);
        let digit2 = b'0' + (num % 10);
        Self {
            rx_id: 0,
            channel_reverse: 0,
            motor_mask: 1 << 2, // CH3 is default motor channel
            bidir_mask: 0,
            gvars: [0; 5],
            multiswitch_addresses: [-1; NUM_MULTISWITCH_SLOTS],
            timer_secs: 300,
            failsafe_thr: 1000,
            servo_rate_hz: 50,
            timers_ext: [
                crate::timer::TimerConfig::disabled(),
                crate::timer::TimerConfig::disabled(),
                crate::timer::TimerConfig::disabled(),
            ],
            name: [
                b'M', b'O', b'D', b'E', b'L', b' ', digit1, digit2, b' ', b' ',
            ],
            model_type: 0,
            rf_protocol: crate::board::ActiveBoard::DEFAULT_PROTOCOL as u8,
            rf_module_target: 0,
            rf_power: 0,
            crsf_baud: 2,
            crsf_half_duplex: 0,
            crsf_arm_mode: 0,
            dsm_sub_proto: 0,
            mpm_rx_num: 0,
            mpm_range_check: 0,
            mpm_freq_tune: 0,
            rx_out_mode: 0,
            rx_serial_proto: 0,
            failsafe_mode: 0,
            failsafe_timeout: 20,
            arm_switch: 0,
            sd_log_mode: 0,
            timer_source: 1,
            timer1_stop_src: 0,
            timer1_reset_src: 0,
            timer1_flags: crate::timer::TimerConfig::FLAG_AUDIO_BEEPS,
            vario_switch: 0,
            vario_climb_deadband: 2,
            vario_sink_deadband: 5,
            vario_sink_mode: 1,
            vario_motor_mute: 1,
            telem_slots: [0, 1, 2, 3],
            trims: [0, 0, 0, 0],
            trim_map: [1, 2, 3, 4], // Roll, Pitch, Thr, Yaw
            trim_mode: 0,           // OFF (Lock)
            thr_curve_pts: 5,
            thr_curve_smooth: 0,
            thr_curve: [0, 25, 50, 75, 100, 0, 0, 0, 0],
            soaring: crate::soaring::SoaringConfig::default_factory(),
            channels: [
                crate::mixer::ChannelCfg::new(1),  // CH1: Roll
                crate::mixer::ChannelCfg::new(2),  // CH2: Pitch
                crate::mixer::ChannelCfg::new(58), // CH3: Curved Throttle (THC)
                crate::mixer::ChannelCfg::new(4),  // CH4: Yaw
                crate::mixer::ChannelCfg::new(7),  // CH5: SA
                crate::mixer::ChannelCfg::new(8),  // CH6: SB
                crate::mixer::ChannelCfg::new(5),  // CH7: VRA
                crate::mixer::ChannelCfg::new(6),  // CH8: VRB
                crate::mixer::ChannelCfg::new(9),  // CH9: SC
                crate::mixer::ChannelCfg::new(10), // CH10: SD
                crate::mixer::ChannelCfg::new(0),  // CH11..CH18: None
                crate::mixer::ChannelCfg::new(0),
                crate::mixer::ChannelCfg::new(0),
                crate::mixer::ChannelCfg::new(0),
                crate::mixer::ChannelCfg::new(0),
                crate::mixer::ChannelCfg::new(0),
                crate::mixer::ChannelCfg::new(0),
                crate::mixer::ChannelCfg::new(0),
            ],
            input_mods: [
                crate::mixer::InputMod::default_axis(1), // Roll DR/Expo
                crate::mixer::InputMod::default_axis(2), // Pitch DR/Expo
                crate::mixer::InputMod::default_axis(4), // Yaw DR/Expo
                crate::mixer::InputMod::disabled(),
                crate::mixer::InputMod::disabled(),
                crate::mixer::InputMod::disabled(),
                crate::mixer::InputMod::disabled(),
                crate::mixer::InputMod::disabled(),
            ],
            mixers: [crate::mixer::MixerInst::disabled(); crate::mixer::NUM_PREDEF_MIXERS],
            chan_mods: [crate::mixer::ChanMod::disabled(); crate::mixer::NUM_CHAN_MODS],
            mixes: [MixLine::disabled(); NUM_MIX_LINES],
            logical_switches: [LogicalSwitch::disabled(); NUM_LOGICAL_SWITCHES],
            multiswitch_momentary: [0; NUM_MULTISWITCH_SLOTS],
            multiswitch_labels: default_multiswitch_labels(),
            _reserved: [0; 10],
        }
    }

    /// Get MPM/CC2500 Receiver Number (`0..15`).
    #[inline(always)]
    pub const fn mpm_rx_num(&self) -> u8 {
        self.mpm_rx_num & 0x0F
    }

    /// Set MPM/CC2500 Receiver Number (`0..15`), preserving D16 bind option bits `5:4`.
    #[inline(always)]
    pub fn set_mpm_rx_num(&mut self, rx_num: u8) {
        self.mpm_rx_num = (self.mpm_rx_num & 0xF0) | (rx_num & 0x0F);
    }

    /// Get FrSky D16 (`FrSkyX` / `FrSkyX2`) bind option (`0` = Ch1-8 Tel ON, `1` = Ch1-8 Tel OFF,
    /// `2` = Ch9-16 Tel ON, `3` = Ch9-16 Tel OFF), stored in bits `5:4` of `mpm_rx_num`.
    #[inline(always)]
    pub const fn mpm_d16_bind_opt(&self) -> u8 {
        (self.mpm_rx_num >> 4) & 0x03
    }

    /// Set FrSky D16 bind option (`0..=3`), preserving Receiver Number bits `3:0`.
    #[inline(always)]
    pub fn set_mpm_d16_bind_opt(&mut self, opt: u8) {
        self.mpm_rx_num = (self.mpm_rx_num & 0x0F) | ((opt & 0x03) << 4);
    }

    /// Check if MPM/CC2500 reduced-power Range Check mode is active.
    #[inline(always)]
    pub const fn mpm_range_check(&self) -> bool {
        self.mpm_range_check != 0
    }

    /// Set MPM/CC2500 reduced-power Range Check mode.
    #[inline(always)]
    pub fn set_mpm_range_check(&mut self, enabled: bool) {
        self.mpm_range_check = if enabled { 1 } else { 0 };
    }

    /// Get RF Module Target (`0` = Internal Module, `1` = External Module Bay).
    #[inline(always)]
    pub const fn rf_module_target(&self) -> u8 {
        self.rf_module_target & 0x01
    }

    /// Set RF Module Target (`0` = Internal Module, `1` = External Module Bay).
    #[inline(always)]
    pub fn set_rf_module_target(&mut self, target: u8) {
        self.rf_module_target = target & 0x01;
    }

    /// Get MPM/CC2500 RF Frequency Fine-Tuning offset (`-127..+127`).
    #[inline(always)]
    pub const fn mpm_freq_tune(&self) -> i8 {
        self.mpm_freq_tune
    }

    /// Set MPM/CC2500 RF Frequency Fine-Tuning offset (`-127..+127`).
    #[inline(always)]
    pub fn set_mpm_freq_tune(&mut self, tune: i8) {
        self.mpm_freq_tune = tune.clamp(-127, 127);
    }

    /// Read source assigned to a logical channel (`0..17` for CH1..CH18).
    #[inline(always)]
    pub fn channel_source(&self, ch_idx: usize) -> u8 {
        if ch_idx < crate::mixer::NUM_CHANNELS {
            self.channels[ch_idx].source
        } else {
            0
        }
    }

    /// Set source assigned to a logical channel (`0..17` for CH1..CH18).
    #[inline(always)]
    pub fn set_channel_source(&mut self, ch_idx: usize, src: u8) {
        if ch_idx < crate::mixer::NUM_CHANNELS {
            self.channels[ch_idx].source = src;
        }
    }

    /// Check if channel `ch_idx` (`0..17`) is flagged as a motor channel.
    #[inline(always)]
    pub const fn is_motor_channel(&self, ch_idx: usize) -> bool {
        ch_idx < crate::mixer::NUM_CHANNELS && (self.motor_mask & (1u32 << ch_idx)) != 0
    }

    /// Check if channel `ch_idx` (`0..17`) is a bidirectional motor channel (neutral = 1500 µs).
    #[inline(always)]
    pub const fn is_bidir_motor(&self, ch_idx: usize) -> bool {
        ch_idx < crate::mixer::NUM_CHANNELS && (self.bidir_mask & (1u32 << ch_idx)) != 0
    }

    /// Set channel motor safety mode: `0 = Off`, `1 = Motor (Unidirectional)`, `2 = Motor (Bidirectional)`.
    #[inline]
    pub fn set_motor_mode(&mut self, ch_idx: usize, mode: u8) {
        if ch_idx >= crate::mixer::NUM_CHANNELS {
            return;
        }
        let bit = 1u32 << ch_idx;
        match mode {
            1 => {
                self.motor_mask |= bit;
                self.bidir_mask &= !bit;
            }
            2 => {
                self.motor_mask |= bit;
                self.bidir_mask |= bit;
            }
            _ => {
                self.motor_mask &= !bit;
                self.bidir_mask &= !bit;
            }
        }
    }

    /// Get channel motor safety mode (`0 = Off`, `1 = Motor`, `2 = Bidir`).
    #[inline(always)]
    pub const fn motor_mode(&self, ch_idx: usize) -> u8 {
        if !self.is_motor_channel(ch_idx) {
            0
        } else if self.is_bidir_motor(ch_idx) {
            2
        } else {
            1
        }
    }

    /// Return the primary predefined mixer kind (instance 0), if any.
    #[inline(always)]
    pub const fn primary_mixer_kind(&self) -> crate::mixer::MixerKind {
        crate::mixer::MixerKind::from_u8(self.mixers[0].kind)
    }

    /// Apply a standard aircraft/vehicle predefined mixer preset to instance 0 and wire CH1..CH6.
    pub fn apply_mixer_preset(&mut self, kind: crate::mixer::MixerKind) {
        use crate::mixer::{MixerInst, MixerKind, MIXER_OUT_SRC_BASE};
        self.mixers[0] = MixerInst::default_for_kind(kind);
        match kind {
            MixerKind::None => {
                self.channels[0].source = 1;  // Roll
                self.channels[1].source = 2;  // Pitch
                self.channels[2].source = 58; // THC
                self.channels[3].source = 4;  // Yaw
                self.motor_mask = 1 << 2;
                self.bidir_mask = 0;
            }
            MixerKind::Delta => {
                self.channels[0].source = MIXER_OUT_SRC_BASE;     // Left Elevon
                self.channels[1].source = MIXER_OUT_SRC_BASE + 1; // Right Elevon
                self.channels[2].source = 58;                     // THC
                self.channels[3].source = 4;                      // Yaw
                self.motor_mask = 1 << 2;
                self.bidir_mask = 0;
            }
            MixerKind::VTail | MixerKind::MTail => {
                self.channels[0].source = 1;                      // Roll
                self.channels[1].source = MIXER_OUT_SRC_BASE;     // Left V/M-Tail
                self.channels[2].source = 58;                     // THC
                self.channels[3].source = MIXER_OUT_SRC_BASE + 1; // Right V/M-Tail
                self.motor_mask = 1 << 2;
                self.bidir_mask = 0;
            }
            MixerKind::Flaperon => {
                self.channels[0].source = MIXER_OUT_SRC_BASE;     // Left Flaperon
                self.channels[1].source = 2;                      // Pitch
                self.channels[2].source = 58;                     // THC
                self.channels[3].source = 4;                      // Yaw
                self.channels[5].source = MIXER_OUT_SRC_BASE + 1; // Right Flaperon (CH6)
                self.motor_mask = 1 << 2;
                self.bidir_mask = 0;
            }
            MixerKind::DiffThrust => {
                self.channels[0].source = MIXER_OUT_SRC_BASE;     // Left Motor
                self.channels[1].source = MIXER_OUT_SRC_BASE + 1; // Right Motor
                self.channels[2].source = 58;
                self.channels[3].source = 4;
                self.motor_mask = (1 << 0) | (1 << 1);
                self.bidir_mask = 0;
            }
            MixerKind::Tank => {
                self.channels[0].source = MIXER_OUT_SRC_BASE;     // Left Track
                self.channels[1].source = MIXER_OUT_SRC_BASE + 1; // Right Track
                self.channels[2].source = 3;
                self.channels[3].source = 4;
                self.motor_mask = (1 << 0) | (1 << 1);
                self.bidir_mask = (1 << 0) | (1 << 1);
            }
            MixerKind::Crow => {
                self.channels[0].source = MIXER_OUT_SRC_BASE;     // AilL
                self.channels[5].source = MIXER_OUT_SRC_BASE + 1; // AilR (CH6)
                self.channels[1].source = 2;                      // Pitch (add EleC via mix or source)
            }
        }
    }

    /// Extract complete TimerConfig for Timer 1..4 (index 0..3).
    #[inline]
    pub fn timer_config(&self, idx: usize) -> crate::timer::TimerConfig {
        match idx {
            0 => crate::timer::TimerConfig {
                secs: self.timer_secs,
                start_src: self.timer_source,
                stop_src: self.timer1_stop_src,
                reset_src: self.timer1_reset_src,
                flags: self.timer1_flags,
            },
            1..=3 => self.timers_ext[idx - 1],
            _ => crate::timer::TimerConfig::disabled(),
        }
    }

    /// Set TimerConfig for Timer 1..4 (index 0..3).
    #[inline]
    pub fn set_timer_config(&mut self, idx: usize, cfg: crate::timer::TimerConfig) {
        match idx {
            0 => {
                self.timer_secs = cfg.secs;
                self.timer_source = cfg.start_src;
                self.timer1_stop_src = cfg.stop_src;
                self.timer1_reset_src = cfg.reset_src;
                self.timer1_flags = cfg.flags;
            }
            1..=3 => {
                self.timers_ext[idx - 1] = cfg;
            }
            _ => {}
        }
    }

    /// Build array of all 4 TimerConfigs.
    #[inline]
    pub fn all_timer_configs(&self) -> [crate::timer::TimerConfig; 4] {
        [
            self.timer_config(0),
            self.timer_config(1),
            self.timer_config(2),
            self.timer_config(3),
        ]
    }

    #[inline(always)]
    pub fn model_type(&self) -> crate::safety::ModelType {
        crate::safety::ModelType::from_u8(self.model_type)
    }

    #[inline(always)]
    pub fn set_model_type(&mut self, mtype: crate::safety::ModelType) {
        self.model_type = mtype.to_u8();
    }
}

/// Compact storage layout containing radio settings and active model (488 bytes).
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct RadioStorage {
    pub radio: RadioConfig,
    pub active_model: ModelConfig,
}

impl RadioStorage {
    pub const fn empty() -> Self {
        unsafe { core::mem::MaybeUninit::zeroed().assume_init() }
    }

    #[allow(dead_code)]
    pub const fn default_factory() -> Self {
        Self {
            radio: RadioConfig::default_factory(),
            active_model: ModelConfig::default_for_index(0),
        }
    }

    pub fn init_default(&mut self) {
        self.radio = RadioConfig::default_factory();
        self.active_model = ModelConfig::default_for_index(0);
    }

    #[inline(always)]
    pub fn active_model(&self) -> &ModelConfig {
        &self.active_model
    }

    #[inline(always)]
    pub fn active_model_mut(&mut self) -> &mut ModelConfig {
        &mut self.active_model
    }

    /// Check if a mix source ID (`0..=71`) is available on this radio.
    /// Filters out unconfigured switches, potentiometers, and unmapped channels.
    #[inline]
    pub fn is_source_available(&self, src: u8) -> bool {
        if (50..=58).contains(&src) || (60..=71).contains(&src) {
            return true;
        }
        crate::board::ActiveBoard::is_source_available(&self.radio, &self.active_model, src)
    }

    /// Check if a switch condition index (0..44, matching `SWITCH_COND_NAMES`) is valid given hardware config.
    #[inline]
    pub fn is_switch_cond_available(&self, cond: u8) -> bool {
        if (29..=44).contains(&cond) {
            return true;
        }
        crate::board::ActiveBoard::is_switch_cond_available(&self.radio, cond)
    }

    /// Check if a channel index (1..=18) is configured / active in the active model.
    pub fn is_channel_configured(&self, ch: u8) -> bool {
        if ch == 0 || ch > 18 {
            return false;
        }
        let ch_idx = (ch - 1) as usize;
        if self.active_model.channels[ch_idx].source != 0 {
            return true;
        }
        self.active_model.mixes.iter().any(|m| m.target_ch == ch)
    }

    /// Check if a weight modulation source index (0..=41, matching `WEIGHT_SRC_NAMES`) is available.
    pub fn is_weight_src_available(&self, ws: u8) -> bool {
        match ws {
            0 => true,                                      // Fixed weight
            1..=18 => self.is_channel_configured(ws),       // Bipolar CH1..CH18
            19..=36 => self.is_channel_configured(ws - 18), // Unipolar CH1+..CH18+
            37..=41 => true, // GV1..GV5 (Global Variables) are always available
            _ => false,
        }
    }

    /// Find the next available mix source index, wrapping around.
    fn next_matching<F>(&self, cur: u8, max_len: usize, pred: F) -> u8
    where
        F: Fn(u8) -> bool,
    {
        let max = max_len.min(u8::MAX as usize) as u8;
        if max == 0 {
            return cur;
        }
        let start = if cur < max { cur } else { 0 };
        for step in 1..=max {
            let candidate = ((start as u16 + step as u16) % max as u16) as u8;
            if pred(candidate) {
                return candidate;
            }
        }
        cur
    }

    fn prev_matching<F>(&self, cur: u8, max_len: usize, pred: F) -> u8
    where
        F: Fn(u8) -> bool,
    {
        let max = max_len.min(u8::MAX as usize) as u8;
        if max == 0 {
            return cur;
        }
        let start = if cur < max { cur } else { 0 };
        for step in 1..=max {
            let candidate = ((start as u16 + max as u16 - step as u16) % max as u16) as u8;
            if pred(candidate) {
                return candidate;
            }
        }
        cur
    }

    /// Find the next available mix source index, wrapping around.
    pub fn next_available_source(&self, cur: u8, max_len: usize) -> u8 {
        self.next_matching(cur, max_len, |c| self.is_source_available(c))
    }

    /// Find the previous available mix source index, wrapping around.
    pub fn prev_available_source(&self, cur: u8, max_len: usize) -> u8 {
        self.prev_matching(cur, max_len, |c| self.is_source_available(c))
    }

    /// Find the next available weight modulation source index, wrapping around.
    pub fn next_available_weight_src(&self, cur: u8, max_len: usize) -> u8 {
        self.next_matching(cur, max_len, |c| self.is_weight_src_available(c))
    }

    /// Find the previous available weight modulation source index, wrapping around.
    pub fn prev_available_weight_src(&self, cur: u8, max_len: usize) -> u8 {
        self.prev_matching(cur, max_len, |c| self.is_weight_src_available(c))
    }

    /// Find the next available switch condition index, wrapping around.
    pub fn next_available_switch_cond(&self, cur: u8, max_len: usize) -> u8 {
        self.next_matching(cur, max_len, |c| self.is_switch_cond_available(c))
    }

    /// Find the previous available switch condition index, wrapping around.
    pub fn prev_available_switch_cond(&self, cur: u8, max_len: usize) -> u8 {
        self.prev_matching(cur, max_len, |c| self.is_switch_cond_available(c))
    }

    /// Check if a physical switch index (0..6: None, SA..SF) is available for Dual Rate selection.
    pub fn is_dr_switch_available(&self, sw: u8) -> bool {
        match sw {
            0 => true, // None
            1 => matches!(
                self.radio.get_adc_mode(0),
                AdcInputMode::TwoPos | AdcInputMode::ThreePos | AdcInputMode::InstantTrim
            ), // SA
            2 => matches!(
                self.radio.get_adc_mode(1),
                AdcInputMode::TwoPos | AdcInputMode::ThreePos | AdcInputMode::InstantTrim
            ), // SB
            3 => matches!(
                self.radio.get_adc_mode(2),
                AdcInputMode::TwoPos | AdcInputMode::ThreePos | AdcInputMode::InstantTrim
            ), // SC
            4 => matches!(
                self.radio.get_adc_mode(3),
                AdcInputMode::TwoPos | AdcInputMode::ThreePos | AdcInputMode::InstantTrim
            ), // SD
            5 => self.is_source_available(31), // SE
            6 => self.is_source_available(32), // SF
            _ => false,
        }
    }

    /// Find next available Dual Rate switch (0..6).
    pub fn next_available_dr_switch(&self, cur: u8) -> u8 {
        let mut candidate = (cur + 1) % 7;
        while candidate != cur {
            if self.is_dr_switch_available(candidate) {
                return candidate;
            }
            candidate = (candidate + 1) % 7;
        }
        cur
    }

    /// Find previous available Dual Rate switch (0..6).
    pub fn prev_available_dr_switch(&self, cur: u8) -> u8 {
        let mut candidate = if cur == 0 { 6 } else { cur - 1 };
        while candidate != cur {
            if self.is_dr_switch_available(candidate) {
                return candidate;
            }
            candidate = if candidate == 0 { 6 } else { candidate - 1 };
        }
        cur
    }

    /// Check if an AUX channel input source (from `crate::board::BoardProfile::AUX_SOURCES`) is hardware-available.
    pub fn is_aux_source_available(&self, src: u8) -> bool {
        match src {
            0..=4 => true,                            // None, Roll, Pitch, Thr, Yaw
            5 => self.radio.get_adc_mode(4).is_pot(), // VRA
            6 => self.is_source_available(6),         // VRB / S2
            7 => matches!(
                self.radio.get_adc_mode(0),
                AdcInputMode::TwoPos | AdcInputMode::ThreePos | AdcInputMode::InstantTrim
            ), // SA
            8 => matches!(
                self.radio.get_adc_mode(1),
                AdcInputMode::TwoPos | AdcInputMode::ThreePos | AdcInputMode::InstantTrim
            ), // SB
            9 => matches!(
                self.radio.get_adc_mode(2),
                AdcInputMode::TwoPos | AdcInputMode::ThreePos | AdcInputMode::InstantTrim
            ), // SC
            10 => matches!(
                self.radio.get_adc_mode(3),
                AdcInputMode::TwoPos | AdcInputMode::ThreePos | AdcInputMode::InstantTrim
            ), // SD
            31..=34 => self.is_source_available(src), // SE, SF, SG, SH
            35 => {
                self.radio.ext_adc != 0
                    && matches!(
                        self.radio.get_adc_mode(6),
                        AdcInputMode::TwoPos | AdcInputMode::ThreePos
                    )
            } // SWI
            36 => {
                self.radio.ext_adc != 0
                    && matches!(
                        self.radio.get_adc_mode(7),
                        AdcInputMode::TwoPos | AdcInputMode::ThreePos
                    )
            } // SWJ
            37 => self.radio.ext_adc != 0 && self.radio.get_adc_mode(6).is_pot(), // VRC
            38 => self.radio.ext_adc != 0 && self.radio.get_adc_mode(7).is_pot(), // VRD
            39 => self.radio.ext_adc != 0 && self.radio.get_adc_mode(8).is_pot(), // VRE
            40 => self.radio.ext_adc != 0 && self.radio.get_adc_mode(9).is_pot(), // VRF
            41 => self.radio.ext_adc != 0 && self.radio.get_adc_mode(10).is_pot(), // VRG
            42 => {
                self.radio.ext_adc != 0
                    && matches!(
                        self.radio.get_adc_mode(8),
                        AdcInputMode::TwoPos | AdcInputMode::ThreePos
                    )
            } // SWK
            43 => {
                self.radio.ext_adc != 0
                    && matches!(
                        self.radio.get_adc_mode(9),
                        AdcInputMode::TwoPos | AdcInputMode::ThreePos
                    )
            } // SWL
            44 => {
                self.radio.ext_adc != 0
                    && matches!(
                        self.radio.get_adc_mode(10),
                        AdcInputMode::TwoPos | AdcInputMode::ThreePos
                    )
            } // SWM
            50..=57 => true, // L1..L8 Logical Switches
            _ => false,
        }
    }

    /// Cycle to next available AUX source in a given slice of source IDs.
    pub fn next_available_aux_source(&self, cur: u8, sources: &[u8]) -> u8 {
        let cur_pos = sources.iter().position(|&s| s == cur).unwrap_or(0);
        let len = sources.len();
        let mut idx = (cur_pos + 1) % len;
        while idx != cur_pos {
            let cand = sources[idx];
            if self.is_aux_source_available(cand) {
                return cand;
            }
            idx = (idx + 1) % len;
        }
        cur
    }

    /// Cycle to previous available AUX source in a given slice of source IDs.
    pub fn prev_available_aux_source(&self, cur: u8, sources: &[u8]) -> u8 {
        let cur_pos = sources.iter().position(|&s| s == cur).unwrap_or(0);
        let len = sources.len();
        let mut idx = if cur_pos == 0 { len - 1 } else { cur_pos - 1 };
        while idx != cur_pos {
            let cand = sources[idx];
            if self.is_aux_source_available(cand) {
                return cand;
            }
            idx = if idx == 0 { len - 1 } else { idx - 1 };
        }
        cur
    }

    /// Check if an Arm Switch condition (0..=28: 0: None, 1..=28: SA^..SWMv) is available.
    pub fn is_arm_switch_available(&self, sw: u8) -> bool {
        match sw {
            0 => true, // None
            1..=28 => self.is_switch_cond_available(sw),
            _ => false,
        }
    }

    /// Cycle to next available Arm Switch (0..=28).
    pub fn next_available_arm_switch(&self, cur: u8) -> u8 {
        let mut candidate = (cur + 1) % 29;
        while candidate != cur {
            if self.is_arm_switch_available(candidate) {
                return candidate;
            }
            candidate = (candidate + 1) % 29;
        }
        cur
    }

    /// Cycle to previous available Arm Switch (0..=28).
    pub fn prev_available_arm_switch(&self, cur: u8) -> u8 {
        let mut candidate = if cur == 0 { 28 } else { cur - 1 };
        while candidate != cur {
            if self.is_arm_switch_available(candidate) {
                return candidate;
            }
            candidate = if candidate == 0 { 28 } else { candidate - 1 };
        }
        cur
    }

    /// Check if a Timer Source (0..=31: 0: Off, 1: THs, 2: THt, 3: Always On, 4..=31: SA^..SWMv) is available.
    pub fn is_timer_source_available(&self, ts: u8) -> bool {
        match ts {
            0..=3 => true, // Off, THs, THt, Always On
            4..=31 => self.is_switch_cond_available(ts - 3),
            _ => false,
        }
    }

    /// Cycle to next available Timer Source (0..=31).
    pub fn next_available_timer_source(&self, cur: u8) -> u8 {
        self.next_matching(cur, 32, |c| self.is_timer_source_available(c))
    }

    /// Cycle to previous available Timer Source (0..=31).
    pub fn prev_available_timer_source(&self, cur: u8) -> u8 {
        self.prev_matching(cur, 32, |c| self.is_timer_source_available(c))
    }

    /// Sanitize all radio and model parameters to valid operating bounds.
    pub fn sanitize(&mut self) {
        self.radio.magic = FLASH_MAGIC;
        self.radio.version = CONFIG_VERSION;
        if self.radio.vbat_warn_deci < crate::board::ActiveBoard::VBAT_WARN_MIN_DECI
            || self.radio.vbat_warn_deci > crate::board::ActiveBoard::VBAT_WARN_MAX_DECI
        {
            self.radio.vbat_warn_deci = crate::board::ActiveBoard::VBAT_WARN_DEFAULT_DECI;
        }
        if self.radio.active_model >= NUM_MODELS as u8 {
            self.radio.active_model = 0;
        }
        if self.radio.backlight_brightness == 0 || self.radio.backlight_brightness > 10 {
            self.radio.backlight_brightness = 10;
        }
        if self.radio.lcd_contrast < 15 || self.radio.lcd_contrast > 55 {
            self.radio.lcd_contrast = 37;
        }
        if self.radio.usb_mode > 3 {
            self.radio.usb_mode = 0;
        }
        if self.radio.ext_module_pwr > 1 {
            self.radio.ext_module_pwr = 0;
        }
        if self.radio.tone_style > 1 {
            self.radio.tone_style = 1;
        }
        if self.radio.servo_rate_hz < 50 || self.radio.servo_rate_hz > 400 {
            self.radio.servo_rate_hz = 50;
        }
        if self.radio.rx_out_mode > 1 {
            self.radio.rx_out_mode = 0;
        }
        if self.radio.rx_serial_proto > 1 {
            self.radio.rx_serial_proto = 0;
        }
        if self.radio.ext_switches > 1 {
            self.radio.ext_switches = 0;
        }
        if self.radio.rear_left_func > 2 {
            self.radio.rear_left_func = 0;
        }
        if self.radio.rear_right_func > 2 {
            self.radio.rear_right_func = 0;
        }
        if self.radio.touch_disabled > 1 {
            self.radio.touch_disabled = 0;
        }
        if self.radio.j4_sg_sh_en > 1 {
            self.radio.j4_sg_sh_en = 0;
        }
        if self.radio.switch_beep > 1 {
            self.radio.switch_beep = 1;
        }
        if self.radio.gimbal_type > 1 {
            self.radio.gimbal_type = 0;
        }
        if self.radio.stick_invert_mask > 0x0F {
            self.radio.stick_invert_mask = 0x03; // Default RH+RV inverted
        }
        if (self.radio.voice_cfg & 0x0F) > 11 {
            self.radio.voice_cfg &= 0xF0; // Default 80% voice volume
        }
        if ((self.radio.voice_cfg >> 4) as usize) >= RadioConfig::VOICE_LANGS.len() {
            self.radio.voice_cfg &= 0x0F; // Default "en" voice language
        }
        if self.radio.voice_interval > 4 {
            self.radio.voice_interval = 0; // Default 15s voice callout interval
        }

        for stick in self.radio.sticks.iter_mut() {
            if stick.min >= stick.center
                || stick.center >= stick.max
                || stick.max > crate::adc::ADC_MAX
            {
                stick.min = crate::adc::ADC_MIN;
                stick.center = crate::adc::ADC_CENTER;
                stick.max = crate::adc::ADC_MAX;
            }
        }
        if self.radio.ext_adc > 1 {
            self.radio.ext_adc = 0;
        }
        for pot in self.radio.aux_pots.iter_mut() {
            if pot.min >= pot.center || pot.center >= pot.max || pot.max > crate::adc::ADC_MAX {
                pot.min = crate::adc::ADC_MIN;
                pot.center = crate::adc::ADC_CENTER;
                pot.max = crate::adc::ADC_MAX;
            }
        }
        for mode in self.radio.adc_modes.iter_mut() {
            if *mode > 7 {
                *mode = 0; // Default
            }
        }
        if self.radio.trainer_config > 7 {
            self.radio.trainer_config = 0;
        } else if (self.radio.trainer_config & 0x06) == 0x06 {
            // PPM In and DSM SAT In are mutually exclusive
            self.radio.trainer_config &= !0x04;
        }

        let m = &mut self.active_model;
        for slot in m.telem_slots.iter_mut() {
            if *slot > 22 {
                *slot = 22; // OFF
            }
        }
        #[cfg(feature = "baitboat")]
        let max_model_type = 5;
        #[cfg(not(feature = "baitboat"))]
        let max_model_type = 4;
        if m.model_type > max_model_type {
            m.model_type = 0;
        }
        if m.rf_protocol > crate::board::RfProtocol::MAX_VALUE {
            m.rf_protocol = crate::board::ActiveBoard::DEFAULT_PROTOCOL as u8;
        }
        if m.rf_module_target > 1 {
            m.rf_module_target = 0;
        }
        if m.dsm_sub_proto > 3 {
            m.dsm_sub_proto = 0;
        }
        if m.crsf_baud > 4 {
            m.crsf_baud = 2;
        }
        if m.crsf_half_duplex > 1 {
            m.crsf_half_duplex = 0;
        }
        if m.trim_mode > 2 {
            m.trim_mode = 0;
        }
        for tm in m.trim_map.iter_mut() {
            if *tm > 4 {
                *tm = 0;
            }
        }
        for ch in m.channels.iter_mut() {
            ch.sanitize();
        }
        for im in m.input_mods.iter_mut() {
            im.sanitize();
        }
        for mx in m.mixers.iter_mut() {
            mx.sanitize();
        }
        for cm in m.chan_mods.iter_mut() {
            cm.sanitize();
        }
        if m.failsafe_thr < 900 || m.failsafe_thr > 2100 {
            m.failsafe_thr = 1000;
        }
        for mix in m.mixes.iter_mut() {
            if mix.target_ch > 18
                || mix.source > 71
                || mix.weight_src > 41
                || mix.mode > 2
                || mix.switch > 44
            {
                *mix = MixLine::disabled();
            }
        }
        for gv in m.gvars.iter_mut() {
            *gv = (*gv).clamp(-1000, 1000);
        }
        for addr in m.multiswitch_addresses.iter_mut() {
            if *addr < -1 || *addr > 255 {
                *addr = -1;
            }
        }
        if m.thr_curve_pts != 5 && m.thr_curve_pts != 7 && m.thr_curve_pts != 9 {
            m.thr_curve_pts = 5;
            m.thr_curve = [0, 25, 50, 75, 100, 0, 0, 0, 0];
        }
        if m.arm_switch > 44 {
            m.arm_switch = 0;
        }
        if m.crsf_arm_mode > 1 {
            m.crsf_arm_mode = 0;
        }
        if m.timer_source > 31 {
            m.timer_source = 0;
        }
        if m.timer_secs > 3600 {
            m.timer_secs = 0;
        }
        if m.timer1_stop_src > 44 {
            m.timer1_stop_src = 0;
        }
        if m.timer1_reset_src > 44 {
            m.timer1_reset_src = 0;
        }
        if m.timer1_flags > 0x07 {
            m.timer1_flags = crate::timer::TimerConfig::FLAG_AUDIO_BEEPS;
        }
        for timer in m.timers_ext.iter_mut() {
            if timer.secs > 3600 {
                timer.secs = 0;
            }
            if timer.start_src > 31 {
                timer.start_src = 0;
            }
            if timer.stop_src > 44 {
                timer.stop_src = 0;
            }
            if timer.reset_src > 44 {
                timer.reset_src = 0;
            }
            if timer.flags > 0x07 {
                timer.flags = crate::timer::TimerConfig::FLAG_AUDIO_BEEPS;
            }
        }
        if m.rf_power > 3 {
            m.rf_power = 0;
        }
        if m.servo_rate_hz < 50 || m.servo_rate_hz > 400 {
            m.servo_rate_hz = 50;
        }
        if m.rx_out_mode > 1 {
            m.rx_out_mode = 0;
        }
        let max_serial_proto = match m.rf_protocol {
            2 => crate::rf::cc2500::Cc2500Protocol::COUNT - 1,
            4 => crate::rf::mpm::MpmProtocol::COUNT - 1,
            _ => 1,
        };
        if m.rx_serial_proto > max_serial_proto {
            m.rx_serial_proto = 0;
        }
        m.set_mpm_range_check(false);
        if m.name[0] == 0 || m.name[0] == 0xFF {
            let num = self.radio.active_model + 1;
            let digit1 = b'0' + (num / 10);
            let digit2 = b'0' + (num % 10);
            m.name = [
                b'M', b'O', b'D', b'E', b'L', b' ', digit1, digit2, b' ', b' ',
            ];
        }
        if m.vario_switch > 44 {
            m.vario_switch = 0;
        }
        if m.vario_climb_deadband < 1 || m.vario_climb_deadband > 10 {
            m.vario_climb_deadband = 2;
        }
        if m.vario_sink_deadband < 1 || m.vario_sink_deadband > 20 {
            m.vario_sink_deadband = 5;
        }
        if m.vario_sink_mode > 2 {
            m.vario_sink_mode = 1;
        }
        if m.vario_motor_mute > 1 {
            m.vario_motor_mute = 1;
        }
        m.soaring.sanitize();
        for ls in m.logical_switches.iter_mut() {
            if ls.func > 10 {
                *ls = LogicalSwitch::disabled();
                continue;
            }
            if ls.is_boolean_func() {
                if ls.v1 > 44 {
                    ls.v1 = 0;
                }
                if ls.v2 < 0 || ls.v2 > 44 {
                    ls.v2 = 0;
                }
            } else {
                if ls.v1 > 57 {
                    ls.v1 = 0;
                }
                if ls.is_dual_source_func() {
                    if ls.v2 < 0 || ls.v2 > 57 {
                        ls.v2 = 0;
                    }
                } else {
                    ls.v2 = ls.v2.clamp(-100, 100);
                }
            }
            if ls.and_sw > 44 {
                ls.and_sw = 0;
            }
            if ls.delay_tenths > 100 {
                ls.delay_tenths = 100;
            }
            if ls.duration_tenths > 100 {
                ls.duration_tenths = 100;
            }
        }
    }

    /// Extract vario configuration for the active model.
    pub fn vario_config(&self) -> crate::vario::VarioConfig {
        crate::vario::VarioConfig {
            switch: self.active_model.vario_switch,
            climb_deadband_dm_s: self.active_model.vario_climb_deadband,
            sink_deadband_dm_s: self.active_model.vario_sink_deadband,
            sink_mode: crate::vario::VarioSinkMode::from_u8(self.active_model.vario_sink_mode),
            motor_mute: self.active_model.vario_motor_mute != 0,
        }
    }
}

const _: () = assert!(core::mem::size_of::<RadioConfig>() == 160);
const _: () = assert!(core::mem::size_of::<MixLine>() == 8);
const _: () = assert!(core::mem::size_of::<LogicalSwitch>() == 6);
const _: () = assert!(core::mem::size_of::<ModelConfig>() == 760);
const _: () = assert!(core::mem::size_of::<RadioStorage>() == 920);

static mut GLOBAL_STORAGE: RadioStorage = RadioStorage::empty();

/// Get global mutable reference to persistent RadioStorage.
#[allow(static_mut_refs)]
pub fn get_storage() -> &'static mut RadioStorage {
    unsafe {
        if GLOBAL_STORAGE.radio.magic == 0 {
            GLOBAL_STORAGE.init_default();
        }
        &mut GLOBAL_STORAGE
    }
}

/// CCITT CRC-16 (polynomial 0x1021, init 0xFFFF).
pub fn crc16_ccitt_false(data: &[u8]) -> u16 {
    let mut crc = 0xFFFFu16;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if (crc & 0x8000) != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

/// Standard CRC-16 / ARC (polynomial 0x8005, init 0x0000, unreversed).
pub fn crc16_arc(data: &[u8]) -> u16 {
    let mut crc = 0u16;
    for &b in data {
        crc ^= (b as u16) << 8;
        for _ in 0..8 {
            if (crc & 0x8000) != 0 {
                crc = (crc << 1) ^ 0x8005;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

/// Compute board-specific CRC-16 for EEPROM slots.
pub fn crc16(data: &[u8]) -> u16 {
    crate::board::ActiveBoard::eeprom_crc16(data)
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum StorageError {
    I2cError,
    CrcMismatch,
    VersionMismatch(u8),
    LengthMismatch(u16),
    EmptySlot,
    SlotCorrupt,
}

/// 8-byte framing envelope for EEPROM slots.
/// Provides integrity validation, versioning, and length check.
/// [0..2] magic: u16 LE (0x4658 = "FX")
/// [2]    version: u8 (1 = SCHEMA_VERSION)
/// [3]    flags: u8 (0 = Valid active slot, 0xFF = Deleted)
/// [4..6] len: u16 LE (exact payload byte length)
/// [6..8] crc16: u16 LE (CRC-16 over payload)
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SlotHeader {
    pub magic: u16,
    pub version: u8,
    pub flags: u8,
    pub len: u16,
    pub crc16: u16,
}

impl SlotHeader {
    pub fn parse(buf: &[u8; HEADER_SIZE]) -> Self {
        Self {
            magic: u16::from_le_bytes([buf[0], buf[1]]),
            version: buf[2],
            flags: buf[3],
            len: u16::from_le_bytes([buf[4], buf[5]]),
            crc16: u16::from_le_bytes([buf[6], buf[7]]),
        }
    }

    pub fn to_bytes(&self) -> [u8; HEADER_SIZE] {
        let mut out = [0u8; HEADER_SIZE];
        out[0..2].copy_from_slice(&self.magic.to_le_bytes());
        out[2] = self.version;
        out[3] = self.flags;
        out[4..6].copy_from_slice(&self.len.to_le_bytes());
        out[6..8].copy_from_slice(&self.crc16.to_le_bytes());
        out
    }

    pub fn is_valid(&self, expected_len: usize) -> Result<(), StorageError> {
        if self.magic != SLOT_MAGIC {
            return Err(StorageError::EmptySlot);
        }
        if self.version != SCHEMA_VERSION {
            return Err(StorageError::VersionMismatch(self.version));
        }
        if self.flags != 0 {
            return Err(StorageError::EmptySlot);
        }
        if self.len as usize != expected_len {
            return Err(StorageError::LengthMismatch(self.len));
        }
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct EepromStatus {
    pub bus: u8,  // 0: None, 1: I2C1 (PB8/PB9), 2: I2C2 (PB10/PB11)
    pub addr: u8, // 7-bit slave address, e.g. 0x50
    pub detected: bool,
    pub read_ok: bool,
    pub write_ok: bool,
    pub write_err: bool,
}

pub static mut EEPROM_STATUS: EepromStatus = EepromStatus {
    bus: 0,
    addr: 0x50,
    detected: false,
    read_ok: false,
    write_ok: false,
    write_err: false,
};

pub fn get_eeprom_status() -> EepromStatus {
    unsafe { EEPROM_STATUS }
}

pub struct EepromI2c;

impl EepromI2c {
    pub fn init() {
        unsafe {
            if let Some(ref mut bus) = STORAGE_BUS {
                let ok = bus.init();
                EEPROM_STATUS.detected = ok;
            }
        }
    }

    /// Read sequential bytes starting at 16-bit EEPROM address.
    pub fn read(addr: u16, data: &mut [u8]) -> Result<(), StorageError> {
        if data.is_empty() {
            return Ok(());
        }
        unsafe {
            if let Some(ref mut bus) = STORAGE_BUS {
                match bus.read(addr, data) {
                    Ok(()) => {
                        EEPROM_STATUS.read_ok = true;
                        Ok(())
                    }
                    Err(()) => Err(StorageError::I2cError),
                }
            } else {
                Err(StorageError::I2cError)
            }
        }
    }

    /// Write arbitrary slice to non-volatile storage via the registered StorageBus driver.
    pub fn write(addr: u16, data: &[u8]) -> Result<(), StorageError> {
        if data.is_empty() {
            return Ok(());
        }
        unsafe {
            if let Some(ref mut bus) = STORAGE_BUS {
                match bus.write(addr, data) {
                    Ok(()) => {
                        EEPROM_STATUS.write_ok = true;
                        Ok(())
                    }
                    Err(()) => {
                        EEPROM_STATUS.write_err = true;
                        Err(StorageError::I2cError)
                    }
                }
            } else {
                Err(StorageError::I2cError)
            }
        }
    }
}

/// Helper to read a raw C struct from an EEPROM slot with 8-byte framing and CRC16 verification.
/// Under `SCHEMA_VERSION = 2` (v1.0.0), any v1 slot fails `hdr.is_valid` and resets cleanly to v2 defaults.
pub fn load_slot<T: Copy>(addr: u16, dst: &mut T) -> Result<(), StorageError> {
    #[cfg(test)]
    {
        let _ = addr;
        let _ = dst;
        Err(StorageError::EmptySlot)
    }

    #[cfg(not(test))]
    {
        let mut hdr_bytes = [0u8; HEADER_SIZE];
        EepromI2c::read(addr, &mut hdr_bytes)?;
        let hdr = SlotHeader::parse(&hdr_bytes);

        let expected_size = core::mem::size_of::<T>();
        hdr.is_valid(expected_size)?;

        let dst_bytes =
            unsafe { core::slice::from_raw_parts_mut(dst as *mut T as *mut u8, expected_size) };
        EepromI2c::read(addr + HEADER_SIZE as u16, dst_bytes)?;

        if crc16(dst_bytes) != hdr.crc16 {
            return Err(StorageError::CrcMismatch);
        }

        Ok(())
    }
}

/// Helper to write a raw C struct into an EEPROM slot with 8-byte framing and CRC16.
pub fn save_slot<T: Copy>(addr: u16, src: &T) -> Result<(), StorageError> {
    #[cfg(test)]
    {
        let _ = addr;
        let _ = src;
        Ok(())
    }

    #[cfg(not(test))]
    {
        let size = core::mem::size_of::<T>();
        let src_bytes = unsafe { core::slice::from_raw_parts(src as *const T as *const u8, size) };

        let crc = crc16(src_bytes);
        let hdr = SlotHeader {
            magic: SLOT_MAGIC,
            version: SCHEMA_VERSION,
            flags: 0,
            len: size as u16,
            crc16: crc,
        };

        // Write header first
        let hdr_bytes = hdr.to_bytes();
        let res = (|| -> Result<(), StorageError> {
            EepromI2c::write(addr, &hdr_bytes)?;
            EepromI2c::write(addr + HEADER_SIZE as u16, src_bytes)?;
            Ok(())
        })();

        unsafe {
            if res.is_ok() {
                EEPROM_STATUS.write_ok = true;
                EEPROM_STATUS.write_err = false;
            } else {
                EEPROM_STATUS.write_err = true;
            }
        }

        res
    }
}

/// Read only model name and magic without transferring the entire ModelConfig.
pub fn peek_model_name(slot_idx: usize) -> Option<[u8; 10]> {
    #[cfg(test)]
    {
        let _ = slot_idx;
        None
    }
    #[cfg(not(test))]
    {
        let addr = model_slot_addr(slot_idx)?;
        // Read 8-byte header + first 10 bytes (ModelConfig::name) = 18 bytes
        let mut buf = [0u8; HEADER_SIZE + 10];
        if EepromI2c::read(addr, &mut buf).is_err() {
            return None;
        }

        let hdr = SlotHeader::parse(unsafe {
            &*(buf[0..HEADER_SIZE].as_ptr() as *const [u8; HEADER_SIZE])
        });
        if hdr.magic != SLOT_MAGIC || hdr.flags != 0 {
            return None;
        }
        if hdr.len != (core::mem::size_of::<ModelConfig>() as u16)
            && hdr.len != 128
            && hdr.len != 348
            && hdr.len != 364
            && hdr.len != 368
            && hdr.len != 392
        {
            return None;
        }

        let mut name = [0u8; 10];
        name.copy_from_slice(&buf[HEADER_SIZE..HEADER_SIZE + 10]);
        Some(name)
    }
}

/// Delete a model slot in O(1) by invalidating the magic bytes in the 8-byte header.
pub fn delete_model_slot(slot_idx: usize) -> bool {
    #[cfg(test)]
    {
        let _ = slot_idx;
        true
    }
    #[cfg(not(test))]
    {
        let Some(addr) = model_slot_addr(slot_idx) else {
            return false;
        };
        // Overwrite magic with 0xFFFF to invalidate slot
        let invalid_magic = [0xFFu8, 0xFFu8];
        EepromI2c::write(addr, &invalid_magic).is_ok()
    }
}

/// Initialize EEPROM peripheral and load radio storage into memory.
#[inline(never)]
pub fn load_storage_into(storage: &mut RadioStorage) {
    #[cfg(test)]
    {
        storage.init_default();
    }

    #[cfg(not(test))]
    {
        EepromI2c::init();

        // 1. Try loading RadioConfig from Primary Slot A
        let mut radio_loaded = false;
        if load_slot(RADIO_SLOT_A_ADDR, &mut storage.radio).is_ok() {
            radio_loaded = true;
            unsafe {
                EEPROM_STATUS.read_ok = true;
            }
        } else if load_slot(RADIO_SLOT_B_ADDR, &mut storage.radio).is_ok() {
            // Primary Slot A failed, but Backup Slot B succeeded!
            radio_loaded = true;
            unsafe {
                EEPROM_STATUS.read_ok = true;
            }
            // Restore Slot A from Slot B
            let _ = save_slot(RADIO_SLOT_A_ADDR, &storage.radio);
        }

        if !radio_loaded {
            // Fresh / uninitialized EEPROM -> load factory defaults and save to Slot A and B
            storage.init_default();
            let _ = save_slot(RADIO_SLOT_A_ADDR, &storage.radio);
            let _ = save_slot(RADIO_SLOT_B_ADDR, &storage.radio);
            if let Some(addr) = model_slot_addr(0) {
                let _ = save_slot(addr, &storage.active_model);
            }
            storage.sanitize();
            return;
        }

        // 2. Load only the active model into the active-model SRAM cache
        let active_idx = (storage.radio.active_model as usize).min(NUM_MODELS - 1);
        if let Some(addr) = model_slot_addr(active_idx) {
            if load_slot(addr, &mut storage.active_model).is_err() {
                storage.active_model = ModelConfig::default_for_index(active_idx);
            }
        } else {
            storage.active_model = ModelConfig::default_for_index(0);
        }

        storage.sanitize();
    }
}

/// Load a specific model slot into .
pub fn load_active_model(storage: &mut RadioStorage) -> bool {
    let active_idx = (storage.radio.active_model as usize).min(NUM_MODELS - 1);
    if let Some(addr) = model_slot_addr(active_idx) {
        if load_slot(addr, &mut storage.active_model).is_ok() {
            storage.sanitize();
            return true;
        }
    }
    storage.active_model = ModelConfig::default_for_index(active_idx);
    storage.sanitize();
    false
}

/// Save arbitrary ModelConfig to an arbitrary model slot index.
pub fn save_model_to_slot(slot_idx: usize, model: &ModelConfig) -> bool {
    if let Some(addr) = model_slot_addr(slot_idx) {
        save_slot(addr, model).is_ok()
    } else {
        false
    }
}

#[allow(dead_code)]
pub fn load_storage() -> RadioStorage {
    let mut storage = RadioStorage::empty();
    load_storage_into(&mut storage);
    storage
}

pub fn load_config() -> RadioConfig {
    let storage = get_storage();
    storage.radio
}

/// Save the currently active model to its dedicated EEPROM slot.
pub fn save_active_model(storage: &RadioStorage) -> bool {
    #[cfg(test)]
    {
        let _ = storage;
        true
    }
    #[cfg(not(test))]
    {
        let idx = (storage.radio.active_model as usize).min(NUM_MODELS - 1);
        save_model_to_slot(idx, &storage.active_model)
    }
}

/// Save RadioConfig using A/B ping-pong save to ensure power-loss safety.
pub fn save_radio_config(storage: &RadioStorage) -> bool {
    #[cfg(test)]
    {
        let _ = storage;
        true
    }
    #[cfg(not(test))]
    {
        // Ping-pong A/B commit:
        // Step 1: Write Backup Slot B first with new CRC16
        let ok_b = save_slot(RADIO_SLOT_B_ADDR, &storage.radio).is_ok();
        // Step 2: Write Primary Slot A with new CRC16
        let ok_a = save_slot(RADIO_SLOT_A_ADDR, &storage.radio).is_ok();
        ok_a && ok_b
    }
}

/// Atomic storage commit: writes active model first, then RadioConfig A/B.
pub fn save_storage(storage: &RadioStorage) -> bool {
    let mut ok = save_active_model(storage);
    ok = ok && save_radio_config(storage);
    ok
}

/// Read persisted receiver ID from active model if previously bound.
pub fn load_saved_rx_id() -> Option<u32> {
    let storage = get_storage();
    let active = storage.active_model();
    if active.rx_id != 0 && active.rx_id != 0xFFFF_FFFF {
        Some(active.rx_id)
    } else {
        None
    }
}

/// Verified unallocated pre-partition SD card gap (`1..min_partition_start_lba`)
/// so virtual EEPROM / raw telemetry writes never collide with EdgeTX FAT32 filesystems.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SdRawGap {
    /// First safe pre-partition sector for the 16 KB (32 sectors) virtual EEPROM.
    pub eeprom_base_sector: u32,
    /// Exclusive upper bound (`first_lba` of the lowest partition). No raw write may touch `>= limit_sector`.
    pub limit_sector: u32,
}

impl SdRawGap {
    pub const EEPROM_SECTORS: u32 = 32; // 16 KB / 512 B
    pub const PREFERRED_EEPROM_BASE: u32 = 256;
    pub const PREFERRED_TELEM_BASE: u32 = 512;

    /// Inspect Sector 0 (MBR) to verify a safe unallocated gap before Partition 1.
    /// Rejects FAT/EXFAT Volume Boot Records ("superfloppy" cards without MBR), GPT protective disks,
    /// invalid signatures, and cards whose first partition starts below sector `288`.
    pub fn from_mbr(sector0: &[u8; 512]) -> Option<Self> {
        if sector0[510] != 0x55 || sector0[511] != 0xAA {
            return None;
        }
        // Reject raw FAT12/16/32 or EXFAT Volume Boot Record directly on Sector 0
        if &sector0[54..57] == b"FAT"
            || &sector0[82..87] == b"FAT32"
            || &sector0[3..11] == b"EXFAT   "
        {
            return None;
        }

        let mut min_first_lba = u32::MAX;
        for entry_idx in 0..4 {
            let off = 446 + entry_idx * 16;
            let ptype = sector0[off + 4];
            if ptype == 0 {
                continue;
            }
            // Reject GPT protective MBR (GPT tables span sectors 2..33+)
            if ptype == 0xEE {
                return None;
            }
            let first_lba = u32::from_le_bytes([
                sector0[off + 8],
                sector0[off + 9],
                sector0[off + 10],
                sector0[off + 11],
            ]);
            if first_lba == 0 {
                return None;
            }
            if first_lba < min_first_lba {
                min_first_lba = first_lba;
            }
        }

        if min_first_lba == u32::MAX
            || min_first_lba < Self::PREFERRED_EEPROM_BASE + Self::EEPROM_SECTORS
        {
            return None;
        }

        Some(Self {
            eeprom_base_sector: Self::PREFERRED_EEPROM_BASE,
            limit_sector: min_first_lba,
        })
    }

    #[inline]
    pub const fn is_eeprom_sector_safe(&self, sector_idx: u32) -> bool {
        sector_idx >= self.eeprom_base_sector
            && sector_idx < (self.eeprom_base_sector + Self::EEPROM_SECTORS)
            && sector_idx < self.limit_sector
    }

    #[inline]
    pub const fn has_safe_telem_ring(&self) -> bool {
        self.limit_sector >= Self::PREFERRED_TELEM_BASE + 128
    }

    #[inline]
    pub const fn is_telem_sector_safe(&self, sector_idx: u32) -> bool {
        self.has_safe_telem_ring()
            && sector_idx >= Self::PREFERRED_TELEM_BASE
            && sector_idx < self.limit_sector
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adc_input_mode_detent() {
        assert_eq!(AdcInputMode::from_u8(0), AdcInputMode::Default);
        assert_eq!(AdcInputMode::from_u8(1), AdcInputMode::Off);
        assert_eq!(AdcInputMode::from_u8(2), AdcInputMode::TwoPos);
        assert_eq!(AdcInputMode::from_u8(3), AdcInputMode::ThreePos);
        assert_eq!(AdcInputMode::from_u8(4), AdcInputMode::SixPos);
        assert_eq!(AdcInputMode::from_u8(5), AdcInputMode::Pot);
        assert_eq!(AdcInputMode::from_u8(6), AdcInputMode::PotDetent);
        assert_eq!(AdcInputMode::from_u8(7), AdcInputMode::InstantTrim);
        assert!(AdcInputMode::Pot.is_pot());
        assert!(AdcInputMode::PotDetent.is_pot());
        assert!(!AdcInputMode::InstantTrim.is_pot());
        assert!(!AdcInputMode::Off.is_pot());
        assert!(AdcInputMode::Off.is_off());
        assert!(!AdcInputMode::Pot.has_detent());
        assert!(AdcInputMode::PotDetent.has_detent());
    }

    #[test]
    fn test_pot_calib_and_radio_config_defaults() {
        let radio = RadioConfig::default_factory();
        assert_eq!(radio.aux_pots.len(), 11);
        assert_eq!(radio.adc_modes.len(), 11);
        for i in 0..11 {
            assert_eq!(radio.aux_pots[i].min, crate::adc::ADC_MIN);
            assert_eq!(radio.aux_pots[i].center, crate::adc::ADC_CENTER);
            assert_eq!(radio.aux_pots[i].max, crate::adc::ADC_MAX);
            assert_eq!(radio.adc_modes[i], 0);
        }

        let pot = PotCalib::new(100, 2000, 3900);
        assert_eq!(pot.min, 100);
        assert_eq!(pot.center, 2000);
        assert_eq!(pot.max, 3900);

        assert_eq!(radio.gimbal_type, 0);
        assert!(!radio.is_hall_gimbals());
        assert_eq!(radio.stick_invert_mask, 0x03);
        assert!(radio.is_stick_inverted(0)); // RH
        assert!(radio.is_stick_inverted(1)); // RV
        assert!(!radio.is_stick_inverted(2)); // LV
        assert!(!radio.is_stick_inverted(3)); // LH
    }

    #[test]
    fn test_model_config_size_and_rf_power() {
        assert_eq!(core::mem::size_of::<ModelConfig>(), 760);
        assert_eq!(core::mem::size_of::<RadioConfig>(), 160);
        assert_eq!(core::mem::size_of::<RadioStorage>(), 920);
        assert!(HEADER_SIZE + core::mem::size_of::<ModelConfig>() <= MODEL_SLOT_SIZE);
        assert!(HEADER_SIZE + core::mem::size_of::<RadioConfig>() <= RADIO_SLOT_SIZE);

        let default_m = ModelConfig::default_for_index(0);
        assert_eq!(default_m.rf_power, 0);
        assert_eq!(default_m.vario_switch, 0);
        assert_eq!(default_m.vario_climb_deadband, 2);
        assert_eq!(default_m.vario_sink_deadband, 5);
        assert_eq!(default_m.vario_sink_mode, 1);
        assert_eq!(default_m.vario_motor_mute, 1);

        let mut storage = RadioStorage::default_factory();
        storage.active_model.rf_power = 99;
        storage.active_model.vario_switch = 99;
        storage.active_model.vario_climb_deadband = 99;
        storage.active_model.vario_sink_deadband = 99;
        storage.active_model.vario_sink_mode = 99;
        storage.active_model.vario_motor_mute = 99;
        storage.active_model.timer1_stop_src = 99;
        storage.active_model.timer1_reset_src = 99;
        storage.active_model.timer1_flags = 0xFF;
        storage.active_model.timers_ext[0] = crate::timer::TimerConfig {
            secs: 9999,
            start_src: 99,
            stop_src: 99,
            reset_src: 99,
            flags: 0xFF,
        };
        storage.sanitize();
        assert_eq!(storage.active_model.rf_power, 0);
        assert_eq!(storage.active_model.vario_switch, 0);
        assert_eq!(storage.active_model.vario_climb_deadband, 2);
        assert_eq!(storage.active_model.vario_sink_deadband, 5);
        assert_eq!(storage.active_model.vario_sink_mode, 1);
        assert_eq!(storage.active_model.vario_motor_mute, 1);
        assert_eq!(storage.active_model.timer1_stop_src, 0);
        assert_eq!(storage.active_model.timer1_reset_src, 0);
        assert_eq!(
            storage.active_model.timer1_flags,
            crate::timer::TimerConfig::FLAG_AUDIO_BEEPS
        );
        assert_eq!(storage.active_model.timers_ext[0].secs, 0);
        assert_eq!(storage.active_model.timers_ext[0].start_src, 0);
        assert_eq!(storage.active_model.timers_ext[0].stop_src, 0);
        assert_eq!(storage.active_model.timers_ext[0].reset_src, 0);
        assert_eq!(
            storage.active_model.timers_ext[0].flags,
            crate::timer::TimerConfig::FLAG_AUDIO_BEEPS
        );
    }

    #[test]
    fn test_schema_v2_rejects_v1_slot_header() {
        let v1_hdr = SlotHeader {
            magic: SLOT_MAGIC,
            version: 1,
            flags: 0,
            len: 464,
            crc16: 0x1234,
        };
        assert_eq!(
            v1_hdr.is_valid(core::mem::size_of::<ModelConfig>()),
            Err(StorageError::VersionMismatch(1))
        );
        let v2_hdr = SlotHeader {
            magic: SLOT_MAGIC,
            version: SCHEMA_VERSION,
            flags: 0,
            len: core::mem::size_of::<ModelConfig>() as u16,
            crc16: 0x1234,
        };
        assert!(v2_hdr.is_valid(core::mem::size_of::<ModelConfig>()).is_ok());
    }

    #[test]
    fn test_telem_slots_sanitize() {
        let mut storage = RadioStorage::default_factory();
        storage.active_model.telem_slots = [0, 11, 22, 23];
        storage.sanitize();
        assert_eq!(storage.active_model.telem_slots, [0, 11, 22, 22]);
    }

    #[test]
    fn test_trainer_config_helpers_and_sanitize() {
        let mut radio = RadioConfig::default_factory();
        assert_eq!(radio.trainer_config, 0);
        assert!(!radio.is_ppm_out_enabled());
        assert!(!radio.is_ppm_in_enabled());

        radio.set_ppm_out_enabled(true);
        assert!(radio.is_ppm_out_enabled());
        assert!(!radio.is_ppm_in_enabled());
        assert_eq!(radio.trainer_config, 1);

        radio.set_ppm_in_enabled(true);
        assert!(radio.is_ppm_out_enabled());
        assert!(radio.is_ppm_in_enabled());
        assert_eq!(radio.trainer_config, 3);

        radio.set_ppm_out_enabled(false);
        assert!(!radio.is_ppm_out_enabled());
        assert!(radio.is_ppm_in_enabled());
        assert_eq!(radio.trainer_config, 2);

        let mut storage = RadioStorage::default_factory();
        storage.radio.set_dsm_sat_in_enabled(true);
        storage.radio.set_ppm_out_enabled(true);
        assert_eq!(storage.radio.trainer_config, 5);
        storage.sanitize();
        assert_eq!(storage.radio.trainer_config, 5);

        // Mutually exclusive PPM In + DSM SAT bits clear DSM SAT
        storage.radio.trainer_config = 7;
        storage.sanitize();
        assert_eq!(storage.radio.trainer_config, 3);

        storage.radio.trainer_config = 99;
        storage.sanitize();
        assert_eq!(storage.radio.trainer_config, 0);
    }

    #[test]
    fn test_sanitize_preserves_instant_trim_and_rf_protocols() {
        let mut storage = RadioStorage::default_factory();

        // AdcInputMode::InstantTrim = 7 must survive sanitize; invalid values (>7) reset to Default (0)
        storage.radio.adc_modes[0] = AdcInputMode::InstantTrim as u8;
        storage.radio.adc_modes[1] = 8;
        storage.sanitize();
        assert_eq!(storage.radio.adc_modes[0], AdcInputMode::InstantTrim as u8);
        assert_eq!(storage.radio.adc_modes[1], 0);

        // RfProtocol variants Cc2500 (2) and Dsm (3) + dsm_sub_proto (0..=3) must survive sanitize
        for proto in [0u8, 1, 2, 3] {
            storage.active_model.rf_protocol = proto;
            storage.active_model.dsm_sub_proto = proto;
            storage.sanitize();
            assert_eq!(storage.active_model.rf_protocol, proto);
            assert_eq!(storage.active_model.dsm_sub_proto, proto);
        }

        // Dual RF Module target (0 = Internal, 1 = External Bay)
        assert_eq!(storage.active_model.rf_module_target(), 0);
        storage.active_model.set_rf_module_target(1);
        assert_eq!(storage.active_model.rf_module_target(), 1);
        storage.active_model.set_mpm_rx_num(12);
        storage.active_model.set_mpm_range_check(true);
        assert_eq!(storage.active_model.rf_module_target(), 1);
        assert_eq!(storage.active_model.mpm_rx_num(), 12);
        assert!(storage.active_model.mpm_range_check());
        storage.active_model.set_rf_module_target(0);
        assert_eq!(storage.active_model.rf_module_target(), 0);
        assert_eq!(storage.active_model.mpm_rx_num(), 12);
        assert!(storage.active_model.mpm_range_check());

        // Out-of-range protocol / sub-protocol reset safely
        storage.active_model.rf_protocol = 99;
        storage.active_model.dsm_sub_proto = 99;
        storage.sanitize();
        assert_eq!(
            storage.active_model.rf_protocol,
            crate::board::ActiveBoard::DEFAULT_PROTOCOL as u8
        );
        assert_eq!(storage.active_model.dsm_sub_proto, 0);
    }

    #[test]
    fn test_crc16_and_slot_header() {
        let test_payload = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
        let crc = crc16(&test_payload);
        assert_ne!(crc, 0);

        let hdr = SlotHeader {
            magic: SLOT_MAGIC,
            version: SCHEMA_VERSION,
            flags: 0,
            len: test_payload.len() as u16,
            crc16: crc,
        };
        let bytes = hdr.to_bytes();
        let parsed = SlotHeader::parse(&bytes);
        assert_eq!(parsed, hdr);
        assert!(parsed.is_valid(test_payload.len()).is_ok());

        // Corrupted length check
        assert_eq!(
            parsed.is_valid(test_payload.len() + 1),
            Err(StorageError::LengthMismatch(test_payload.len() as u16))
        );

        // Invalid magic check
        let mut corrupted_bytes = bytes;
        corrupted_bytes[0] = 0xFF;
        let corrupted_hdr = SlotHeader::parse(&corrupted_bytes);
        assert_eq!(
            corrupted_hdr.is_valid(test_payload.len()),
            Err(StorageError::EmptySlot)
        );
    }

    #[test]
    fn test_eeprom_slot_addressing_and_bounds() {
        assert_eq!(RADIO_SLOT_A_ADDR, 0x0000);
        assert_eq!(RADIO_SLOT_B_ADDR, 0x0100);
        assert_eq!(MODEL_BASE_ADDR, 0x0200);

        // Verify model slot addresses are strictly 64-byte page-aligned
        for i in 0..NUM_MODELS {
            let addr = model_slot_addr(i).expect("Valid model slot");
            assert_eq!(addr % (PAGE_SIZE as u16), 0);
            assert!(addr >= MODEL_BASE_ADDR);
            assert!((addr as usize + MODEL_SLOT_SIZE) <= EEPROM_TOTAL_SIZE);
        }
        assert_eq!(model_slot_addr(NUM_MODELS), None);
    }
    #[test]
    fn test_dynamic_mixer_source_and_switch_availability() {
        let mut storage = RadioStorage::default_factory();

        // Stock config: ext_switches=0, ext_adc=0, j4_sg_sh_en=0
        // Sticks (1..4), MAX (11), CH1..CH4 (12..15), Thr+ (30) always available
        for s in 0..=4 {
            assert!(storage.is_source_available(s));
        }
        assert!(storage.is_source_available(11));
        assert!(storage.is_source_available(30));

        // Pots VRA & VRB are available by default (Pocket has no VRB / S2)
        assert!(storage.is_source_available(5));
        #[cfg(not(feature = "board-pocket"))]
        assert!(storage.is_source_available(6));
        #[cfg(feature = "board-pocket")]
        assert!(!storage.is_source_available(6));

        // Switches SA..SD are available by default
        assert!(storage.is_source_available(7)); // SA
        assert!(storage.is_source_available(8)); // SB
        assert!(storage.is_source_available(9)); // SC
        assert!(storage.is_source_available(10)); // SD

        #[cfg(any(feature = "board-i6x", feature = "board-i6s"))]
        {
            // Extended switches SE..SH and SWI..SWM / VRC..VRG unavailable by default
            assert!(!storage.is_source_available(31)); // SE
            assert!(!storage.is_source_available(32)); // SF
            assert!(!storage.is_source_available(33)); // SG
            assert!(!storage.is_source_available(34)); // SH
            assert!(!storage.is_source_available(35)); // SWI
            assert!(!storage.is_source_available(36)); // SWJ
            assert!(!storage.is_source_available(37)); // VRC
            assert!(!storage.is_source_available(38)); // VRD
            assert!(!storage.is_source_available(39)); // VRE
            assert!(!storage.is_source_available(40)); // VRF
            assert!(!storage.is_source_available(41)); // VRG
            assert!(!storage.is_source_available(42)); // SWK
            assert!(!storage.is_source_available(43)); // SWL
            assert!(!storage.is_source_available(44)); // SWM

            // Enable ext_adc
            storage.radio.ext_adc = 1;
            assert!(storage.is_source_available(37)); // VRC
            assert!(storage.is_source_available(38)); // VRD
            assert!(storage.is_source_available(39)); // VRE
            assert!(storage.is_source_available(40)); // VRF
            assert!(storage.is_source_available(41)); // VRG
            storage.radio.set_adc_mode_val(10, 2); // 2-pos switch on PC5 (2 = TwoPos)
            assert!(!storage.is_source_available(41)); // VRG is now switch, not pot
            assert!(storage.is_source_available(44)); // SWM
            assert!(storage.is_switch_cond_available(27)); // SWM^
            assert!(storage.is_switch_cond_available(28)); // SWMv

            // Turn PC5 to OFF (1 = Off)
            storage.radio.set_adc_mode_val(10, 1);
            assert!(!storage.is_source_available(41)); // VRG not available when Off
            assert!(!storage.is_source_available(44)); // SWM not available when Off
            assert!(!storage.is_switch_cond_available(27)); // SWM^ not available when Off
            assert!(!storage.is_switch_cond_available(28)); // SWMv not available when Off
            assert!(!storage.is_aux_source_available(41)); // VRG not available in AUX CHANNELS
            assert!(!storage.is_aux_source_available(44)); // SWM not available in AUX CHANNELS

            // Enable ext_switches
            storage.radio.ext_switches = 1;
            assert!(storage.is_source_available(31)); // SE
            assert!(storage.is_source_available(32)); // SF
            assert!(storage.is_switch_cond_available(11)); // SE^
            assert!(storage.is_switch_cond_available(14)); // SFv

            // Enable SG/SH
            storage.radio.j4_sg_sh_en = 1;
            assert!(storage.is_source_available(33)); // SG
            assert!(storage.is_source_available(34)); // SH
            assert!(storage.is_switch_cond_available(15)); // SG^
            assert!(storage.is_switch_cond_available(18)); // SHv
        }

        // Channels: default model has aux_channels [7, 8, 5, 6, 9, 10, 0, 0, 0, 0, 0, 0, 0, 0]
        // CH5..CH10 are configured (sources 16..21)
        for ch in 1..=10 {
            assert!(storage.is_channel_configured(ch));
            assert!(storage.is_source_available(ch + 11));
            assert!(storage.is_weight_src_available(ch)); // Bipolar
            assert!(storage.is_weight_src_available(ch + 18)); // Unipolar
        }
        // CH11..CH18 are unmapped (source=0) and no mix line targets them
        for ch in 11..=18 {
            assert!(!storage.is_channel_configured(ch));
            assert!(!storage.is_source_available(ch + 11));
            assert!(!storage.is_weight_src_available(ch));
            assert!(!storage.is_weight_src_available(ch + 18));
        }

        // Target CH11 with a mix line -> CH11 becomes configured
        storage.active_model.mixes[0].target_ch = 11;
        assert!(storage.is_channel_configured(11));
        assert!(storage.is_source_available(22)); // CH11 source
        assert!(storage.is_weight_src_available(11));

        // Global Variables GV1..GV5 (sources 45..49 and weight sources 37..41) are always available
        for gv in 45..=49 {
            assert!(storage.is_source_available(gv));
        }
        for ws in 37..=41 {
            assert!(storage.is_weight_src_available(ws));
        }
    }

    #[test]
    fn test_sd_raw_gap_mbr_validation() {
        // 1. Standard 1 MiB aligned FAT32 SD card (Partition 1 at LBA 2048)
        let mut mbr = [0u8; 512];
        mbr[510] = 0x55;
        mbr[511] = 0xAA;
        mbr[446 + 4] = 0x0C; // FAT32 LBA
        mbr[446 + 8..446 + 12].copy_from_slice(&2048u32.to_le_bytes());

        let gap = SdRawGap::from_mbr(&mbr).expect("2048 LBA MBR must provide safe gap");
        assert_eq!(gap.eeprom_base_sector, 256);
        assert_eq!(gap.limit_sector, 2048);
        assert!(gap.is_eeprom_sector_safe(256));
        assert!(gap.is_eeprom_sector_safe(287));
        assert!(!gap.is_eeprom_sector_safe(288));
        assert!(!gap.is_eeprom_sector_safe(2048));
        assert!(gap.has_safe_telem_ring());
        assert!(gap.is_telem_sector_safe(512));
        assert!(!gap.is_telem_sector_safe(2048));

        // 2. Legacy CHS aligned MBR (Partition 1 at LBA 63 -> NO safe gap!)
        mbr[446 + 8..446 + 12].copy_from_slice(&63u32.to_le_bytes());
        assert!(SdRawGap::from_mbr(&mbr).is_none());

        // 3. Superfloppy FAT32 directly on Sector 0 -> NO safe gap!
        let mut vbr = [0u8; 512];
        vbr[510] = 0x55;
        vbr[511] = 0xAA;
        vbr[82..87].copy_from_slice(b"FAT32");
        vbr[446 + 4] = 0x0C;
        vbr[446 + 8..446 + 12].copy_from_slice(&2048u32.to_le_bytes());
        assert!(SdRawGap::from_mbr(&vbr).is_none());

        // 4. GPT Protective MBR -> NO safe gap!
        mbr[446 + 4] = 0xEE;
        mbr[446 + 8..446 + 12].copy_from_slice(&1u32.to_le_bytes());
        assert!(SdRawGap::from_mbr(&mbr).is_none());
    }

    #[test]
    fn test_next_prev_matching_edge_cases() {
        let storage = RadioStorage::default_factory();
        assert_eq!(storage.next_matching(0, 0, |_| true), 0);
        assert_eq!(storage.prev_matching(0, 0, |_| true), 0);
        assert_eq!(storage.next_matching(10, 4, |i| i == 2), 2);
        assert_eq!(storage.prev_matching(10, 4, |i| i == 2), 2);
        assert_eq!(storage.next_matching(3, 5, |_| false), 3);
        assert_eq!(storage.prev_matching(2, 5, |_| false), 2);
    }

    #[test]
    fn test_sanitize_extended_bounds() {
        let mut storage = RadioStorage::default_factory();
        storage.active_model.channels[0].source = 71;
        storage.active_model.channels[1].source = 72;
        storage.active_model.channels[4].source = 58;
        storage.active_model.mixes[0].switch = 44;
        storage.active_model.mixes[1].switch = 45;
        storage.active_model.multiswitch_addresses[0] = 16;
        storage.active_model.multiswitch_addresses[1] = 999;

        storage.sanitize();

        assert_eq!(storage.active_model.channels[0].source, 71);
        assert_eq!(storage.active_model.channels[1].source, 0);
        assert_eq!(storage.active_model.channels[4].source, 58);
        assert_eq!(storage.active_model.mixes[0].switch, 44);
        assert_eq!(storage.active_model.mixes[1].switch, 0);
        assert_eq!(storage.active_model.multiswitch_addresses[0], 16);
        assert_eq!(storage.active_model.multiswitch_addresses[1], -1);
    }
}
