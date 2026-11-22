//! Curve interpolation engine for throttle, input, and channel curves.
//! Supports 5-point, 7-point, and 9-point curves with linear or Catmull-Rom cubic spline smoothing
//! in the signed ±1024 Q10 fixed-point domain with zero per-frame integer divisions.

/// Convert a 0..=100% curve point into signed Q10 `[-1024, +1024]` without division.
#[inline(always)]
pub const fn pct_to_curve_q10(pct: u8) -> i32 {
    let clamped = if pct > 100 { 100 } else { pct } as i32;
    // 2048 / 100 = 20.48; 20.48 * 65536 = 1_342_177.28
    ((clamped * 1_342_177 + 32_768) >> 16) - 1024
}

/// Evaluate a 5-point, 7-point, or 9-point curve in the signed ±1024 Q10 domain.
/// - `x`: input in `[-1024, +1024]`
/// - `pts_mode`: 5, 7, or 9
/// - `smooth`: false (linear) or true (Catmull-Rom cubic spline)
/// - `curve`: slice of 0..=100% curve points
///
/// Returns output in `[-1024, +1024]` using only shifts and multiplies (zero divisions).
pub fn evaluate_curve_q10(x: i32, pts_mode: u8, smooth: bool, curve: &[u8]) -> i32 {
    let count: usize = match pts_mode {
        9 => 9,
        7 => 7,
        _ => 5,
    };

    let mut pts = [-1024i32; 9];
    for (idx, pt) in pts.iter_mut().enumerate().take(count) {
        if idx < curve.len() {
            *pt = pct_to_curve_q10(curve[idx]);
        }
    }

    let u = (x + 1024).clamp(0, 2048);
    if u <= 0 {
        return pts[0].clamp(-1024, 1024);
    }
    if u >= 2048 {
        return pts[count - 1].clamp(-1024, 1024);
    }

    // Map u ∈ (0, 2048) to segment index `i` and Q8 fraction `t ∈ [0, 256)` without division:
    // - 5pt (4 segs): s = u >> 1 ∈ [0, 1024) = 4 * 256
    // - 7pt (6 segs): s = (u * 3) >> 2 ∈ [0, 1536) = 6 * 256
    // - 9pt (8 segs): s = u ∈ [0, 2048) = 8 * 256
    let s = match count {
        9 => u,
        7 => (u * 3) >> 2,
        _ => u >> 1,
    };
    let max_seg = count - 2;
    let i = ((s >> 8) as usize).min(max_seg);
    let t = s - ((i as i32) << 8);

    if !smooth {
        let y = pts[i] + (((pts[i + 1] - pts[i]) * t + 128) >> 8);
        return y.clamp(-1024, 1024);
    }

    let p0 = if i > 0 {
        pts[i - 1]
    } else {
        2 * pts[0] - pts[1]
    };
    let p1 = pts[i];
    let p2 = pts[i + 1];
    let p3 = if i + 2 < count {
        pts[i + 2]
    } else {
        2 * pts[count - 1] - pts[count - 2]
    };

    catmull_rom_q10(p0, p1, p2, p3, t)
}

#[inline]
fn catmull_rom_q10(p0: i32, p1: i32, p2: i32, p3: i32, t: i32) -> i32 {
    let t2 = (t * t) >> 8;
    let t3 = (t2 * t) >> 8;

    let h00 = 2 * t3 - 3 * t2 + 256;
    let h10 = t3 - 2 * t2 + t;
    let h01 = -2 * t3 + 3 * t2;
    let h11 = t3 - t2;

    let m1 = (p2 - p0) >> 1;
    let m2 = (p3 - p1) >> 1;

    let y = (h00 * p1 + h01 * p2 + h10 * m1 + h11 * m2 + 128) >> 8;
    y.clamp(-1024, 1024)
}

/// Legacy compatibility wrapper evaluating a 5/7/9-point curve on `0..=1000` (used by UI graph preview).
/// Maps `0..=1000` -> `[-1024, +1024]` -> `evaluate_curve_q10` -> `0..=1000`.
pub fn evaluate_curve(input: u16, pts_mode: u8, smooth: bool, curve: &[u8]) -> u16 {
    let u = input.min(1000) as i32;
    let x_q10 = ((u * 2048 + 500) / 1000) - 1024;
    let y_q10 = evaluate_curve_q10(x_q10, pts_mode, smooth, curve);
    (((y_q10 + 1024) * 1000 + 1024) >> 11).clamp(0, 1000) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pct_to_curve_q10_exact_quarters() {
        assert_eq!(pct_to_curve_q10(0), -1024);
        assert_eq!(pct_to_curve_q10(25), -512);
        assert_eq!(pct_to_curve_q10(50), 0);
        assert_eq!(pct_to_curve_q10(75), 512);
        assert_eq!(pct_to_curve_q10(100), 1024);
    }

    #[test]
    fn test_5pt_q10_linear_identity() {
        let curve = [0, 25, 50, 75, 100];
        assert_eq!(evaluate_curve_q10(-1024, 5, false, &curve), -1024);
        assert_eq!(evaluate_curve_q10(-512, 5, false, &curve), -512);
        assert_eq!(evaluate_curve_q10(0, 5, false, &curve), 0);
        assert_eq!(evaluate_curve_q10(512, 5, false, &curve), 512);
        assert_eq!(evaluate_curve_q10(1024, 5, false, &curve), 1024);
        assert_eq!(evaluate_curve_q10(256, 5, false, &curve), 256);
    }

    #[test]
    fn test_7pt_q10_linear_and_spline() {
        let curve = [0, 17, 33, 50, 67, 83, 100];
        assert_eq!(evaluate_curve_q10(-1024, 7, false, &curve), -1024);
        assert_eq!(evaluate_curve_q10(0, 7, false, &curve), 0);
        assert_eq!(evaluate_curve_q10(1024, 7, false, &curve), 1024);
        assert_eq!(evaluate_curve_q10(-1024, 7, true, &curve), -1024);
        assert_eq!(evaluate_curve_q10(0, 7, true, &curve), 0);
        assert_eq!(evaluate_curve_q10(1024, 7, true, &curve), 1024);

        let mut prev = -1024;
        for x in (-1024..=1024).step_by(32) {
            let val = evaluate_curve_q10(x, 7, true, &curve);
            assert!(val >= prev, "7pt spline must be monotonic at {}", x);
            prev = val;
        }
    }

    #[test]
    fn test_9pt_q10_linear_and_spline() {
        let curve = [0, 12, 25, 37, 50, 62, 75, 87, 100];
        assert_eq!(evaluate_curve_q10(-1024, 9, false, &curve), -1024);
        assert_eq!(evaluate_curve_q10(0, 9, false, &curve), 0);
        assert_eq!(evaluate_curve_q10(1024, 9, false, &curve), 1024);
        assert_eq!(evaluate_curve_q10(0, 9, true, &curve), 0);

        let mut prev = -1024;
        for x in (-1024..=1024).step_by(32) {
            let val = evaluate_curve_q10(x, 9, true, &curve);
            assert!(val >= prev, "9pt spline must be monotonic at {}", x);
            prev = val;
        }
    }

    #[test]
    fn test_5pt_linear_identity() {
        let curve = [0, 25, 50, 75, 100, 0, 0, 0, 0];
        assert_eq!(evaluate_curve(0, 5, false, &curve), 0);
        assert_eq!(evaluate_curve(250, 5, false, &curve), 250);
        assert_eq!(evaluate_curve(500, 5, false, &curve), 500);
        assert_eq!(evaluate_curve(750, 5, false, &curve), 750);
        assert_eq!(evaluate_curve(1000, 5, false, &curve), 1000);
        assert_eq!(evaluate_curve(125, 5, false, &curve), 125);
        assert_eq!(evaluate_curve(375, 5, false, &curve), 375);
    }

    #[test]
    fn test_5pt_linear_flat_and_inverted() {
        let flat = [50, 50, 50, 50, 50, 0, 0, 0, 0];
        assert_eq!(evaluate_curve(0, 5, false, &flat), 500);
        assert_eq!(evaluate_curve(500, 5, false, &flat), 500);
        assert_eq!(evaluate_curve(1000, 5, false, &flat), 500);

        let inverted = [100, 75, 50, 25, 0, 0, 0, 0, 0];
        assert_eq!(evaluate_curve(0, 5, false, &inverted), 1000);
        assert_eq!(evaluate_curve(250, 5, false, &inverted), 750);
        assert_eq!(evaluate_curve(500, 5, false, &inverted), 500);
        assert_eq!(evaluate_curve(750, 5, false, &inverted), 250);
        assert_eq!(evaluate_curve(1000, 5, false, &inverted), 0);
    }

    #[test]
    fn test_5pt_spline_endpoints_and_monotonicity() {
        let curve = [0, 25, 50, 75, 100, 0, 0, 0, 0];
        assert_eq!(evaluate_curve(0, 5, true, &curve), 0);
        assert_eq!(evaluate_curve(500, 5, true, &curve), 500);
        assert_eq!(evaluate_curve(1000, 5, true, &curve), 1000);

        let mut prev = 0;
        for x in (50..=1000).step_by(50) {
            let val = evaluate_curve(x, 5, true, &curve);
            assert!(
                val >= prev,
                "Spline must be monotonically non-decreasing at {}",
                x
            );
            prev = val;
        }
    }
}
