//! Native Soaring & Competition Telemetry Engine (F5J Priority 1, F3K extension).
//!
//! Zero-heap, fixed-point implementation designed for deterministic real-time execution
//! across both Cortex-M0 and Cortex-M4F targets.

pub mod f5j;

/// Soaring competition discipline assigned to a model.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SoaringDiscipline {
    Off = 0,
    F5j = 1,
    F3k = 2,
}

impl SoaringDiscipline {
    #[inline]
    pub const fn from_u8(val: u8) -> Self {
        match val {
            1 => Self::F5j,
            2 => Self::F3k,
            _ => Self::Off,
        }
    }

    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::F5j => "F5J",
            Self::F3k => "F3K",
        }
    }

    #[inline]
    pub const fn next(self) -> Self {
        match self {
            Self::Off => Self::F5j,
            Self::F5j => Self::F3k,
            Self::F3k => Self::Off,
        }
    }
}

/// Persistent per-model soaring & F5J competition configuration (24 bytes).
/// Embedded directly into `ModelConfig` within the 768-byte EEPROM model slot.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SoaringConfig {
    /// 0: Off, 1: F5J, 2: F3K (DLG)
    pub discipline: u8,
    /// 0-based motor channel index (`0..17`, e.g. `2` = CH3, `6` = CH7)
    pub motor_ch: u8,
    /// Competition working time target in minutes (`1..=99`, default `10`)
    pub target_time_mins: u8,
    /// Motor start trigger threshold in % (`-100..=100`, default `-80`)
    pub motor_on_thr_pct: i8,
    /// Motor shutoff hysteresis threshold in % (`-100..=100`, default `-90`)
    pub motor_off_thr_pct: i8,
    /// Minimum climb altitude in meters before landing detection activates (default `15`)
    pub min_climb_m: u8,
    /// Ground ceiling window in meters for landing stability detection (default `5`)
    pub land_alt_max_m: u8,
    /// Seconds of ground stability required to confirm touchdown (default `3`)
    pub confirm_secs: u8,
    /// Optional motor arming switch condition (`0..=28`)
    pub arm_switch: u8,
    /// Optional crow / landing switch condition (`0..=28`)
    pub crow_switch: u8,
    /// Glider tail topology (`0`: X-tail / Std, `1`: V-tail, `2`: M-tail)
    pub tail_type: u8,
    /// M-tail aileron-to-elevator roll-assist weight (`-100..=+100`, default `20`)
    pub mtail_aile_ele: i8,
    /// M-tail rudder-to-elevator roll-assist weight (`-100..=+100`, default `20`)
    pub mtail_rud_ele: i8,
    /// Feature bitflags (`Bit 0`: Auto-Log SD, `Bit 1`: Audio countdown cues)
    pub flags: u8,
    /// Reserved for future F3K / glider parameters
    pub _reserved: [u8; 10],
}

const _: () = assert!(core::mem::size_of::<SoaringConfig>() == 24);

impl SoaringConfig {
    pub const FLAG_AUTO_LOG: u8 = 1 << 0;
    pub const FLAG_AUDIO_CUES: u8 = 1 << 1;

    /// Default factory configuration (`discipline = Off`).
    pub const fn default_factory() -> Self {
        Self {
            discipline: SoaringDiscipline::Off as u8,
            motor_ch: 2, // CH3 (Moto) default
            target_time_mins: 10,
            motor_on_thr_pct: -80,
            motor_off_thr_pct: -90,
            min_climb_m: 15,
            land_alt_max_m: 5,
            confirm_secs: 3,
            arm_switch: 0,
            crow_switch: 0,
            tail_type: 0,
            mtail_aile_ele: 20,
            mtail_rud_ele: 20,
            flags: Self::FLAG_AUTO_LOG | Self::FLAG_AUDIO_CUES,
            _reserved: [0; 10],
        }
    }

    /// Default factory configuration pre-configured for F5J competition.
    pub const fn default_f5j() -> Self {
        let mut cfg = Self::default_factory();
        cfg.discipline = SoaringDiscipline::F5j as u8;
        cfg
    }

    #[inline(always)]
    pub const fn discipline_enum(&self) -> SoaringDiscipline {
        SoaringDiscipline::from_u8(self.discipline)
    }

    #[inline(always)]
    pub const fn is_f5j(&self) -> bool {
        self.discipline == (SoaringDiscipline::F5j as u8)
    }

    #[inline(always)]
    pub const fn audio_cues_enabled(&self) -> bool {
        (self.flags & Self::FLAG_AUDIO_CUES) != 0
    }

    #[inline(always)]
    pub const fn auto_log_enabled(&self) -> bool {
        (self.flags & Self::FLAG_AUTO_LOG) != 0
    }

    /// Sanitize all parameters to safe bounds.
    pub fn sanitize(&mut self) {
        if self.discipline > 2 {
            self.discipline = 0;
        }
        if self.motor_ch >= crate::mixer::NUM_CHANNELS as u8 {
            self.motor_ch = 2;
        }
        if self.target_time_mins == 0 || self.target_time_mins > 99 {
            self.target_time_mins = 10;
        }
        if self.motor_on_thr_pct < -98 || self.motor_on_thr_pct > 90 {
            self.motor_on_thr_pct = -80;
        }
        if self.motor_off_thr_pct < -100 || self.motor_off_thr_pct >= self.motor_on_thr_pct {
            self.motor_off_thr_pct = (self.motor_on_thr_pct as i16 - 10).clamp(-100, 80) as i8;
        }
        if self.min_climb_m == 0 || self.min_climb_m > 100 {
            self.min_climb_m = 15;
        }
        if self.land_alt_max_m == 0 || self.land_alt_max_m > 30 {
            self.land_alt_max_m = 5;
        }
        if self.confirm_secs == 0 || self.confirm_secs > 15 {
            self.confirm_secs = 3;
        }
        if self.arm_switch > 28 {
            self.arm_switch = 0;
        }
        if self.crow_switch > 28 {
            self.crow_switch = 0;
        }
        if self.tail_type > 2 {
            self.tail_type = 0;
        }
        self.mtail_aile_ele = self.mtail_aile_ele.clamp(-100, 100);
        self.mtail_rud_ele = self.mtail_rud_ele.clamp(-100, 100);
    }
}
