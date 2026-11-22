//! Flight Control, Input Conditioning, and 18-Channel ±1024 Fixed-Point Mixing Engine.
//!
//! Pipeline order:
//! `Sources -> Input modifiers -> +Trim -> Predefined mixers -> Channel source select ->
//!  Freeform lines -> Channel modifiers -> Reverse -> Subtrim/Endpoints -> Safety -> Protocol units`

pub mod fx;
pub mod pipeline;
pub mod predefined;
pub mod sources;
pub mod switches;

pub use fx::{
    apply_differential_q10, apply_rate_curve, mul_q10, pct_to_q10, proportional_saturate_pair,
    stick_1000_to_q10, to_crsf, to_us, CHANNEL_CENTER_US, CHANNEL_HALF_SPAN_US, CHANNEL_MAX_US,
    CHANNEL_MIN_US, CHANNEL_REVERSE_SUM, CHANNEL_SPAN_US, FX_CENTER, FX_MAX, FX_MIN, FX_ONE,
    FX_SHIFT,
};
pub use pipeline::{
    compute_channels, compute_channels_compiled, ChanMod, ChannelCfg, CompiledModel, InputMod,
    NUM_CHAN_MODS, NUM_INPUT_MODS,
};
pub use predefined::{
    eval_predefined_mixer, MixerInst, MixerKind, ParamDesc, NUM_PREDEF_MIXERS,
};
pub use sources::{evaluate_source_q10, MixSource, MIXER_OUT_SRC_BASE, MIXER_OUT_SRC_MAX, SRC_THC};
pub use switches::{
    eval_arm_switch, eval_feature_switch, evaluate_logical_switch_raw_q10, is_dr_high,
    is_switch_active, is_timer_active, LogicalSwitchBank,
};

pub const NUM_CHANNELS: usize = 18;

// Normalized stick acquisition bounds (-1000..+1000) used by input/safety layers
pub const MIXER_MIN: i16 = -1000;
pub const MIXER_CENTER: i16 = 0;
pub const MIXER_MAX: i16 = 1000;

/// Pre-flight safe idle channel pulses (failsafe throttle and switches up/disarmed).
pub const SAFE_IDLE_CHANNELS: [u16; NUM_CHANNELS] = [
    CHANNEL_CENTER_US, // CH1 Roll / Aileron
    CHANNEL_CENTER_US, // CH2 Pitch / Elevator
    CHANNEL_MIN_US,    // CH3 Throttle
    CHANNEL_CENTER_US, // CH4 Yaw / Rudder
    CHANNEL_MIN_US,    // CH5 Aux 1 (SA)
    CHANNEL_MIN_US,    // CH6 Aux 2 (SB)
    CHANNEL_CENTER_US, // CH7 Pot VRA
    CHANNEL_CENTER_US, // CH8 Pot VRB
    CHANNEL_MIN_US,    // CH9 Aux 3 (SC)
    CHANNEL_MIN_US,    // CH10 Aux 4 (SD)
    CHANNEL_CENTER_US, // CH11 Extra 1
    CHANNEL_CENTER_US, // CH12 Extra 2
    CHANNEL_CENTER_US, // CH13 Extra 3
    CHANNEL_CENTER_US, // CH14 Extra 4
    CHANNEL_CENTER_US, // CH15 Extra 5
    CHANNEL_CENTER_US, // CH16 Extra 6
    CHANNEL_CENTER_US, // CH17 Extra 7
    CHANNEL_CENTER_US, // CH18 Extra 8
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{SwitchPos, Switches};
    use crate::storage::{LogicalSwitch, MixLine, ModelConfig};
    use crate::trim::TrimController;

    #[test]
    fn test_compute_channels_normal_centered() {
        let model = ModelConfig::default_for_index(0);
        let trims = TrimController::new();
        let switches = Switches::default();
        let chs = compute_channels(0, 0, -1000, 0, &[0, 0], &switches, &model, &trims, false);

        assert_eq!(chs[0], 1500);
        assert_eq!(chs[1], 1500);
        assert_eq!(chs[2], 988);
        assert_eq!(chs[3], 1500);
    }

    #[test]
    fn test_safety_after_reverse_unidirectional_and_bidir() {
        let mut model = ModelConfig::default_for_index(0);
        model.arm_switch = 2; // SA Down to arm
        model.channel_reverse = 1 << 2; // Reverse CH3 (Throttle)
        let trims = TrimController::new();
        let mut switches = Switches::default();
        switches.sa = SwitchPos::Up; // Disarmed!

        // Even with CH3 reversed and full throttle stick, disarmed MUST output physical idle 988 µs!
        let disarmed = compute_channels(0, 0, 1000, 0, &[0, 0], &switches, &model, &trims, false);
        assert_eq!(
            disarmed[2], CHANNEL_MIN_US,
            "Reversed throttle must output 988 µs when disarmed"
        );

        // Armed + reversed full throttle -> reversed full throw
        switches.sa = SwitchPos::Down;
        let armed = compute_channels(0, 0, 1000, 0, &[0, 0], &switches, &model, &trims, false);
        assert_eq!(armed[2], CHANNEL_MIN_US);
        let armed_low = compute_channels(0, 0, -1000, 0, &[0, 0], &switches, &model, &trims, false);
        assert_eq!(armed_low[2], CHANNEL_MAX_US);

        // Bidirectional motor (Tank / Boat) disarmed -> neutral 1500 µs regardless of reverse
        model.motor_mask = (1 << 0) | (1 << 1);
        model.bidir_mask = (1 << 0) | (1 << 1);
        model.channel_reverse = 1 << 0;
        switches.sa = SwitchPos::Up;
        let bidir_disarmed =
            compute_channels(1000, 1000, 0, 0, &[0, 0], &switches, &model, &trims, false);
        assert_eq!(bidir_disarmed[0], CHANNEL_CENTER_US);
        assert_eq!(bidir_disarmed[1], CHANNEL_CENTER_US);

        // Competition motor_locked (F5J single-run rule) forces motor off even when armed
        switches.sa = SwitchPos::Down;
        let locked = compute_channels(1000, 1000, 1000, 0, &[0, 0], &switches, &model, &trims, true);
        assert_eq!(locked[0], CHANNEL_CENTER_US);
        assert_eq!(locked[1], CHANNEL_CENTER_US);
    }

    #[test]
    fn test_no_intermediate_clamp_on_mix_lines() {
        let mut model = ModelConfig::default_for_index(0);
        // CH5 starts at 0; +80% MAX (+819) +80% MAX (+819) -80% MAX (-819) => +819 (~1910 µs)
        model.channels[4].source = 0;
        model.mixes[0] = MixLine {
            target_ch: 5,
            source: 11, // MAX (+1024)
            weight_src: 0,
            weight: 80,
            offset: 0,
            switch: 0,
            mode: 0,
            _reserved: 0,
        };
        model.mixes[1] = model.mixes[0];
        model.mixes[2] = MixLine {
            weight: -80,
            ..model.mixes[0]
        };

        let trims = TrimController::new();
        let switches = Switches::default();
        let chs = compute_channels(0, 0, -1000, 0, &[0, 0], &switches, &model, &trims, false);
        // Without intermediate clamping, result is +819 Q10 => 1500 + 409 = 1909 µs
        // (If it had clamped after line 2 at +1024, line 3 would have left 1024 - 819 = +205 => 1602 µs!)
        assert_eq!(chs[4], to_us(pct_to_q10(80)));
    }

    #[test]
    fn test_predefined_tank_mixer_integration() {
        let mut model = ModelConfig::default_for_index(0);
        model.mixers[0] = MixerInst::default_for_kind(MixerKind::Tank); // in_a=3 (Thr), in_b=1 (Roll)
        model.channels[0].source = MIXER_OUT_SRC_BASE; // Left track = Mixer 0 Out 0
        model.channels[1].source = MIXER_OUT_SRC_BASE + 1; // Right track = Mixer 0 Out 1
        model.motor_mask = (1 << 0) | (1 << 1);
        model.bidir_mask = (1 << 0) | (1 << 1);

        let trims = TrimController::new();
        let switches = Switches::default();
        // Forward 50% (+500) + Right Steer 25% (+250) -> Left ~+768, Right ~+256
        let chs = compute_channels(250, 0, 500, 0, &[0, 0], &switches, &model, &trims, false);
        assert!(chs[0] > chs[1], "Left track {} must exceed right track {}", chs[0], chs[1]);
        assert!(chs[0] > 1850 && chs[1] > 1600);
    }

    #[test]
    fn test_overflow_fuzz_32_lines_no_panic() {
        let mut model = ModelConfig::default_for_index(0);
        for line in model.mixes.iter_mut() {
            *line = MixLine {
                target_ch: 1,
                source: 11, // MAX
                weight_src: 0,
                weight: 100,
                offset: 100,
                switch: 0,
                mode: 0,
                _reserved: 0,
            };
        }
        let trims = TrimController::new();
        let switches = Switches::default();
        let chs = compute_channels(1000, 1000, 1000, 1000, &[1000; 7], &switches, &model, &trims, false);
        assert_eq!(chs[0], CHANNEL_MAX_US);
    }

    #[test]
    fn test_logical_switches_instantaneous_chaining() {
        let mut model = ModelConfig::default_for_index(0);
        model.logical_switches[0] = LogicalSwitch {
            func: 1, // a > x
            v1: 1,   // Roll
            v2: 50,  // > 50%
            and_sw: 0,
            delay_tenths: 0,
            duration_tenths: 0,
        };
        model.channels[4].source = 50; // L1
        let trims = TrimController::new();
        let switches = Switches::default();
        let low = compute_channels(0, 0, -1000, 0, &[0, 0], &switches, &model, &trims, false);
        let high = compute_channels(800, 0, -1000, 0, &[0, 0], &switches, &model, &trims, false);
        assert_eq!(low[4], CHANNEL_MIN_US);
        assert_eq!(high[4], CHANNEL_MAX_US);
    }
}
