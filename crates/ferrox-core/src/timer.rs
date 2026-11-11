//! Independent Flight Timers Engine.
//!
//! Provides up to 4 fully independent flight timers with:
//! - Count up (secs == 0) or countdown with overtime (secs > 0)
//! - Independent Start/Run, Stop/Pause, and Reset switches
//! - Level-gated or latching (start/stop) execution modes
//! - Configurable audio countdown and expiration chimes
//! - Auto-reset on model arming transitions

use crate::board::{ActiveBoard, BoardProfile};
use crate::buzzer::Buzzer;
use crate::input::Switches;
use crate::mixer;

pub const NUM_TIMERS: usize = 4;

/// Configuration for a single flight timer (6 bytes).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TimerConfig {
    /// Target duration in seconds (0 = count up from 00:00; 1..=3600 = countdown).
    pub secs: u16,
    /// Start / run trigger source:
    /// - 0: Off
    /// - 1: THs (Throttle stick > 5% / > -900)
    /// - 2: THt (Throttle stick latched once > 5%)
    /// - 3: Always On (Continuous)
    /// - 4..=31: Switch conditions (SA^..SWMv)
    pub start_src: u8,
    /// Optional independent Stop / Pause switch source:
    /// - 0: None / Unassigned
    /// - 1..=28: Switch conditions (SA^..SWMv)
    pub stop_src: u8,
    /// Optional dedicated Reset switch source:
    /// - 0: None / Unassigned
    /// - 1..=28: Switch conditions (SA^..SWMv)
    pub reset_src: u8,
    /// Configuration bitflags:
    /// - Bit 0: Latch mode (0 = Level active while start_src true & stop_src false;
    ///                     1 = Latch running on start_src rising edge, latch paused on stop_src rising edge)
    /// - Bit 1: Arm reset (auto-reset timer when model transitions to Armed)
    /// - Bit 2: Audio beeps (10s countdown beeps and expiration alarm enabled, default 1)
    pub flags: u8,
}

impl TimerConfig {
    pub const FLAG_LATCH_MODE: u8 = 1 << 0;
    pub const FLAG_ARM_RESET: u8 = 1 << 1;
    pub const FLAG_AUDIO_BEEPS: u8 = 1 << 2;

    pub const fn disabled() -> Self {
        Self {
            secs: 0,
            start_src: 0,
            stop_src: 0,
            reset_src: 0,
            flags: Self::FLAG_AUDIO_BEEPS,
        }
    }

    #[inline(always)]
    pub const fn is_latch_mode(&self) -> bool {
        (self.flags & Self::FLAG_LATCH_MODE) != 0
    }

    #[inline(always)]
    pub const fn has_arm_reset(&self) -> bool {
        (self.flags & Self::FLAG_ARM_RESET) != 0
    }

    #[inline(always)]
    pub const fn has_audio_beeps(&self) -> bool {
        (self.flags & Self::FLAG_AUDIO_BEEPS) != 0
    }

    pub fn set_latch_mode(&mut self, enabled: bool) {
        if enabled {
            self.flags |= Self::FLAG_LATCH_MODE;
        } else {
            self.flags &= !Self::FLAG_LATCH_MODE;
        }
    }

    pub fn set_arm_reset(&mut self, enabled: bool) {
        if enabled {
            self.flags |= Self::FLAG_ARM_RESET;
        } else {
            self.flags &= !Self::FLAG_ARM_RESET;
        }
    }

    pub fn set_audio_beeps(&mut self, enabled: bool) {
        if enabled {
            self.flags |= Self::FLAG_AUDIO_BEEPS;
        } else {
            self.flags &= !Self::FLAG_AUDIO_BEEPS;
        }
    }
}

/// Runtime execution state for a single timer.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TimerState {
    pub remaining_secs: u16,
    pub elapsed_secs: u16,
    pub ms_acc: u16,
    pub latched_running: bool,
    pub is_running: bool,
    pub prev_start_active: bool,
    pub prev_stop_active: bool,
    pub prev_reset_active: bool,
}

impl TimerState {
    pub const fn new(initial_secs: u16) -> Self {
        Self {
            remaining_secs: initial_secs,
            elapsed_secs: 0,
            ms_acc: 0,
            latched_running: false,
            is_running: false,
            prev_start_active: false,
            prev_stop_active: false,
            prev_reset_active: false,
        }
    }

    pub fn reset(&mut self, initial_secs: u16) {
        self.remaining_secs = initial_secs;
        self.elapsed_secs = 0;
        self.ms_acc = 0;
        self.latched_running = false;
        self.is_running = false;
    }
}

/// A bank of 4 independent flight timers.
#[derive(Copy, Clone, Debug)]
pub struct TimerBank {
    pub timers: [TimerState; NUM_TIMERS],
}

impl Default for TimerBank {
    fn default() -> Self {
        Self::new()
    }
}

impl TimerBank {
    pub const fn new() -> Self {
        Self {
            timers: [
                TimerState::new(0),
                TimerState::new(0),
                TimerState::new(0),
                TimerState::new(0),
            ],
        }
    }

    /// Reset all timers to configured model target seconds.
    pub fn reset_all(&mut self, configs: &[TimerConfig; NUM_TIMERS]) {
        for (state, cfg) in self.timers.iter_mut().zip(configs.iter()) {
            state.reset(cfg.secs);
        }
    }

    /// Reset a specific timer by index (0..3).
    pub fn reset_timer(&mut self, idx: usize, target_secs: u16) {
        if idx < NUM_TIMERS {
            self.timers[idx].reset(target_secs);
        }
    }

    /// Update all timers with the elapsed delta time.
    ///
    /// Evaluates start triggers, stop/pause triggers, reset switches, and arm transitions.
    pub fn update(
        &mut self,
        dt_ms: u16,
        configs: &[TimerConfig; NUM_TIMERS],
        throttle: i16,
        switches: &Switches,
        is_armed: bool,
        just_armed: bool,
        just_disarmed: bool,
        buzzer: &mut Buzzer,
    ) {
        for (_i, (state, cfg)) in self.timers.iter_mut().zip(configs.iter()).enumerate() {
            // Handle auto-reset on arming transition if configured
            if cfg.has_arm_reset() {
                if just_armed {
                    state.reset(cfg.secs);
                } else if just_disarmed {
                    state.latched_running = false;
                }
            }

            // Evaluate reset switch edge (1..=28 switch conditions)
            if cfg.reset_src > 0 && cfg.reset_src <= ActiveBoard::MAX_MIX_SWITCH {
                let reset_active = mixer::is_switch_active(cfg.reset_src, switches);
                let rising_edge = reset_active && !state.prev_reset_active;
                state.prev_reset_active = reset_active;
                if rising_edge {
                    state.reset(cfg.secs);
                    buzzer.play_tone(2400, 100);
                    continue;
                }
            } else {
                state.prev_reset_active = false;
            }

            // If timer has no start source assigned, it remains idle
            if cfg.start_src == 0 {
                state.is_running = false;
                state.prev_start_active = false;
                state.prev_stop_active = false;
                continue;
            }

            // Evaluate raw start trigger
            let start_active = match cfg.start_src {
                0 => false,
                1 => throttle > -900,
                2 => {
                    // THt: latches on once throttle exceeds 5%
                    if throttle > -900 {
                        state.latched_running = true;
                    }
                    state.latched_running
                }
                3 => true, // Always on
                sw if (4..=31).contains(&sw) => {
                    let cond_idx = sw - 3;
                    if cond_idx <= ActiveBoard::MAX_MIX_SWITCH {
                        mixer::is_switch_active(cond_idx, switches)
                    } else {
                        false
                    }
                }
                _ => false,
            };

            // Evaluate raw stop trigger (1..=28 switch conditions)
            let stop_active = if cfg.stop_src > 0 && cfg.stop_src <= ActiveBoard::MAX_MIX_SWITCH {
                mixer::is_switch_active(cfg.stop_src, switches)
            } else {
                false
            };

            let start_edge = start_active && !state.prev_start_active;
            let stop_edge = stop_active && !state.prev_stop_active;
            state.prev_start_active = start_active;
            state.prev_stop_active = stop_active;

            // Determine running state based on mode
            if cfg.is_latch_mode() {
                // Latch mode: start edge latches ON; stop edge latches OFF
                if start_edge {
                    state.latched_running = true;
                }
                if stop_edge {
                    state.latched_running = false;
                }
                // Also respect THt unlatch if throttle stick drops in unlatched conditions
                state.is_running = state.latched_running && is_armed;
            } else {
                // Level mode: runs while start condition holds AND stop condition does NOT hold
                state.is_running = start_active && !stop_active && is_armed;
            }

            // Accumulate elapsed time if running
            if state.is_running && dt_ms > 0 {
                state.ms_acc = state.ms_acc.saturating_add(dt_ms);
                while state.ms_acc >= 1000 {
                    state.ms_acc -= 1000;
                    if cfg.secs > 0 {
                        // Countdown mode
                        if state.remaining_secs > 0 {
                            state.remaining_secs -= 1;
                            if cfg.has_audio_beeps() {
                                if state.remaining_secs == 0 {
                                    buzzer.timer_elapsed_alarm();
                                } else if state.remaining_secs <= 10 {
                                    buzzer.timer_countdown_beep();
                                } else if state.remaining_secs.is_multiple_of(60) {
                                    buzzer.timer_minute_beep();
                                }
                            }
                        } else {
                            // Overtime (counting upwards past 00:00)
                            state.elapsed_secs = state.elapsed_secs.saturating_add(1);
                            if cfg.has_audio_beeps() && state.elapsed_secs.is_multiple_of(60) {
                                buzzer.timer_minute_beep();
                            }
                        }
                    } else {
                        // Count-up mode
                        state.elapsed_secs = state.elapsed_secs.saturating_add(1);
                        if cfg.has_audio_beeps() && state.elapsed_secs.is_multiple_of(60) {
                            buzzer.timer_minute_beep();
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{SwitchPos, Switches};

    fn make_test_switches() -> Switches {
        Switches::new()
    }

    #[test]
    fn test_timer_countdown_and_overtime() {
        let mut bank = TimerBank::new();
        let mut buzzer = Buzzer::new();
        let switches = make_test_switches();
        let configs = [
            TimerConfig {
                secs: 3,
                start_src: 3, // Always on
                stop_src: 0,
                reset_src: 0,
                flags: TimerConfig::FLAG_AUDIO_BEEPS,
            },
            TimerConfig::disabled(),
            TimerConfig::disabled(),
            TimerConfig::disabled(),
        ];

        bank.reset_all(&configs);
        assert_eq!(bank.timers[0].remaining_secs, 3);
        assert_eq!(bank.timers[0].elapsed_secs, 0);

        // Advance 1 second
        bank.update(1000, &configs, 0, &switches, true, false, false, &mut buzzer);
        assert_eq!(bank.timers[0].remaining_secs, 2);
        assert_eq!(bank.timers[0].elapsed_secs, 0);

        // Advance 2 seconds -> should reach 0
        bank.update(2000, &configs, 0, &switches, true, false, false, &mut buzzer);
        assert_eq!(bank.timers[0].remaining_secs, 0);
        assert_eq!(bank.timers[0].elapsed_secs, 0);

        // Advance 1 more second -> overtime elapsed
        bank.update(1000, &configs, 0, &switches, true, false, false, &mut buzzer);
        assert_eq!(bank.timers[0].remaining_secs, 0);
        assert_eq!(bank.timers[0].elapsed_secs, 1);
    }

    #[test]
    fn test_timer_level_vs_latch_mode() {
        let mut bank = TimerBank::new();
        let mut buzzer = Buzzer::new();
        let mut switches = make_test_switches();

        // SA^ is condition 1 (start_src = 4)
        // SBv is condition 5 (stop_src = 5)
        let latch_cfg = TimerConfig {
            secs: 100,
            start_src: 4, // SA^
            stop_src: 5,  // SBv
            reset_src: 0,
            flags: TimerConfig::FLAG_LATCH_MODE,
        };
        let configs = [
            latch_cfg,
            TimerConfig::disabled(),
            TimerConfig::disabled(),
            TimerConfig::disabled(),
        ];

        // Initially SA is Down (SA^ is false)
        switches.sa = SwitchPos::Down;
        switches.sb = SwitchPos::Up;
        bank.update(100, &configs, 0, &switches, true, false, false, &mut buzzer);
        assert!(!bank.timers[0].is_running);

        // Flick SA to Up (rising edge of SA^)
        switches.sa = SwitchPos::Up;
        bank.update(100, &configs, 0, &switches, true, false, false, &mut buzzer);
        assert!(bank.timers[0].is_running);

        // Move SA back to Down: in latch mode, it should KEEP running!
        switches.sa = SwitchPos::Down;
        bank.update(100, &configs, 0, &switches, true, false, false, &mut buzzer);
        assert!(bank.timers[0].is_running);

        // Flick SB to Down (rising edge of SBv): stops timer
        switches.sb = SwitchPos::Down;
        bank.update(100, &configs, 0, &switches, true, false, false, &mut buzzer);
        assert!(!bank.timers[0].is_running);
    }

    #[test]
    fn test_timer_reset_switch() {
        let mut bank = TimerBank::new();
        let mut buzzer = Buzzer::new();
        let mut switches = make_test_switches();

        let configs = [
            TimerConfig {
                secs: 60,
                start_src: 3, // Always on
                stop_src: 0,
                reset_src: 2, // SAv is condition 2
                flags: 0,
            },
            TimerConfig::disabled(),
            TimerConfig::disabled(),
            TimerConfig::disabled(),
        ];

        bank.reset_all(&configs);
        // Run for 5 seconds
        bank.update(5000, &configs, 0, &switches, true, false, false, &mut buzzer);
        assert_eq!(bank.timers[0].remaining_secs, 55);

        // Flick SA to Down (SAv active -> reset triggered)
        switches.sa = SwitchPos::Down;
        bank.update(100, &configs, 0, &switches, true, false, false, &mut buzzer);
        assert_eq!(bank.timers[0].remaining_secs, 60);
        assert_eq!(bank.timers[0].elapsed_secs, 0);
    }
}
