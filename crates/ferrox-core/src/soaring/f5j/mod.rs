//! FAI F5J Electric Thermal Duration Soaring State Machine.
//!
//! Synchronized with WimH's SoarETX / `f5j14.lua` protocol:
//! - Motor detection on configurable channel (`CH1..CH18`) with hysteresis thresholds
//! - Maximum 30-second motor run timer
//! - 10-second post-motor FAI launch height measurement window
//! - Working time countdown (`target_time_mins`, default 10:00)
//! - Automated touchdown detection (`<= land_alt_max_m`, `|delta| <= 0.2m` for `confirm_secs`, `>= min_climb_m` gate)

pub mod graph;
pub mod score;

use crate::buzzer::Buzzer;
use crate::mixer::{CHANNEL_CENTER_US, NUM_CHANNELS};
use crate::rf::afhds2a::TelemetryData;

use super::SoaringConfig;
use graph::AltitudeGraph;

/// Active phase of the FAI F5J competition flight.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum F5jPhase {
    /// On the ground / ready for launch.
    Idle = 0,
    /// Motor running (up to 30 seconds).
    Motor = 2,
    /// 10-second post-motor zoom measurement window.
    Window = 3,
    /// Free glide / thermal flight.
    Glide = 4,
    /// Touchdown automatically detected and flight duration frozen.
    Landed = 5,
}

impl F5jPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "RDY",
            Self::Motor => "MOT",
            Self::Window => "WIN",
            Self::Glide => "GLD",
            Self::Landed => "LND",
        }
    }
}

/// Complete real-time state for F5J soaring competition tracking.
#[derive(Clone, Debug)]
pub struct F5jEngine {
    pub phase: F5jPhase,
    pub graph: AltitudeGraph,

    /// Ground elevation zero-offset in decimeters (`0.1m`), set on reset/arm.
    pub alt_zero_dm: i16,
    /// Current altitude relative to launch ground (`>= 0`, decimeters).
    pub rel_alt_dm: i16,

    /// True if motor run hysteresis is currently latched ON.
    pub motor_started: bool,
    /// True if motor was restarted after an initial launch (competition restart = 0 score).
    pub is_restart: bool,

    pub launch_time_ms: u32,
    pub motor_start_ms: u32,
    pub window_start_ms: u32,

    /// Peak altitude in decimeters (`0.1m`) measured during Motor + 10s Window.
    pub f5j_peak_alt_dm: i16,
    /// Frozen official FAI F5J Start Height in whole meters once 10s Window closes.
    pub f5j_start_height_m: u16,

    /// Frozen flight duration in whole seconds once landing is confirmed.
    pub final_flight_sec: u32,
    /// Pilot-selected landing bonus points (`0..=50` in 5-pt steps).
    pub landing_bonus_pts: u8,

    /// True once glider climbs past `min_climb_m`.
    pub has_climbed: bool,
    /// True once landing stability check confirms touchdown.
    pub landed: bool,

    // Landing stability detector state
    last_land_check_ms: u32,
    last_land_alt_dm: i16,
    ground_counter: u8,
    potential_land_ms: u32,

    // Audio cadence deduplication
    last_motor_sec: u32,
    last_work_sec: u32,
    last_window_sec: u32,
}

impl Default for F5jEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl F5jEngine {
    pub const fn new() -> Self {
        Self {
            phase: F5jPhase::Idle,
            graph: AltitudeGraph::new(),
            alt_zero_dm: 0,
            rel_alt_dm: 0,
            motor_started: false,
            is_restart: false,
            launch_time_ms: 0,
            motor_start_ms: 0,
            window_start_ms: 0,
            f5j_peak_alt_dm: 0,
            f5j_start_height_m: 0,
            final_flight_sec: 0,
            landing_bonus_pts: 50,
            has_climbed: false,
            landed: false,
            last_land_check_ms: 0,
            last_land_alt_dm: -9990,
            ground_counter: 0,
            potential_land_ms: 0,
            last_motor_sec: 0,
            last_work_sec: u32::MAX,
            last_window_sec: 0,
        }
    }

    /// Re-zero barometric launch reference to current telemetry altitude and return to `Idle`.
    pub fn reset_to_ready(&mut self, current_raw_alt_dm: i16) {
        let prev_bonus = self.landing_bonus_pts;
        *self = Self::new();
        self.alt_zero_dm = current_raw_alt_dm;
        self.landing_bonus_pts = prev_bonus;
    }

    /// Normalize channel microsecond pulse (`988..2012 µs`) into percentage (`-100..=100%`).
    #[inline]
    pub fn pulse_to_pct(pulse_us: u16) -> i8 {
        let delta = (pulse_us as i32) - (CHANNEL_CENTER_US as i32);
        // 500 µs throw = 100%
        ((delta * 100) / 500).clamp(-100, 100) as i8
    }

    /// Elapsed active flight time in seconds (frozen upon touchdown).
    #[inline]
    pub fn flight_elapsed_sec(&self, now_ms: u32) -> u32 {
        match self.phase {
            F5jPhase::Idle => 0,
            F5jPhase::Landed => self.final_flight_sec,
            _ => now_ms.wrapping_sub(self.launch_time_ms) / 1000,
        }
    }

    /// Current effective display F5J start height in whole meters (live peak until Window closes).
    #[inline]
    pub fn display_f5j_m(&self) -> u16 {
        if self.phase == F5jPhase::Glide || self.phase == F5jPhase::Landed {
            self.f5j_start_height_m
        } else {
            (self.f5j_peak_alt_dm.max(0) / 10) as u16
        }
    }

    /// Cycle landing bonus points (`0, 5, 10, ..., 50, 0`).
    #[inline]
    pub fn cycle_landing_bonus(&mut self) {
        self.landing_bonus_pts = if self.landing_bonus_pts >= 50 {
            0
        } else {
            (self.landing_bonus_pts + 5).min(50)
        };
    }

    /// Update the F5J/F3K state machine, altitude graph, landing detector, and audio alerts.
    pub fn update(
        &mut self,
        now_ms: u32,
        rf_chs: &[u16; NUM_CHANNELS],
        telem: &TelemetryData,
        cfg: &SoaringConfig,
        buzzer: &mut Buzzer,
    ) {
        if !cfg.is_active() {
            return;
        }

        // Convert raw telemetry altitude into relative altitude above ground reference (`>= 0 dm`)
        let raw_dm = if telem.alt_dm != 0 {
            telem.alt_dm
        } else {
            telem.alt_m.saturating_mul(10)
        };

        if self.phase == F5jPhase::Idle && !cfg.is_f3k() {
            // While idle on ground before motor launch, track zero reference gently unless already launched
            self.alt_zero_dm = raw_dm;
            self.rel_alt_dm = 0;
        } else {
            self.rel_alt_dm = raw_dm.saturating_sub(self.alt_zero_dm).max(0);
        }

        let ch_idx = (cfg.motor_ch as usize).min(NUM_CHANNELS - 1);
        let motor_pct = Self::pulse_to_pct(rf_chs[ch_idx]);
        let audio_en = cfg.audio_cues_enabled();

        // 1. Detect motor launch / restart (F5J) or hand-launch throw (F3K)
        let f3k_throw = cfg.is_f3k()
            && self.phase == F5jPhase::Idle
            && (motor_pct > cfg.motor_on_thr_pct || self.rel_alt_dm >= 20);
        if (!self.motor_started && motor_pct > cfg.motor_on_thr_pct) || f3k_throw {
            let was_in_flight = self.phase != F5jPhase::Idle;
            self.motor_started = true;
            self.is_restart = was_in_flight && cfg.is_f5j();
            self.phase = if cfg.is_f3k() {
                F5jPhase::Window
            } else {
                F5jPhase::Motor
            };

            self.launch_time_ms = now_ms;
            self.motor_start_ms = now_ms;
            self.window_start_ms = now_ms;
            self.f5j_peak_alt_dm = self.rel_alt_dm;
            self.f5j_start_height_m = 0;
            self.final_flight_sec = 0;
            self.has_climbed = false;
            self.landed = false;
            self.ground_counter = 0;
            self.last_land_check_ms = now_ms;
            self.last_land_alt_dm = self.rel_alt_dm;
            self.last_motor_sec = 0;
            self.last_work_sec = u32::MAX;
            self.last_window_sec = 0;

            self.graph.start_flight(now_ms, self.rel_alt_dm);
            if audio_en {
                buzzer.play_tone(2000, 80);
            }
        }

        // 2. Process active flight phases (Motor -> Window -> Glide)
        let is_active = self.phase != F5jPhase::Idle && !self.landed;
        if is_active {
            let min_climb_dm = (cfg.min_climb_m as i16).saturating_mul(10);
            if self.rel_alt_dm >= min_climb_dm {
                self.has_climbed = true;
            }

            // Working time countdown cues
            let target_secs = (cfg.target_time_mins.max(1) as u32) * 60;
            let elapsed_sec = now_ms.wrapping_sub(self.launch_time_ms) / 1000;
            if elapsed_sec < target_secs {
                let rem_sec = target_secs - elapsed_sec;
                if rem_sec != self.last_work_sec {
                    self.last_work_sec = rem_sec;
                    if audio_en && self.phase == F5jPhase::Glide {
                        if rem_sec > 120 && rem_sec.is_multiple_of(60) {
                            buzzer.timer_minute_beep();
                            #[cfg(feature = "voice-audio")]
                            if buzzer.enabled {
                                crate::audio::speak_number(
                                    (rem_sec / 60) as i32,
                                    Some(crate::audio::VoiceUnit::Minutes),
                                );
                            }
                        } else if (60..=120).contains(&rem_sec) && rem_sec.is_multiple_of(15) {
                            buzzer.play_tone(1600, 45);
                            #[cfg(feature = "voice-audio")]
                            if buzzer.enabled && rem_sec == 60 {
                                crate::audio::speak_number(
                                    1,
                                    Some(crate::audio::VoiceUnit::Minutes),
                                );
                            }
                        } else if (11..=55).contains(&rem_sec) && rem_sec.is_multiple_of(5) {
                            buzzer.play_tone(1600, 35);
                            #[cfg(feature = "voice-audio")]
                            if buzzer.enabled && rem_sec == 30 {
                                crate::audio::speak_number(
                                    30,
                                    Some(crate::audio::VoiceUnit::Seconds),
                                );
                            }
                        } else if rem_sec <= 10 {
                            buzzer.timer_countdown_beep();
                            #[cfg(feature = "voice-audio")]
                            if buzzer.enabled {
                                crate::audio::speak_number(rem_sec as i32, None);
                            }
                        }
                    }
                }
            } else if self.last_work_sec != 0 {
                self.last_work_sec = 0;
                if audio_en {
                    buzzer.timer_elapsed_alarm();
                }
            }

            match self.phase {
                F5jPhase::Motor => {
                    if self.rel_alt_dm > self.f5j_peak_alt_dm {
                        self.f5j_peak_alt_dm = self.rel_alt_dm;
                    }
                    let motor_elapsed_ms = now_ms.wrapping_sub(self.motor_start_ms);
                    let motor_sec = motor_elapsed_ms / 1000;
                    if motor_sec != self.last_motor_sec {
                        self.last_motor_sec = motor_sec;
                        if audio_en {
                            if motor_sec == 5
                                || motor_sec == 10
                                || motor_sec == 15
                                || motor_sec == 20
                            {
                                buzzer.play_tone(1800, 60);
                            } else if (21..=29).contains(&motor_sec) {
                                buzzer.timer_countdown_beep();
                                #[cfg(feature = "voice-audio")]
                                if buzzer.enabled {
                                    crate::audio::speak_number((30 - motor_sec) as i32, None);
                                }
                            }
                        }
                    }
                    if motor_elapsed_ms >= 30_000 || motor_pct < cfg.motor_off_thr_pct {
                        self.phase = F5jPhase::Window;
                        self.window_start_ms = now_ms;
                        self.last_window_sec = 0;
                        if audio_en {
                            buzzer.play_tone_pattern(2200, 70, 40, 2);
                        }
                    }
                }
                F5jPhase::Window => {
                    if self.rel_alt_dm > self.f5j_peak_alt_dm {
                        self.f5j_peak_alt_dm = self.rel_alt_dm;
                    }
                    let win_elapsed_ms = now_ms.wrapping_sub(self.window_start_ms);
                    let win_sec = win_elapsed_ms / 1000;
                    if win_sec != self.last_window_sec {
                        self.last_window_sec = win_sec;
                        if audio_en && (1..=9).contains(&win_sec) {
                            buzzer.play_tone(1400, 25);
                        }
                    }
                    if win_elapsed_ms >= 10_000 {
                        self.phase = F5jPhase::Glide;
                        self.f5j_start_height_m = (self.f5j_peak_alt_dm.max(0) / 10) as u16;
                        self.graph.mark_f5j_window_closed();
                        if audio_en {
                            buzzer.play_tone(2600, 140);
                            #[cfg(feature = "voice-audio")]
                            if buzzer.enabled {
                                crate::audio::announce_telemetry(
                                    crate::audio::TelemetryCallout::Altitude,
                                    self.f5j_start_height_m as i32,
                                );
                            }
                        }
                    }
                }
                F5jPhase::Glide => {
                    // Landing detection (evaluated every 1.000 second)
                    if self.has_climbed && now_ms.wrapping_sub(self.last_land_check_ms) >= 1000 {
                        self.last_land_check_ms = now_ms;
                        let land_max_dm = (cfg.land_alt_max_m as i16).saturating_mul(10);
                        let is_on_ground = self.rel_alt_dm <= land_max_dm;
                        let is_constant =
                            (self.rel_alt_dm as i32 - self.last_land_alt_dm as i32).abs() <= 2; // <= 0.2m

                        if is_on_ground && is_constant {
                            if self.ground_counter == 0 {
                                self.potential_land_ms = now_ms;
                            }
                            self.ground_counter = self.ground_counter.saturating_add(1);

                            if self.ground_counter >= cfg.confirm_secs.max(1) {
                                self.landed = true;
                                self.phase = F5jPhase::Landed;
                                let raw_dur_sec =
                                    self.potential_land_ms.wrapping_sub(self.launch_time_ms) / 1000;
                                self.final_flight_sec = raw_dur_sec.saturating_sub(1).max(1);
                                if audio_en {
                                    buzzer.chime_calib_success();
                                }
                            }
                        } else {
                            self.ground_counter = 0;
                        }
                        self.last_land_alt_dm = self.rel_alt_dm;
                    }
                }
                F5jPhase::Idle | F5jPhase::Landed => {}
            }

            // Update 15-minute altitude graph
            self.graph.update(
                now_ms,
                self.rel_alt_dm,
                self.phase == F5jPhase::Glide,
                self.landed,
            );
        }

        // Reset motor hysteresis once motor stick/switch returns below OFF threshold
        if matches!(
            self.phase,
            F5jPhase::Window | F5jPhase::Glide | F5jPhase::Landed
        ) && motor_pct < cfg.motor_off_thr_pct
        {
            self.motor_started = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soaring::SoaringConfig;

    fn make_telem(alt_dm: i16) -> TelemetryData {
        let mut t = TelemetryData::default();
        t.connected = true;
        t.alt_dm = alt_dm;
        t.alt_m = alt_dm / 10;
        t
    }

    #[test]
    fn test_f5j_motor_30s_auto_cutoff_and_10s_zoom_window() {
        let mut engine = F5jEngine::new();
        let mut buzzer = Buzzer::new();
        let mut cfg = SoaringConfig::default_f5j();
        cfg.motor_ch = 2; // CH3
        let mut chs = [988u16; NUM_CHANNELS]; // idle low (-100%)

        // Ground baseline at 100.0m ASL (1000 dm)
        engine.update(0, &chs, &make_telem(1000), &cfg, &mut buzzer);
        assert_eq!(engine.phase, F5jPhase::Idle);

        // Engage motor at t = 1000 ms (+100% -> 2012 us)
        chs[2] = 2012;
        engine.update(1000, &chs, &make_telem(1000), &cfg, &mut buzzer);
        assert_eq!(engine.phase, F5jPhase::Motor);

        // Climb to +80.0m above ground by t = 16000 ms (15s motor run)
        engine.update(16_000, &chs, &make_telem(1800), &cfg, &mut buzzer);
        assert_eq!(engine.phase, F5jPhase::Motor);
        assert_eq!(engine.f5j_peak_alt_dm, 800);

        // Auto-cutoff at 30s motor run (t = 31000 ms) while motor still held high
        engine.update(31_000, &chs, &make_telem(1900), &cfg, &mut buzzer);
        assert_eq!(engine.phase, F5jPhase::Window);
        assert_eq!(engine.f5j_peak_alt_dm, 900); // 90.0m at cutoff

        // During 10s zoom window, glider zooms to +128.0m at t = 36000 ms
        chs[2] = 988; // pilot cuts motor stick
        engine.update(36_000, &chs, &make_telem(2280), &cfg, &mut buzzer);
        assert_eq!(engine.phase, F5jPhase::Window);
        assert_eq!(engine.f5j_peak_alt_dm, 1280);

        // At t = 41000 ms (10s window closes), start height freezes at 128m
        engine.update(41_000, &chs, &make_telem(2200), &cfg, &mut buzzer);
        assert_eq!(engine.phase, F5jPhase::Glide);
        assert_eq!(engine.f5j_start_height_m, 128);
    }

    #[test]
    fn test_f5j_auto_landing_detection_ignores_ground_handling() {
        let mut engine = F5jEngine::new();
        let mut buzzer = Buzzer::new();
        let cfg = SoaringConfig::default_f5j();
        let mut chs = [988u16; NUM_CHANNELS];

        // Launch and climb only 8m (< 15m min_climb_m), cut motor -> glide
        engine.update(0, &chs, &make_telem(0), &cfg, &mut buzzer);
        chs[2] = 2000;
        engine.update(1000, &chs, &make_telem(10), &cfg, &mut buzzer);
        chs[2] = 988;
        engine.update(5000, &chs, &make_telem(80), &cfg, &mut buzzer);
        engine.update(15_000, &chs, &make_telem(20), &cfg, &mut buzzer);
        assert_eq!(engine.phase, F5jPhase::Glide);
        assert!(!engine.has_climbed);

        // Sit on ground 5 seconds: must NOT trigger landing because < 15m climb
        for s in 16..=21 {
            engine.update(s * 1000, &chs, &make_telem(10), &cfg, &mut buzzer);
        }
        assert!(!engine.landed);

        // Now simulate real climb >= 15m (200 dm = 20m)
        engine.update(25_000, &chs, &make_telem(200), &cfg, &mut buzzer);
        assert!(engine.has_climbed);

        // Touchdown at t = 60s (1.5m = 15 dm <= 5.0m)
        engine.update(60_000, &chs, &make_telem(15), &cfg, &mut buzzer);
        engine.update(61_000, &chs, &make_telem(16), &cfg, &mut buzzer); // delta = 0.1m (1 dm <= 2 dm)
        engine.update(62_000, &chs, &make_telem(15), &cfg, &mut buzzer);
        assert!(!engine.landed); // 2 ticks so far

        engine.update(63_000, &chs, &make_telem(15), &cfg, &mut buzzer); // 3rd tick confirms!
        assert!(engine.landed);
        assert_eq!(engine.phase, F5jPhase::Landed);
        // Launched at 1s, potential land at 61s, minus 1s correction = 59s
        assert_eq!(engine.final_flight_sec, 59);
    }

    #[test]
    fn test_f5j_motor_restart_invalidates_score() {
        let mut engine = F5jEngine::new();
        let mut buzzer = Buzzer::new();
        let cfg = SoaringConfig::default_f5j();
        let mut chs = [988u16; NUM_CHANNELS];

        chs[2] = 2000;
        engine.update(1000, &chs, &make_telem(0), &cfg, &mut buzzer);
        chs[2] = 988;
        engine.update(5000, &chs, &make_telem(300), &cfg, &mut buzzer);
        engine.update(15_000, &chs, &make_telem(280), &cfg, &mut buzzer);
        assert_eq!(engine.phase, F5jPhase::Glide);
        assert!(!engine.is_restart);

        // Restart motor during glide!
        chs[2] = 2000;
        engine.update(30_000, &chs, &make_telem(250), &cfg, &mut buzzer);
        assert_eq!(engine.phase, F5jPhase::Motor);
        assert!(engine.is_restart);
    }
}
