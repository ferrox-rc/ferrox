//! FAI F5J scoring calculator and CSV log formatting utilities (zero-heap).

/// Calculate FAI F5J motor launch height penalty in tenths of a point (0.1 pt resolution).
///
/// Rules (FAI SC4 F5J):
/// - `0..=200 m`: 0.5 pts per meter (5 tenths/m)
/// - `> 200 m`: 100.0 pts + 3.0 pts per meter above 200m (30 tenths/m)
#[inline]
pub const fn calculate_launch_penalty_tenths(start_height_m: u16) -> u32 {
    if start_height_m <= 200 {
        (start_height_m as u32) * 5
    } else {
        1000 + ((start_height_m as u32 - 200) * 30)
    }
}

/// Calculate net FAI F5J flight score in tenths of a point (can be negative, clamped to 0 in contests).
#[inline]
pub fn calculate_net_score_tenths(
    flight_sec: u32,
    target_sec: u32,
    start_height_m: u16,
    landing_bonus_pts: u8,
    is_restart: bool,
) -> i32 {
    if is_restart {
        return 0;
    }
    let duration_pts_tenths = (flight_sec.min(target_sec) as i32) * 10;
    let bonus_tenths = (landing_bonus_pts.min(50) as i32) * 10;
    let penalty_tenths = calculate_launch_penalty_tenths(start_height_m) as i32;
    (duration_pts_tenths + bonus_tenths - penalty_tenths).max(0)
}

/// Valid FAI F5J landing bonus options (0 to 50 points in 5-point increments).
pub const LANDING_BONUS_STEPS: [u8; 11] = [0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50];

/// Append a decimal unsigned integer into `buf` at `pos`, returning the updated index.
#[inline]
pub fn push_u32(buf: &mut [u8], mut pos: usize, mut val: u32) -> usize {
    if val == 0 {
        if pos < buf.len() {
            buf[pos] = b'0';
            pos += 1;
        }
        return pos;
    }
    let mut digits = [0u8; 10];
    let mut len = 0;
    while val > 0 && len < digits.len() {
        digits[len] = b'0' + (val % 10) as u8;
        val /= 10;
        len += 1;
    }
    while len > 0 && pos < buf.len() {
        len -= 1;
        buf[pos] = digits[len];
        pos += 1;
    }
    pos
}

/// Append fixed-point tenths (`val_tenths`, e.g. 142 -> "14.2") into `buf` at `pos`.
#[inline]
pub fn push_tenths(buf: &mut [u8], mut pos: usize, val_tenths: i32) -> usize {
    let abs_val = if val_tenths < 0 {
        if pos < buf.len() {
            buf[pos] = b'-';
            pos += 1;
        }
        (-val_tenths) as u32
    } else {
        val_tenths as u32
    };
    pos = push_u32(buf, pos, abs_val / 10);
    if pos < buf.len() {
        buf[pos] = b'.';
        pos += 1;
    }
    if pos < buf.len() {
        buf[pos] = b'0' + (abs_val % 10) as u8;
        pos += 1;
    }
    pos
}

#[inline]
pub fn push_bytes(buf: &mut [u8], mut pos: usize, s: &[u8]) -> usize {
    for &b in s {
        if pos >= buf.len() {
            break;
        }
        buf[pos] = b;
        pos += 1;
    }
    pos
}

/// Format a CSV time-series data row: `time_s,alt_avg_m,alt_min_m\n`.
pub fn format_csv_row(buf: &mut [u8], time_ds: u32, avg_dm: i16, min_dm: i16) -> usize {
    let mut pos = 0;
    pos = push_tenths(buf, pos, time_ds as i32);
    pos = push_bytes(buf, pos, b",");
    pos = push_tenths(buf, pos, avg_dm as i32);
    pos = push_bytes(buf, pos, b",");
    pos = push_tenths(buf, pos, min_dm as i32);
    pos = push_bytes(buf, pos, b"\n");
    pos
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_f5j_penalty_and_net_score() {
        // <= 200m: 0.5 pts/m
        assert_eq!(calculate_launch_penalty_tenths(0), 0);
        assert_eq!(calculate_launch_penalty_tenths(100), 500); // 50.0 pts
        assert_eq!(calculate_launch_penalty_tenths(200), 1000); // 100.0 pts
        // > 200m: 100 + 3.0 pts/m above 200
        assert_eq!(calculate_launch_penalty_tenths(210), 1300); // 130.0 pts

        // Net score: 580s flight (580 pts) + 45 landing - 71.0 penalty (142m) = 554.0 pts
        let net = calculate_net_score_tenths(580, 600, 142, 45, false);
        assert_eq!(net, 5540);

        // Restart invalidates score
        assert_eq!(calculate_net_score_tenths(580, 600, 142, 45, true), 0);
    }

    #[test]
    fn test_format_csv_row() {
        let mut buf = [0u8; 32];
        let len = format_csv_row(&mut buf, 82, 1425, 1390);
        assert_eq!(&buf[..len], b"8.2,142.5,139.0\n");
    }
}
