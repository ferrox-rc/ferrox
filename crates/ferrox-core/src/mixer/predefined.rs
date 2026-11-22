//! Static Predefined Mixer Registry (`MixerKind` & `MixerInst`).
//!
//! All predefined mixers are generic, static-dispatch functions (zero `dyn`, zero heap)
//! operating on Q10 (±1024) inputs and returning up to 3 Q10 outputs.
//! Elevon/Delta, V-Tail, Diff Thrust (Baitboat), and Tank use full-throw `a + b`
//! with proportional saturation when `max(|L|, |R|) > 1024` to preserve steering authority.

use super::fx::{apply_differential_q10, mul_q10, pct_to_q10, proportional_saturate_pair};

/// Maximum number of simultaneous predefined mixer instances per model.
pub const NUM_PREDEF_MIXERS: usize = 4;

/// Static parameter metadata for UI rendering and sanitization.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ParamDesc {
    pub name: &'static str,
    pub min: i8,
    pub max: i8,
    pub default: i8,
}

/// Predefined mixer topologies supported by Ferrox.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum MixerKind {
    #[default]
    None = 0,
    Delta = 1,
    VTail = 2,
    Flaperon = 3,
    DiffThrust = 4,
    Tank = 5,
    MTail = 6,
    Crow = 7,
}

impl MixerKind {
    pub const COUNT: u8 = 8;

    #[inline(always)]
    pub const fn from_u8(val: u8) -> Self {
        match val {
            1 => Self::Delta,
            2 => Self::VTail,
            3 => Self::Flaperon,
            4 => Self::DiffThrust,
            5 => Self::Tank,
            6 => Self::MTail,
            7 => Self::Crow,
            _ => Self::None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "OFF",
            Self::Delta => "DELTA/ELEVON",
            Self::VTail => "V-TAIL",
            Self::Flaperon => "FLAPERON",
            Self::DiffThrust => "DIFF THRUST",
            Self::Tank => "TANK",
            Self::MTail => "M-TAIL",
            Self::Crow => "CROW/BFLY",
        }
    }

    pub const fn inputs(self) -> &'static [&'static str] {
        match self {
            Self::None => &[],
            Self::Delta => &["Pitch", "Roll"],
            Self::VTail => &["Pitch", "Yaw"],
            Self::Flaperon => &["Roll", "Flap"],
            Self::DiffThrust => &["Thr", "Steer"],
            Self::Tank => &["Drive", "Steer"],
            Self::MTail => &["Pitch", "Yaw", "Roll"],
            Self::Crow => &["Brake", "Roll"],
        }
    }

    pub const fn outputs(self) -> &'static [&'static str] {
        match self {
            Self::None => &[],
            Self::Delta | Self::VTail | Self::Flaperon | Self::DiffThrust | Self::Tank | Self::MTail => {
                &["L", "R"]
            }
            Self::Crow => &["AilL", "AilR", "EleC"],
        }
    }

    pub const fn params(self) -> &'static [ParamDesc] {
        const EMPTY: &[ParamDesc] = &[];
        const DELTA_P: &[ParamDesc] = &[
            ParamDesc { name: "Diff%", min: -100, max: 100, default: 0 },
            ParamDesc { name: "Roll%", min: 10, max: 100, default: 100 },
        ];
        const VTAIL_P: &[ParamDesc] = &[
            ParamDesc { name: "Yaw%", min: 10, max: 100, default: 100 },
        ];
        const FLAPERON_P: &[ParamDesc] = &[
            ParamDesc { name: "Diff%", min: -100, max: 100, default: 0 },
            ParamDesc { name: "Flap%", min: -100, max: 100, default: 50 },
        ];
        const DIFF_THRUST_P: &[ParamDesc] = &[
            ParamDesc { name: "Steer%", min: 10, max: 100, default: 100 },
        ];
        const TANK_P: &[ParamDesc] = &[
            ParamDesc { name: "Steer%", min: 10, max: 100, default: 100 },
            ParamDesc { name: "RevInv", min: 0, max: 1, default: 1 },
        ];
        const MTAIL_P: &[ParamDesc] = &[
            ParamDesc { name: "Ail%", min: -100, max: 100, default: 20 },
            ParamDesc { name: "Rud%", min: -100, max: 100, default: 20 },
        ];
        const CROW_P: &[ParamDesc] = &[
            ParamDesc { name: "AilUp%", min: -100, max: 100, default: 30 },
            ParamDesc { name: "EleC%", min: -100, max: 100, default: -10 },
        ];

        match self {
            Self::None => EMPTY,
            Self::Delta => DELTA_P,
            Self::VTail => VTAIL_P,
            Self::Flaperon => FLAPERON_P,
            Self::DiffThrust => DIFF_THRUST_P,
            Self::Tank => TANK_P,
            Self::MTail => MTAIL_P,
            Self::Crow => CROW_P,
        }
    }
}

/// Persistent configuration for a single predefined mixer instance (8 bytes, naturally aligned).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct MixerInst {
    pub kind: u8,
    pub in_a: u8,
    pub in_b: u8,
    pub in_c: u8,
    pub p: [i8; 4],
}

const _: () = assert!(core::mem::size_of::<MixerInst>() == 8);

impl MixerInst {
    pub const fn disabled() -> Self {
        Self {
            kind: MixerKind::None as u8,
            in_a: 0,
            in_b: 0,
            in_c: 0,
            p: [0; 4],
        }
    }

    /// Create a default-configured instance for a given [`MixerKind`].
    pub const fn default_for_kind(kind: MixerKind) -> Self {
        match kind {
            MixerKind::None => Self::disabled(),
            MixerKind::Delta => Self {
                kind: MixerKind::Delta as u8,
                in_a: 2, // Pitch
                in_b: 1, // Roll
                in_c: 0,
                p: [0, 100, 0, 0],
            },
            MixerKind::VTail => Self {
                kind: MixerKind::VTail as u8,
                in_a: 2, // Pitch
                in_b: 4, // Yaw
                in_c: 0,
                p: [100, 0, 0, 0],
            },
            MixerKind::Flaperon => Self {
                kind: MixerKind::Flaperon as u8,
                in_a: 1, // Roll
                in_b: 8, // SB (or Flap source)
                in_c: 0,
                p: [0, 50, 0, 0],
            },
            MixerKind::DiffThrust => Self {
                kind: MixerKind::DiffThrust as u8,
                in_a: 58, // THC (curved throttle)
                in_b: 4,  // Yaw
                in_c: 0,
                p: [100, 0, 0, 0],
            },
            MixerKind::Tank => Self {
                kind: MixerKind::Tank as u8,
                in_a: 3, // Raw Thr / Drive
                in_b: 1, // Roll / Steer
                in_c: 0,
                p: [100, 1, 0, 0], // Steer 100%, RevInv = 1
            },
            MixerKind::MTail => Self {
                kind: MixerKind::MTail as u8,
                in_a: 2, // Pitch
                in_b: 4, // Yaw
                in_c: 1, // Roll
                p: [20, 20, 0, 0],
            },
            MixerKind::Crow => Self {
                kind: MixerKind::Crow as u8,
                in_a: 8, // Brake (SB)
                in_b: 1, // Roll
                in_c: 0,
                p: [30, -10, 0, 0],
            },
        }
    }

    pub fn sanitize(&mut self) {
        if self.kind >= MixerKind::COUNT {
            *self = Self::disabled();
            return;
        }
        let k = MixerKind::from_u8(self.kind);
        for (idx, desc) in k.params().iter().enumerate() {
            if idx < 4 {
                self.p[idx] = self.p[idx].clamp(desc.min, desc.max);
            }
        }
    }
}

/// Evaluate a predefined mixer instance in Q10 (`[-1024, +1024]`), returning up to 3 outputs.
pub fn eval_predefined_mixer(kind: MixerKind, inp: &[i32; 3], p: &[i8; 4]) -> [i32; 3] {
    match kind {
        MixerKind::None => [0, 0, 0],
        MixerKind::Delta => {
            let pitch = inp[0];
            let roll_gain = pct_to_q10(if p[1] == 0 { 100 } else { p[1] as i32 });
            let roll = mul_q10(inp[1], roll_gain);
            let left_r = apply_differential_q10(-roll, p[0]);
            let right_r = apply_differential_q10(roll, p[0]);
            let (l, r) = proportional_saturate_pair(pitch + left_r, pitch + right_r);
            [l, r, 0]
        }
        MixerKind::VTail => {
            let pitch = inp[0];
            let yaw_gain = pct_to_q10(if p[0] == 0 { 100 } else { p[0] as i32 });
            let yaw = mul_q10(inp[1], yaw_gain);
            let (l, r) = proportional_saturate_pair(pitch + yaw, pitch - yaw);
            [l, r, 0]
        }
        MixerKind::Flaperon => {
            let roll = inp[0];
            let flap_gain = pct_to_q10(p[1] as i32);
            let flap = mul_q10(inp[1], flap_gain);
            let left_r = apply_differential_q10(roll, p[0]);
            let right_r = apply_differential_q10(-roll, p[0]);
            let (l, r) = proportional_saturate_pair(left_r + flap, right_r + flap);
            [l, r, 0]
        }
        MixerKind::DiffThrust => {
            let thr = inp[0];
            let steer_gain = pct_to_q10(if p[0] == 0 { 100 } else { p[0] as i32 });
            let steer = mul_q10(inp[1], steer_gain);
            let (l, r) = proportional_saturate_pair(thr + steer, thr - steer);
            [l, r, 0]
        }
        MixerKind::Tank => {
            // PR #38 tracked vehicle steering: optional steering inversion when driving in reverse
            let drive = inp[0];
            let steer_gain = pct_to_q10(if p[0] == 0 { 100 } else { p[0] as i32 });
            let mut steer = mul_q10(inp[1], steer_gain);
            if p[1] != 0 && drive < -20 {
                steer = -steer;
            }
            let (l, r) = proportional_saturate_pair(drive + steer, drive - steer);
            [l, r, 0]
        }
        MixerKind::MTail => {
            let pitch = inp[0];
            let yaw = inp[1];
            let roll = inp[2];
            let assist = mul_q10(roll, pct_to_q10(p[0] as i32))
                + mul_q10(yaw, pct_to_q10(p[1] as i32));
            let (l, r) = proportional_saturate_pair(pitch + yaw + assist, pitch - yaw - assist);
            [l, r, 0]
        }
        MixerKind::Crow => {
            // Brake input (unipolar 0..1024 from signed -1024..+1024)
            let brake_uni = ((inp[0] + 1024) >> 1).clamp(0, 1024);
            let roll = inp[1];
            let ail_up = mul_q10(brake_uni, pct_to_q10(p[0] as i32));
            let ele_comp = mul_q10(brake_uni, pct_to_q10(p[1] as i32));
            [roll + ail_up, -roll + ail_up, ele_comp]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_full_throw_and_proportional_saturation() {
        // Pure pitch -> full ±1024 on both elevons (not halved to ±512)
        let out = eval_predefined_mixer(MixerKind::Delta, &[1024, 0, 0], &[0, 100, 0, 0]);
        assert_eq!(out, [1024, 1024, 0]);

        // Full pitch + half roll -> 1024-512=512, 1024+512=1536 -> saturated proportionally
        let sat = eval_predefined_mixer(MixerKind::Delta, &[1024, 512, 0], &[0, 100, 0, 0]);
        assert_eq!(sat[1], 1024);
        assert_eq!(sat[0], 341); // 512 * 1024 / 1536
    }

    #[test]
    fn test_vtail_full_throw() {
        let out = eval_predefined_mixer(MixerKind::VTail, &[1024, 0, 0], &[100, 0, 0, 0]);
        assert_eq!(out, [1024, 1024, 0]);
        let yaw = eval_predefined_mixer(MixerKind::VTail, &[0, 1024, 0], &[100, 0, 0, 0]);
        assert_eq!(yaw, [1024, -1024, 0]);
    }

    #[test]
    fn test_tank_forward_and_reverse_steering_inversion() {
        // Forward + right steer: left track faster than right track
        let fwd = eval_predefined_mixer(MixerKind::Tank, &[512, 256, 0], &[100, 1, 0, 0]);
        assert_eq!(fwd, [768, 256, 0]);

        // Reverse + right steer with RevInv=1: steering inverted for car-like reverse
        let rev = eval_predefined_mixer(MixerKind::Tank, &[-512, 256, 0], &[100, 1, 0, 0]);
        assert_eq!(rev, [-768, -256, 0]);

        // Pivot in place (drive=0)
        let pivot = eval_predefined_mixer(MixerKind::Tank, &[0, 512, 0], &[100, 1, 0, 0]);
        assert_eq!(pivot, [512, -512, 0]);
    }

    #[test]
    fn test_crow_butterfly_outputs() {
        let out = eval_predefined_mixer(MixerKind::Crow, &[1024, 0, 0], &[30, -10, 0, 0]);
        // At full brake (uni = 1024): AilL = +30% (~307), AilR = +30% (~307), EleC = -10% (~-102)
        assert!(out[0] > 300 && out[0] < 315);
        assert_eq!(out[0], out[1]);
        assert!(out[2] < -95 && out[2] > -110);
    }
}
