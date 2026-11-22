//! Fixed-point Q10 (±1024) arithmetic primitives for the Ferrox mixer pipeline.
//!
//! Nominal range is `[-1024, +1024]` (`FX_ONE = 1024`, `FX_SHIFT = 10`).
//! Intermediates are unclamped `i32`, providing > 2,000,000x headroom before overflow.
//! All hot-path operations are division-free (shifts and multiplies only).

/// Fixed-point 1.0 in Q10 (`1 << 10`).
pub const FX_ONE: i32 = 1024;
/// Bit shift for Q10 fixed-point arithmetic.
pub const FX_SHIFT: u32 = 10;
/// Nominal minimum value (-100%).
pub const FX_MIN: i32 = -FX_ONE;
/// Nominal neutral value (0%).
pub const FX_CENTER: i32 = 0;
/// Nominal maximum value (+100%).
pub const FX_MAX: i32 = FX_ONE;

// FlySky / AFHDS standard microsecond pulse range (988..2012 µs)
pub const CHANNEL_MIN_US: u16 = 988;
pub const CHANNEL_CENTER_US: u16 = 1500;
pub const CHANNEL_MAX_US: u16 = 2012;
pub const CHANNEL_SPAN_US: u32 = (CHANNEL_MAX_US - CHANNEL_MIN_US) as u32; // 1024
pub const CHANNEL_HALF_SPAN_US: i32 = (CHANNEL_SPAN_US / 2) as i32; // 512
pub const CHANNEL_REVERSE_SUM: u16 = CHANNEL_MIN_US + CHANNEL_MAX_US; // 3000

/// Multiply two Q10 fixed-point values: `(a * b + 512) >> 10`.
/// Rounds half-up with at most 1/2 LSB bias and no sign branch.
#[inline(always)]
pub const fn mul_q10(a: i32, b: i32) -> i32 {
    (a * b + 512) >> FX_SHIFT
}

/// Convert a percentage (`-125..=+125`, etc.) to Q10 fixed-point.
/// Called when compiling a model (`CompiledModel::from_model`), never in the per-frame hot path.
#[inline(always)]
pub const fn pct_to_q10(pct: i32) -> i32 {
    if pct >= 0 {
        (pct * FX_ONE + 50) / 100
    } else {
        (pct * FX_ONE - 50) / 100
    }
}

/// Convert a normalized stick/pot input in `[-1000, +1000]` to Q10 `[-1024, +1024]` without division.
/// Exact at endpoints: `-1000 -> -1024`, `0 -> 0`, `+1000 -> +1024`.
#[inline(always)]
pub const fn stick_1000_to_q10(val: i16) -> i32 {
    let v = val as i32;
    let scaled = if v >= 0 {
        (v * 1049 + 512) >> 10
    } else {
        -((-v * 1049 + 512) >> 10)
    };
    if scaled > FX_MAX {
        FX_MAX
    } else if scaled < FX_MIN {
        FX_MIN
    } else {
        scaled
    }
}

/// Convert Q10 `[-1024, +1024]` to exact microsecond pulse width `[988, 2012]` with zero division.
/// Guarantees exact reverse symmetry: `to_us(-v) == 3000 - to_us(v)` for all `v ∈ [-1024, 1024]`.
#[inline(always)]
pub const fn to_us(v: i32) -> u16 {
    let clamped = if v > FX_MAX {
        FX_MAX
    } else if v < FX_MIN {
        FX_MIN
    } else {
        v
    };
    // Signed division by power-of-two 2 compiles to 2 single-cycle shift/add instructions on Cortex-M0/M4
    // (no __aeabi_idiv) and preserves exact symmetry: (-v)/2 == -(v/2).
    (CHANNEL_CENTER_US as i32 + (clamped / 2)) as u16
}

/// Convert Q10 `[-1024, +1024]` directly to 11-bit CRSF ticks (`172..=1811`, center `992`).
#[inline(always)]
pub const fn to_crsf(v: i32) -> u16 {
    let clamped = if v > FX_MAX {
        FX_MAX
    } else if v < FX_MIN {
        FX_MIN
    } else {
        v
    };
    let ticks = 992 + ((clamped * 819 + 512) >> FX_SHIFT);
    if ticks < 172 {
        172
    } else if ticks > 1811 {
        1811
    } else {
        ticks as u16
    }
}

/// Apply rate (in Q10) and cubic expo (`-100..=+100`) to a Q10 input `x ∈ [-1024, +1024]`:
/// `output = rate * curve(x)`.
///
/// Guarantees that at full stick (`x = ±1024`), output equals `±rate_q10` regardless of expo.
/// Division-free: `x^3` with `|x| <= 1024` fits in `i32` (`1024^2 = 1048576`, `(x*x >> 10)*x >> 10`).
#[inline]
pub const fn apply_rate_curve(x: i32, rate_q10: i32, expo: i8) -> i32 {
    let clamped_x = if x > FX_MAX {
        FX_MAX
    } else if x < FX_MIN {
        FX_MIN
    } else {
        x
    };

    if expo == 0 {
        return mul_q10(clamped_x, rate_q10);
    }

    let expo_clamped = if expo > 100 {
        100
    } else if expo < -100 {
        -100
    } else {
        expo
    } as i32;

    // Cubic term in Q10: (x * x >> 10) * x >> 10, exact at ±1024 -> ±1024
    let x2 = (clamped_x * clamped_x) >> FX_SHIFT;
    let x3 = (x2 * clamped_x) >> FX_SHIFT;

    let expo_q10 = pct_to_q10(if expo_clamped > 0 {
        expo_clamped
    } else {
        -expo_clamped
    });

    let curved = if expo_clamped > 0 {
        // Soften center: x * (1 - e) + x^3 * e = x + (x^3 - x) * e
        clamped_x + mul_q10(x3 - clamped_x, expo_q10)
    } else {
        // Sharpen center: x + (x - x^3) * |e|
        clamped_x + mul_q10(clamped_x - x3, expo_q10)
    };

    mul_q10(curved, rate_q10)
}

/// Apply differential attenuation (`-100..=+100%`) to a Q10 deflection value.
/// When `diff > 0`, down-deflection (`val < 0`) is attenuated by `(100 - diff)%`.
/// When `diff < 0`, up-deflection (`val > 0`) is attenuated by `(100 + diff)%`.
#[inline(always)]
pub const fn apply_differential_q10(val: i32, diff: i8) -> i32 {
    if diff == 0 {
        return val;
    }
    let d = if diff > 100 {
        100
    } else if diff < -100 {
        -100
    } else {
        diff
    } as i32;

    if d > 0 && val < 0 {
        mul_q10(val, pct_to_q10(100 - d))
    } else if d < 0 && val > 0 {
        mul_q10(val, pct_to_q10(100 + d))
    } else {
        val
    }
}

/// Proportionally saturate a pair `(l, r)` so `max(|l|, |r|) <= FX_ONE` while preserving
/// the steering/control ratio. Only performs a single divide when saturation actually occurs.
#[inline]
pub const fn proportional_saturate_pair(l: i32, r: i32) -> (i32, i32) {
    let abs_l = if l < 0 { -l } else { l };
    let abs_r = if r < 0 { -r } else { r };
    let peak = if abs_l > abs_r { abs_l } else { abs_r };
    if peak <= FX_ONE {
        (l, r)
    } else {
        ((l * FX_ONE) / peak, (r * FX_ONE) / peak)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_us_exact_endpoints_and_center() {
        assert_eq!(to_us(-1024), 988);
        assert_eq!(to_us(0), 1500);
        assert_eq!(to_us(1024), 2012);
        assert_eq!(to_us(-2000), 988);
        assert_eq!(to_us(2000), 2012);
    }

    #[test]
    fn test_to_us_reverse_symmetry_all_values() {
        for v in -1024..=1024 {
            assert_eq!(
                to_us(-v),
                CHANNEL_REVERSE_SUM - to_us(v),
                "Reverse symmetry violated at v={}",
                v
            );
        }
    }

    #[test]
    fn test_to_crsf_endpoints_and_center() {
        assert_eq!(to_crsf(-1024), 173);
        assert_eq!(to_crsf(0), 992);
        assert_eq!(to_crsf(1024), 1811);
    }

    #[test]
    fn test_apply_rate_curve_exact_at_full_stick_for_all_expo() {
        for rate_pct in [30, 50, 75, 100] {
            let rate_q10 = pct_to_q10(rate_pct);
            for expo in (-100..=100).step_by(10) {
                let pos = apply_rate_curve(1024, rate_q10, expo as i8);
                let neg = apply_rate_curve(-1024, rate_q10, expo as i8);
                assert_eq!(
                    pos, rate_q10,
                    "Full positive stick mismatch at rate={}%, expo={}",
                    rate_pct, expo
                );
                assert_eq!(
                    neg, -rate_q10,
                    "Full negative stick mismatch at rate={}%, expo={}",
                    rate_pct, expo
                );
            }
        }
    }

    #[test]
    fn test_apply_rate_curve_expo_direction() {
        let linear = apply_rate_curve(512, FX_ONE, 0);
        let soft = apply_rate_curve(512, FX_ONE, 50);
        let sharp = apply_rate_curve(512, FX_ONE, -50);
        assert_eq!(linear, 512);
        assert!(soft < linear, "Positive expo softens center: {} < {}", soft, linear);
        assert!(sharp > linear, "Negative expo sharpens center: {} > {}", sharp, linear);
    }

    #[test]
    fn test_proportional_saturate_preserves_ratio() {
        assert_eq!(proportional_saturate_pair(600, 400), (600, 400));
        let (l, r) = proportional_saturate_pair(2048, 1024);
        assert_eq!(l, 1024);
        assert_eq!(r, 512);
    }

    #[test]
    fn test_stick_1000_to_q10_endpoints() {
        assert_eq!(stick_1000_to_q10(-1000), -1024);
        assert_eq!(stick_1000_to_q10(0), 0);
        assert_eq!(stick_1000_to_q10(1000), 1024);
    }
}
