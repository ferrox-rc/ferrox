//! 11-Stage Fixed-Point (±1024 Q10) Flight Mixer Pipeline & CompiledModel RAM Cache.
//!
//! Stage order:
//! 1. Sources (`±1024`)
//! 2. Input modifiers (`rate * curve(x)` or spline curve)
//! 3. Digital trims (Q10, per `trim_map`, division-free T-idle)
//! 4. Predefined mixers (`Delta`, `VTail`, `Flaperon`, `DiffThrust`, `Tank`, `MTail`, `Crow`) -> IDs `60..=71`
//! 5. Channel source select (`channels[i].source`) -> snapshot `stage5_ch`
//! 6. Freeform mix lines (`mixes[0..32]`, unclamped; `CHx` reads `stage5_ch`)
//! 7. Channel curve modifiers (`chan_mods[0..4]`)
//! 8. Channel reverse (exact sign flip)
//! 9. Subtrim & endpoints (the ONLY clamp)
//! 10. Microsecond conversion (`to_us`, division-free & reverse-symmetric)
//! 11. Motor safety lockout (AFTER reverse & endpoints using precomputed `safe_us`)

use super::fx::{
    apply_rate_curve, mul_q10, pct_to_q10, stick_1000_to_q10, to_us, CHANNEL_CENTER_US, FX_MAX,
    FX_MIN, FX_ONE,
};
use super::predefined::{eval_predefined_mixer, MixerKind, NUM_PREDEF_MIXERS};
use super::sources::evaluate_source_q10;
use super::switches::{eval_arm_switch, evaluate_logical_switch_raw_q10, is_switch_active};
use super::NUM_CHANNELS;
use crate::curve::evaluate_curve_q10;
use crate::input::{SwitchPos, Switches};
use crate::storage::{ModelConfig, NUM_MIX_LINES};
use crate::trim::TrimController;

pub const NUM_INPUT_MODS: usize = 8;
pub const NUM_CHAN_MODS: usize = 4;

/// Per-channel output configuration (4 bytes, naturally aligned).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ChannelCfg {
    /// Source ID (`0..=71`: sticks, pots, switches, GVs, L1..L8, THC, or mixer outputs `60..=71`)
    pub source: u8,
    /// Subtrim offset in percent (`-100..=+100`, 1% ≈ 5 µs, clamped to ±25% in UI)
    pub subtrim: i8,
    /// Minimum endpoint in percent (`-100..=0`, default `-100`)
    pub min: i8,
    /// Maximum endpoint in percent (`0..=100`, default `+100`)
    pub max: i8,
}

const _: () = assert!(core::mem::size_of::<ChannelCfg>() == 4);

impl ChannelCfg {
    pub const fn new(source: u8) -> Self {
        Self {
            source,
            subtrim: 0,
            min: -100,
            max: 100,
        }
    }

    pub fn sanitize(&mut self) {
        if self.source > 71 {
            self.source = 0;
        }
        self.subtrim = self.subtrim.clamp(-100, 100);
        self.min = self.min.clamp(-100, 0);
        self.max = self.max.clamp(0, 100);
    }
}

/// Per-input conditioning modifier replacing legacy DR/Expo & throttle curve fields (8 bytes).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct InputMod {
    /// Target input (`0` = disabled, `1` = Roll, `2` = Pitch, `3` = Thr, `4` = Yaw, `5..=41` pots)
    pub input: u8,
    /// `0` = Dual-Rate + Cubic Expo, `1` = Curve (`curve_ref`)
    pub kind: u8,
    /// Switch condition (`0` = Always High, `1..=44` = High when active else Low)
    pub switch: u8,
    /// High rate in percent (`10..=100`, default `100`)
    pub rate_hi: u8,
    /// Low rate in percent (`10..=100`, default `70`)
    pub rate_lo: u8,
    /// High expo in percent (`-100..=+100`, default `0`)
    pub expo_hi: i8,
    /// Low expo in percent (`-100..=+100`, default `0`)
    pub expo_lo: i8,
    /// Curve slot index (`0` = model throttle/main curve)
    pub curve_ref: u8,
}

const _: () = assert!(core::mem::size_of::<InputMod>() == 8);

impl InputMod {
    pub const fn disabled() -> Self {
        Self {
            input: 0,
            kind: 0,
            switch: 0,
            rate_hi: 100,
            rate_lo: 70,
            expo_hi: 0,
            expo_lo: 0,
            curve_ref: 0,
        }
    }

    pub const fn default_axis(input: u8) -> Self {
        Self {
            input,
            kind: 0,
            switch: 0,
            rate_hi: 100,
            rate_lo: 70,
            expo_hi: 0,
            expo_lo: 0,
            curve_ref: 0,
        }
    }

    pub fn sanitize(&mut self) {
        if self.input > 41 {
            self.input = 0;
        }
        if self.kind > 1 {
            self.kind = 0;
        }
        if self.switch > 44 {
            self.switch = 0;
        }
        self.rate_hi = self.rate_hi.clamp(10, 100);
        self.rate_lo = self.rate_lo.clamp(10, 100);
        self.expo_hi = self.expo_hi.clamp(-100, 100);
        self.expo_lo = self.expo_lo.clamp(-100, 100);
        if self.curve_ref > 0 {
            self.curve_ref = 0;
        }
    }
}

/// Post-mix curve-only channel modifier (4 bytes).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ChanMod {
    /// Target channel (`0` = disabled, `1..=18`)
    pub channel: u8,
    /// Curve reference (`0` = model curve)
    pub curve_ref: u8,
    /// Activation switch condition (`0` = Always On, `1..=44`)
    pub switch: u8,
    pub _pad: u8,
}

const _: () = assert!(core::mem::size_of::<ChanMod>() == 4);

impl ChanMod {
    pub const fn disabled() -> Self {
        Self {
            channel: 0,
            curve_ref: 0,
            switch: 0,
            _pad: 0,
        }
    }

    pub fn sanitize(&mut self) {
        if self.channel > NUM_CHANNELS as u8 {
            self.channel = 0;
        }
        if self.curve_ref > 0 {
            self.curve_ref = 0;
        }
        if self.switch > 44 {
            self.switch = 0;
        }
        self._pad = 0;
    }
}

/// RAM-only compiled model cache holding pre-scaled Q10 parameters and precomputed safe pulses.
/// Rebuilt on model load or menu edit so the per-frame flight pipeline performs zero `% -> Q10` divides.
#[derive(Copy, Clone, Debug)]
pub struct CompiledModel {
    pub input_rate_hi_q10: [i32; NUM_INPUT_MODS],
    pub input_rate_lo_q10: [i32; NUM_INPUT_MODS],
    pub mix_weight_q10: [i32; NUM_MIX_LINES],
    pub mix_offset_q10: [i32; NUM_MIX_LINES],
    pub ch_subtrim_q10: [i32; NUM_CHANNELS],
    pub ch_min_q10: [i32; NUM_CHANNELS],
    pub ch_max_q10: [i32; NUM_CHANNELS],
    /// Precomputed physical safe microsecond pulse for each channel (applied AFTER reverse & endpoints).
    pub safe_us: [u16; NUM_CHANNELS],
}

impl CompiledModel {
    pub fn from_model(model: &ModelConfig) -> Self {
        let mut input_rate_hi_q10 = [FX_ONE; NUM_INPUT_MODS];
        let mut input_rate_lo_q10 = [pct_to_q10(70); NUM_INPUT_MODS];
        for i in 0..NUM_INPUT_MODS {
            input_rate_hi_q10[i] = pct_to_q10(model.input_mods[i].rate_hi.clamp(10, 100) as i32);
            input_rate_lo_q10[i] = pct_to_q10(model.input_mods[i].rate_lo.clamp(10, 100) as i32);
        }

        let mut mix_weight_q10 = [0i32; NUM_MIX_LINES];
        let mut mix_offset_q10 = [0i32; NUM_MIX_LINES];
        for i in 0..NUM_MIX_LINES {
            mix_weight_q10[i] = pct_to_q10(model.mixes[i].weight as i32);
            mix_offset_q10[i] = pct_to_q10(model.mixes[i].offset as i32);
        }

        let mut ch_subtrim_q10 = [0i32; NUM_CHANNELS];
        let mut ch_min_q10 = [FX_MIN; NUM_CHANNELS];
        let mut ch_max_q10 = [FX_MAX; NUM_CHANNELS];
        let mut safe_us = [CHANNEL_CENTER_US; NUM_CHANNELS];

        for i in 0..NUM_CHANNELS {
            let cfg = model.channels[i];
            ch_subtrim_q10[i] = pct_to_q10(cfg.subtrim as i32);
            ch_min_q10[i] = pct_to_q10(cfg.min.clamp(-100, 0) as i32);
            ch_max_q10[i] = pct_to_q10(cfg.max.clamp(0, 100) as i32);

            let is_bidir = (model.bidir_mask & (1u32 << i)) != 0;
            let safe_q10 = if is_bidir {
                // Bidirectional motor neutral (1500 µs + subtrim, clamped to endpoints)
                ch_subtrim_q10[i].clamp(ch_min_q10[i], ch_max_q10[i])
            } else {
                // Unidirectional motor physical idle (min endpoint = 988 µs at -100%)
                ch_min_q10[i]
            };
            safe_us[i] = to_us(safe_q10);
        }

        Self {
            input_rate_hi_q10,
            input_rate_lo_q10,
            mix_weight_q10,
            mix_offset_q10,
            ch_subtrim_q10,
            ch_min_q10,
            ch_max_q10,
            safe_us,
        }
    }
}

/// Apply Q10 digital trim to a stick axis without division.
/// Each trim step (`-25..=+25`) equals `8` Q10 units (`4 µs`, matching `±100 µs` full trim authority).
#[inline]
fn apply_trim_q10(val: i32, trim_steps: i8, is_throttle: bool, trim_mode: u8) -> i32 {
    let offset = (trim_steps as i32) << 3; // 25 * 8 = 200 Q10 = 100 µs
    if !is_throttle {
        return val + offset;
    }
    match trim_mode {
        0 => val, // OFF (Lock)
        1 => {
            // IDLE (T-Trim): scales linearly from full trim at idle (-1024) to 0 at full throttle (+1024)
            let idle_factor = (FX_MAX - val.clamp(FX_MIN, FX_MAX)) >> 1; // 0..=1024
            val + mul_q10(offset, idle_factor)
        }
        _ => val + offset, // LINEAR
    }
}

/// Execute the complete 11-stage ±1024 fixed-point flight mixer pipeline.
///
/// * `raw_roll`, `raw_pitch`, `raw_throttle`, `raw_yaw`: normalized stick inputs in `[-1000, +1000]`
/// * `pots`: auxiliary pots in `[-1000, +1000]`
/// * `switches_in`: current physical & delayed logical switch state
/// * `model`: persistent `ModelConfig` v2
/// * `compiled`: precomputed `CompiledModel` Q10 cache
/// * `trims`: active digital trim offsets
/// * `motor_locked`: competition motor lockout flag (e.g. F5J single-run rule)
#[allow(clippy::too_many_arguments)]
pub fn compute_channels_compiled(
    raw_roll: i16,
    raw_pitch: i16,
    raw_throttle: i16,
    raw_yaw: i16,
    pots: &[i16],
    switches_in: &Switches,
    model: &ModelConfig,
    compiled: &CompiledModel,
    trims: &TrimController,
    motor_locked: bool,
) -> [u16; NUM_CHANNELS] {
    let mut local_switches = *switches_in;
    let switches = &mut local_switches;

    // Stage 1: Convert physical sources to signed Q10 [-1024, +1024]
    let raw_thr_q10 = stick_1000_to_q10(raw_throttle);
    let curved_thr_q10 = evaluate_curve_q10(
        raw_thr_q10,
        model.thr_curve_pts,
        model.thr_curve_smooth != 0,
        &model.thr_curve,
    );
    let mut sticks_q10 = [
        stick_1000_to_q10(raw_roll),
        stick_1000_to_q10(raw_pitch),
        raw_thr_q10,
        stick_1000_to_q10(raw_yaw),
        curved_thr_q10,
    ];
    let mut pots_q10 = [0i32; 8];
    let pot_len = pots.len().min(8);
    for i in 0..pot_len {
        pots_q10[i] = stick_1000_to_q10(pots[i]);
    }

    // Stage 2: Input modifiers (Dual Rate + Expo, or Curve)
    for (idx, m) in model.input_mods.iter().enumerate() {
        if m.input == 0 {
            continue;
        }
        let high = m.switch == 0 || is_switch_active(m.switch, switches);
        let rate_q10 = if high {
            compiled.input_rate_hi_q10[idx]
        } else {
            compiled.input_rate_lo_q10[idx]
        };
        let expo = if high { m.expo_hi } else { m.expo_lo };

        let apply = |v: i32| -> i32 {
            if m.kind == 1 {
                let c = evaluate_curve_q10(
                    v,
                    model.thr_curve_pts,
                    model.thr_curve_smooth != 0,
                    &model.thr_curve,
                );
                mul_q10(c, rate_q10)
            } else {
                apply_rate_curve(v, rate_q10, expo)
            }
        };

        match m.input {
            1..=4 => sticks_q10[(m.input - 1) as usize] = apply(sticks_q10[(m.input - 1) as usize]),
            5 => pots_q10[0] = apply(pots_q10[0]),
            6 => pots_q10[1] = apply(pots_q10[1]),
            37..=41 => {
                let p = (m.input - 35) as usize;
                if p < pot_len {
                    pots_q10[p] = apply(pots_q10[p]);
                }
            }
            _ => {}
        }
    }

    // Stage 3: Digital trims (+Trim in Q10, per trim_map)
    let trim_vals = [
        trims.values.roll,
        trims.values.pitch,
        trims.values.throttle,
        trims.values.yaw,
    ];
    for (axis, &trim_step) in trim_vals.iter().enumerate() {
        let target = model.trim_map[axis];
        if (1..=4).contains(&target) {
            let s_idx = (target - 1) as usize;
            let is_thr = target == 3;
            sticks_q10[s_idx] =
                apply_trim_q10(sticks_q10[s_idx], trim_step, is_thr, model.trim_mode);
            if is_thr {
                sticks_q10[4] =
                    apply_trim_q10(sticks_q10[4], trim_step, true, model.trim_mode);
            }
        }
    }

    // Evaluate instantaneous (zero-delay/zero-duration) Logical Switches L1..L8
    let zero_chs = [0i32; NUM_CHANNELS];
    let mut mixer_outs = [0i32; NUM_PREDEF_MIXERS * 3];
    for (i, ls) in model.logical_switches.iter().enumerate() {
        if ls.func != 0 && ls.delay_tenths == 0 && ls.duration_tenths == 0 {
            let active = evaluate_logical_switch_raw_q10(
                ls,
                &sticks_q10,
                &pots_q10[..pot_len],
                switches,
                &zero_chs,
                &model.gvars,
                &mixer_outs,
            );
            if active {
                switches.logical |= 1 << i;
            } else {
                switches.logical &= !(1 << i);
            }
        }
    }

    // Stage 4: Predefined mixers (outputs placed at IDs 60..=71)
    for (inst_idx, inst) in model.mixers.iter().enumerate() {
        let kind = MixerKind::from_u8(inst.kind);
        if kind == MixerKind::None {
            continue;
        }
        let in_a = evaluate_source_q10(
            inst.in_a,
            &sticks_q10,
            &pots_q10[..pot_len],
            switches,
            &zero_chs,
            &model.gvars,
            &mixer_outs,
        );
        let in_b = evaluate_source_q10(
            inst.in_b,
            &sticks_q10,
            &pots_q10[..pot_len],
            switches,
            &zero_chs,
            &model.gvars,
            &mixer_outs,
        );
        let in_c = evaluate_source_q10(
            inst.in_c,
            &sticks_q10,
            &pots_q10[..pot_len],
            switches,
            &zero_chs,
            &model.gvars,
            &mixer_outs,
        );
        let outs = eval_predefined_mixer(kind, &[in_a, in_b, in_c], &inst.p);
        let base = inst_idx * 3;
        mixer_outs[base] = outs[0];
        mixer_outs[base + 1] = outs[1];
        mixer_outs[base + 2] = outs[2];
    }

    // Stage 5: Channel source select (snapshot saved for deterministic Stage-6 CHx reads)
    let mut stage5_ch = [0i32; NUM_CHANNELS];
    for (slot, ch_cfg) in stage5_ch.iter_mut().zip(model.channels.iter()) {
        let src = ch_cfg.source;
        if src != 0 {
            *slot = evaluate_source_q10(
                src,
                &sticks_q10,
                &pots_q10[..pot_len],
                switches,
                &zero_chs,
                &model.gvars,
                &mixer_outs,
            );
        }
    }
    let mut ch_vals = stage5_ch;

    // Stage 6: Freeform mix lines (unclamped i32; CHx references read deterministic stage5_ch)
    for (idx, mix) in model.mixes.iter().enumerate() {
        if mix.target_ch == 0 || mix.target_ch > NUM_CHANNELS as u8 {
            continue;
        }
        if !is_switch_active(mix.switch, switches) {
            continue;
        }

        let target_idx = (mix.target_ch - 1) as usize;
        let src_val = evaluate_source_q10(
            mix.source,
            &sticks_q10,
            &pots_q10[..pot_len],
            switches,
            &stage5_ch,
            &model.gvars,
            &mixer_outs,
        );

        let base_w = compiled.mix_weight_q10[idx];
        let effective_w = match mix.weight_src {
            0 => base_w,
            1..=18 => mul_q10(base_w, stage5_ch[(mix.weight_src - 1) as usize]),
            19..=36 => {
                let uni = ((stage5_ch[(mix.weight_src - 19) as usize] + FX_MAX) >> 1)
                    .clamp(0, FX_MAX);
                mul_q10(base_w, uni)
            }
            37..=41 => mul_q10(base_w, stick_1000_to_q10(model.gvars[(mix.weight_src - 37) as usize])),
            _ => base_w,
        };

        let term = mul_q10(src_val, effective_w) + compiled.mix_offset_q10[idx];
        match mix.mode {
            0 => ch_vals[target_idx] += term,
            1 => ch_vals[target_idx] = mul_q10(ch_vals[target_idx], term),
            2 => ch_vals[target_idx] = term,
            _ => {}
        }
    }

    // Stage 7: Channel curve modifiers
    for cm in model.chan_mods.iter() {
        if cm.channel == 0 || cm.channel > NUM_CHANNELS as u8 {
            continue;
        }
        if cm.switch != 0 && !is_switch_active(cm.switch, switches) {
            continue;
        }
        let idx = (cm.channel - 1) as usize;
        ch_vals[idx] = evaluate_curve_q10(
            ch_vals[idx],
            model.thr_curve_pts,
            model.thr_curve_smooth != 0,
            &model.thr_curve,
        );
    }

    // Stage 8, 9, 10: Reverse -> Subtrim & Endpoints (THE ONLY CLAMP) -> Microseconds
    let mut rf_chs = [CHANNEL_CENTER_US; NUM_CHANNELS];
    let rev_mask = model.channel_reverse;
    for i in 0..NUM_CHANNELS {
        let mut v = ch_vals[i];
        if (rev_mask & (1u32 << i)) != 0 {
            v = -v;
        }
        v = (v + compiled.ch_subtrim_q10[i]).clamp(compiled.ch_min_q10[i], compiled.ch_max_q10[i]);
        rf_chs[i] = to_us(v);
    }

    // Stage 11: Motor safety lockout (AFTER reverse & endpoints)
    let is_heli_hold =
        model.model_type() == crate::safety::ModelType::Heli && switches.sa == SwitchPos::Down;
    let is_disarmed = model.arm_switch > 0
        && model.arm_switch <= 44
        && !eval_arm_switch(model.arm_switch, switches);

    if is_heli_hold || is_disarmed || motor_locked {
        let mut mask = model.motor_mask;
        if model.soaring.is_active() {
            let m_ch = (model.soaring.motor_ch as usize).min(NUM_CHANNELS - 1);
            mask |= 1u32 << m_ch;
        }
        for (i, out) in rf_chs.iter_mut().enumerate() {
            if (mask & (1u32 << i)) != 0 {
                *out = compiled.safe_us[i];
            }
        }
    }

    rf_chs
}

/// Convenience wrapper that compiles `model` on the fly (used by tests and non-hot-path callers).
#[allow(clippy::too_many_arguments)]
pub fn compute_channels(
    raw_roll: i16,
    raw_pitch: i16,
    raw_throttle: i16,
    raw_yaw: i16,
    pots: &[i16],
    switches_in: &Switches,
    model: &ModelConfig,
    trims: &TrimController,
    motor_locked: bool,
) -> [u16; NUM_CHANNELS] {
    let compiled = CompiledModel::from_model(model);
    compute_channels_compiled(
        raw_roll,
        raw_pitch,
        raw_throttle,
        raw_yaw,
        pots,
        switches_in,
        model,
        &compiled,
        trims,
        motor_locked,
    )
}
