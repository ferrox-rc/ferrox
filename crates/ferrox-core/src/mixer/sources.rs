//! Physical, channel, global-variable, logical-switch, and predefined-mixer source evaluation
//! in the signed ±1024 Q10 fixed-point domain.

use super::fx::{stick_1000_to_q10, FX_CENTER, FX_MAX, FX_MIN};
use super::NUM_CHANNELS;
use crate::input::{SwitchPos, Switches};

/// First source ID reserved for predefined mixer outputs (`60..=71`, 4 instances x 3 outputs).
pub const MIXER_OUT_SRC_BASE: u8 = 60;
/// Last source ID reserved for predefined mixer outputs.
pub const MIXER_OUT_SRC_MAX: u8 = 71;
/// Curved throttle source ID (`THC`, compatibility with PR #38).
pub const SRC_THC: u8 = 58;

/// Physical and virtual source identifiers for inputs, channels, and mix lines.
#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MixSource {
    None = 0,
    Roll = 1,
    Pitch = 2,
    Throttle = 3,
    Yaw = 4,
    Vra = 5,
    Vrb = 6,
    Sa = 7,
    Sb = 8,
    Sc = 9,
    Sd = 10,
    Max = 11,
    // 12..=29 map to Channel 1..18 (stage-5 pre-mix values)
    ThrUnipolar = 30,
    Se = 31,
    Sf = 32,
    Sg = 33,
    Sh = 34,
    Swi = 35,
    Swj = 36,
    Vrc = 37,
    Vrd = 38,
    Vre = 39,
    Vrf = 40,
    VrG = 41,
    SwK = 42,
    SwL = 43,
    SwM = 44,
    Gv1 = 45,
    Gv2 = 46,
    Gv3 = 47,
    Gv4 = 48,
    Gv5 = 49,
    // 50..=57 map to Logical Switches L1..L8
    Thc = 58,
    // 60..=71 map to Predefined Mixer outputs (instance 0..4, output 0..3)
}

#[inline(always)]
const fn switch_pos_to_q10(pos: SwitchPos) -> i32 {
    match pos {
        SwitchPos::Up => FX_MIN,
        SwitchPos::Mid => FX_CENTER,
        SwitchPos::Down => FX_MAX,
    }
}

/// Evaluate a source identifier (`0..=71`) to a Q10 value in `[-1024, +1024]`.
///
/// * `sticks_q10`: `[Roll, Pitch, RawThr, Yaw, CurvedThr]` in Q10 (`[-1024, +1024]`)
/// * `pots_q10`: auxiliary pots `VRA..VRG` in Q10 (`[-1024, +1024]`)
/// * `switches`: physical + logical switch states
/// * `channels`: stage-5 channel values in Q10
/// * `gvars`: global variables `GV1..GV5` in `[-1000, +1000]`
/// * `mixer_outs`: predefined mixer outputs `[i32; 12]` (4 instances x 3 outputs)
pub fn evaluate_source_q10(
    src: u8,
    sticks_q10: &[i32; 5],
    pots_q10: &[i32],
    switches: &Switches,
    channels: &[i32; NUM_CHANNELS],
    gvars: &[i16; 5],
    mixer_outs: &[i32; 12],
) -> i32 {
    match src {
        0 => FX_CENTER,
        1..=4 => sticks_q10[(src - 1) as usize],
        5 => pots_q10.first().copied().unwrap_or(0),
        6 => pots_q10.get(1).copied().unwrap_or(0),
        7 => switch_pos_to_q10(switches.sa),
        8 => switch_pos_to_q10(switches.sb),
        9 => switch_pos_to_q10(switches.sc),
        10 => switch_pos_to_q10(switches.sd),
        11 => FX_MAX,
        12..=29 => channels[(src - 12) as usize],
        30 => ((sticks_q10[4] + FX_MAX) >> 1).clamp(0, FX_MAX),
        31 => switch_pos_to_q10(switches.se),
        32 => switch_pos_to_q10(switches.sf),
        33 => switch_pos_to_q10(switches.sg),
        34 => switch_pos_to_q10(switches.sh),
        35 => switch_pos_to_q10(switches.swi),
        36 => switch_pos_to_q10(switches.swj),
        37..=41 => pots_q10.get((src - 35) as usize).copied().unwrap_or(0),
        42 => switch_pos_to_q10(switches.swk),
        43 => switch_pos_to_q10(switches.swl),
        44 => switch_pos_to_q10(switches.swm),
        45..=49 => stick_1000_to_q10(gvars[(src - 45) as usize]),
        50..=57 => {
            if (switches.logical & (1 << (src - 50))) != 0 {
                FX_MAX
            } else {
                FX_MIN
            }
        }
        58 => sticks_q10[4],
        60..=71 => mixer_outs[(src - 60) as usize],
        _ => FX_CENTER,
    }
}
