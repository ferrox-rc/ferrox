//! Physical switch conditions, arming/feature switch helpers, timer triggers,
//! and delayed/pulsed Logical Switches (`L1..L8`).

use super::fx::{pct_to_q10, stick_1000_to_q10};
use super::sources::evaluate_source_q10;
use super::NUM_CHANNELS;
use crate::input::{SwitchPos, Switches};
use crate::storage::{LogicalSwitch, ModelConfig, NUM_LOGICAL_SWITCHES};

/// Check if a physical or logical switch satisfies a mixer activation condition (`0..=44`).
pub fn is_switch_active(condition: u8, switches: &Switches) -> bool {
    match condition {
        0 => true,
        1 => switches.sa == SwitchPos::Up,
        2 => switches.sa == SwitchPos::Down,
        3 => switches.sb == SwitchPos::Up,
        4 => switches.sb == SwitchPos::Mid,
        5 => switches.sb == SwitchPos::Down,
        6 => switches.sc == SwitchPos::Up,
        7 => switches.sc == SwitchPos::Mid,
        8 => switches.sc == SwitchPos::Down,
        9 => switches.sd == SwitchPos::Up,
        10 => switches.sd == SwitchPos::Down,
        11 => switches.se == SwitchPos::Up,
        12 => switches.se == SwitchPos::Down,
        13 => switches.sf == SwitchPos::Up,
        14 => switches.sf == SwitchPos::Down,
        15 => switches.sg == SwitchPos::Up,
        16 => switches.sg == SwitchPos::Down,
        17 => switches.sh == SwitchPos::Up,
        18 => switches.sh == SwitchPos::Down,
        19 => switches.swi == SwitchPos::Up,
        20 => switches.swi == SwitchPos::Down,
        21 => switches.swj == SwitchPos::Up,
        22 => switches.swj == SwitchPos::Down,
        23 => switches.swk == SwitchPos::Up,
        24 => switches.swk == SwitchPos::Down,
        25 => switches.swl == SwitchPos::Up,
        26 => switches.swl == SwitchPos::Down,
        27 => switches.swm == SwitchPos::Up,
        28 => switches.swm == SwitchPos::Down,
        29..=36 => (switches.logical & (1 << (condition - 29))) != 0,
        37..=44 => (switches.logical & (1 << (condition - 37))) == 0,
        _ => true,
    }
}

/// Evaluate arm switch condition with bounds validation.
/// Returns false if `arm_switch` is 0 (unassigned) or out of range.
#[inline]
pub fn eval_arm_switch(arm_switch: u8, switches: &Switches) -> bool {
    if arm_switch > 0 && arm_switch <= 44 {
        is_switch_active(arm_switch, switches)
    } else {
        false
    }
}

/// Evaluate a 3-mode feature switch setting (`0 = Off`, `1 = On / Armed`, `2..=max+1 = Switch (sw - 1)`).
#[inline]
pub fn eval_feature_switch(
    sw_mode: u8,
    on_when_one: bool,
    max_mix_switch: u8,
    switches: &Switches,
) -> bool {
    match sw_mode {
        0 => false,
        1 => on_when_one,
        sw if sw <= (max_mix_switch + 1) => eval_arm_switch(sw - 1, switches),
        _ => false,
    }
}

/// Determine whether High Rates (true) or Low Rates (false) are active for legacy D/R switch IDs (`0..=6`)
/// or full switch condition IDs (`0..=44`).
#[inline]
pub fn is_dr_high(dr_switch: u8, switches: &Switches) -> bool {
    match dr_switch {
        0 => true,
        1 => switches.sa == SwitchPos::Up,
        2 => switches.sb == SwitchPos::Up,
        3 => switches.sc == SwitchPos::Up,
        4 => switches.sd == SwitchPos::Up,
        5 => switches.se == SwitchPos::Up,
        6 => switches.sf == SwitchPos::Up,
        _ => true,
    }
}

/// Evaluate whether a flight timer trigger condition is satisfied.
pub fn is_timer_active(
    timer_source: u8,
    throttle: i16,
    latched: &mut bool,
    switches: &Switches,
    is_armed: bool,
) -> bool {
    if !is_armed {
        return false;
    }
    match timer_source {
        0 => false,
        1 => throttle > -900,
        2 => {
            if throttle > -900 {
                *latched = true;
            }
            *latched
        }
        3 => true,
        sw if (4..=17).contains(&sw) => is_switch_active(sw - 3, switches),
        _ => false,
    }
}

/// Evaluate the raw boolean condition of a single `LogicalSwitch` (`L1..L8`) in Q10.
pub fn evaluate_logical_switch_raw_q10(
    ls: &LogicalSwitch,
    sticks_q10: &[i32; 5],
    pots_q10: &[i32],
    switches: &Switches,
    channels: &[i32; NUM_CHANNELS],
    gvars: &[i16; 5],
    mixer_outs: &[i32; 12],
) -> bool {
    if ls.func == 0 {
        return false;
    }

    if ls.and_sw > 0 && !is_switch_active(ls.and_sw, switches) {
        return false;
    }

    match ls.func {
        1..=4 => {
            let a = evaluate_source_q10(
                ls.v1, sticks_q10, pots_q10, switches, channels, gvars, mixer_outs,
            );
            let x = pct_to_q10(ls.v2 as i32);
            match ls.func {
                1 => a > x,
                2 => a < x,
                3 => a.abs() > x,
                4 => a.abs() < x,
                _ => false,
            }
        }
        5..=7 => {
            let s1 = ls.v1 > 0 && is_switch_active(ls.v1, switches);
            let s2 = ls.v2 > 0 && is_switch_active(ls.v2 as u8, switches);
            match ls.func {
                5 => s1 && s2,
                6 => s1 || s2,
                7 => s1 != s2,
                _ => false,
            }
        }
        8..=10 => {
            let a = evaluate_source_q10(
                ls.v1, sticks_q10, pots_q10, switches, channels, gvars, mixer_outs,
            );
            let b = evaluate_source_q10(
                ls.v2.max(0) as u8,
                sticks_q10,
                pots_q10,
                switches,
                channels,
                gvars,
                mixer_outs,
            );
            match ls.func {
                8 => (a - b).abs() <= 21, // ±2% tolerance in Q10
                9 => a > b,
                10 => a < b,
                _ => false,
            }
        }
        _ => false,
    }
}

/// Real-time state bank for `L1..L8` Delay and Duration timers.
#[derive(Copy, Clone, Debug, Default)]
pub struct LogicalSwitchBank {
    delay_ms: [u16; NUM_LOGICAL_SWITCHES],
    duration_ms: [u16; NUM_LOGICAL_SWITCHES],
    prev_raw: u8,
    active_mask: u8,
}

impl LogicalSwitchBank {
    pub const fn new() -> Self {
        Self {
            delay_ms: [0; NUM_LOGICAL_SWITCHES],
            duration_ms: [0; NUM_LOGICAL_SWITCHES],
            prev_raw: 0,
            active_mask: 0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Evaluate all 8 logical switches sequentially (`L1` -> `L8`) and advance Delay/Duration timers.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        dt_ms: u16,
        cond_sticks: &[i16; 4],
        pots: &[i16],
        switches: &mut Switches,
        channels: &[i32; NUM_CHANNELS],
        model: &ModelConfig,
    ) -> u8 {
        let sticks_q10 = [
            stick_1000_to_q10(cond_sticks[0]),
            stick_1000_to_q10(cond_sticks[1]),
            stick_1000_to_q10(cond_sticks[2]),
            stick_1000_to_q10(cond_sticks[3]),
            stick_1000_to_q10(cond_sticks[2]),
        ];
        let mut pots_q10 = [0i32; 8];
        for (dst, &src) in pots_q10.iter_mut().zip(pots.iter()) {
            *dst = stick_1000_to_q10(src);
        }
        let zero_mix_outs = [0i32; 12];

        switches.logical = self.active_mask;
        for (i, ls) in model.logical_switches.iter().enumerate() {
            let bit = 1u8 << i;
            let raw = evaluate_logical_switch_raw_q10(
                ls,
                &sticks_q10,
                &pots_q10[..pots.len().min(8)],
                switches,
                channels,
                &model.gvars,
                &zero_mix_outs,
            );
            let was_raw = (self.prev_raw & bit) != 0;
            if raw {
                self.prev_raw |= bit;
            } else {
                self.prev_raw &= !bit;
            }

            let delay_target_ms = (ls.delay_tenths as u16) * 100;
            let dur_target_ms = (ls.duration_tenths as u16) * 100;

            let triggered = if !raw {
                self.delay_ms[i] = 0;
                false
            } else if delay_target_ms == 0 {
                !was_raw || dur_target_ms == 0
            } else if self.delay_ms[i] < delay_target_ms {
                self.delay_ms[i] = self.delay_ms[i].saturating_add(dt_ms);
                self.delay_ms[i] >= delay_target_ms
            } else {
                dur_target_ms == 0
            };

            let out = if dur_target_ms > 0 {
                if triggered {
                    self.duration_ms[i] = dur_target_ms;
                    true
                } else if self.duration_ms[i] > 0 {
                    self.duration_ms[i] = self.duration_ms[i].saturating_sub(dt_ms);
                    self.duration_ms[i] > 0
                } else {
                    false
                }
            } else {
                raw && self.delay_ms[i] >= delay_target_ms
            };

            if out {
                self.active_mask |= bit;
                switches.logical |= bit;
            } else {
                self.active_mask &= !bit;
                switches.logical &= !bit;
            }
        }
        self.active_mask
    }
}
