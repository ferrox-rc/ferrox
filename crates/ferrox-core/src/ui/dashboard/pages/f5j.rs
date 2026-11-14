//! F5J Competition & 15-Minute Graphic Altitude Telemetry Dashboard Page.
//!
//! Renders cleanly below the status bar (`y = 12..63`) across 128x64 and 212x64 displays:
//! - Row 1 (`y = 12`): `F5J:<m> MAX:<m> MIN:<m>` + right-aligned phase/timer badge
//! - Graph Area (`y = 19..56`): Auto-scaling Y-axis, 110-sample connected trace, 5m/10m/15m ticks,
//!   dotted vertical guides + 3x3 target boxes for MAX thermal peak and MIN real-flight valley.

use crate::board::{ActiveBoard, BoardProfile};
use crate::display::St7567;
use crate::rf::afhds2a::TelemetryData;
use crate::soaring::f5j::graph::MAX_SAMPLES;
use crate::soaring::f5j::{F5jEngine, F5jPhase};
use crate::storage::RadioStorage;
use crate::ui::format::{ascii_as_str, format_vbat};

fn push_u32_ascii(buf: &mut [u8], mut pos: usize, mut val: u32) -> usize {
    if val == 0 {
        if pos < buf.len() {
            buf[pos] = b'0';
            pos += 1;
        }
        return pos;
    }
    let mut tmp = [0u8; 10];
    let mut len = 0;
    while val > 0 && len < tmp.len() {
        tmp[len] = b'0' + (val % 10) as u8;
        val /= 10;
        len += 1;
    }
    while len > 0 && pos < buf.len() {
        len -= 1;
        buf[pos] = tmp[len];
        pos += 1;
    }
    pos
}

fn push_str_ascii(buf: &mut [u8], mut pos: usize, s: &[u8]) -> usize {
    for &b in s {
        if pos >= buf.len() {
            break;
        }
        buf[pos] = b;
        pos += 1;
    }
    pos
}

#[inline(never)]
pub fn render(
    lcd: &mut St7567,
    storage: &RadioStorage,
    telem: &TelemetryData,
    f5j: &F5jEngine,
    now_ms: u32,
) {
    let w = ActiveBoard::DISPLAY_WIDTH as i32;
    let x_axis_left: i32 = 15;
    let graph_x_start: i32 = 16;
    let x_axis_right: i32 = (w - 3).max(graph_x_start + MAX_SAMPLES as i32 - 1);
    let plot_w: i32 = x_axis_right - graph_x_start;

    let y_top: i32 = 19;
    let y_bottom: i32 = 56;
    let y_axis_base: i32 = 57;
    let chart_h: i32 = y_bottom - y_top; // 37 px

    // --- 1. Row 1 (y = 12): F5J/F3K Start, MAX, MIN Altitudes ---
    let mut hdr_buf = [b' '; 32];
    let mut p = 0;
    let prefix: &[u8] = if storage.active_model().soaring.is_f3k() {
        b"F3K:"
    } else {
        b"F5J:"
    };
    p = push_str_ascii(&mut hdr_buf, p, prefix);
    p = push_u32_ascii(&mut hdr_buf, p, f5j.display_f5j_m() as u32);
    if f5j.is_restart {
        p = push_str_ascii(&mut hdr_buf, p, b"!");
    }
    p = push_str_ascii(&mut hdr_buf, p, b" M:");
    p = push_u32_ascii(&mut hdr_buf, p, (f5j.graph.max_val_dm.max(0) / 10) as u32);
    p = push_str_ascii(&mut hdr_buf, p, b" m:");
    if f5j.graph.min_val_idx.is_some() {
        p = push_u32_ascii(&mut hdr_buf, p, (f5j.graph.min_val_dm.max(0) / 10) as u32);
    } else {
        p = push_str_ascii(&mut hdr_buf, p, b"--");
    }
    lcd.draw_str_4x6(1, 12, ascii_as_str(&hdr_buf[..p]), false);

    // --- 2. Right-aligned Phase & Flight Duration Badge (y = 11..17) ---
    let mut t_buf = [b' '; 12];
    let mut tp = 0;
    let elapsed_sec = f5j.flight_elapsed_sec(now_ms);
    let disp_secs = match f5j.phase {
        F5jPhase::Idle => (storage.active_model().soaring.target_time_mins.max(1) as u32) * 60,
        _ => elapsed_sec,
    };
    let mins = disp_secs / 60;
    let secs = disp_secs % 60;
    tp = push_str_ascii(&mut t_buf, tp, f5j.phase.as_str().as_bytes());
    tp = push_str_ascii(&mut t_buf, tp, b" ");
    tp = push_u32_ascii(&mut t_buf, tp, mins);
    tp = push_str_ascii(&mut t_buf, tp, b":");
    if secs < 10 {
        tp = push_str_ascii(&mut t_buf, tp, b"0");
    }
    tp = push_u32_ascii(&mut t_buf, tp, secs);
    let t_str = ascii_as_str(&t_buf[..tp]);
    let badge_w = (tp as i32) * 4 + 2;
    let badge_x = (w - badge_w - 1).max(0);
    let invert_badge = f5j.phase == F5jPhase::Motor || f5j.phase == F5jPhase::Window || f5j.landed;
    if invert_badge {
        lcd.fill_rect(badge_x, 11, badge_w as u32, 7, true);
        lcd.draw_str_4x6(badge_x + 1, 12, t_str, true);
    } else {
        lcd.draw_str_4x6(badge_x + 1, 12, t_str, false);
    }

    // --- 3. Auto-Scaling Y-Axis Labels ---
    let display_max_m = ((f5j.graph.graph_max_dm.max(10) + 9) / 10) as u32;
    let max_dm = (display_max_m as i32 * 10).max(10);
    let mut ymax_buf = [0u8; 8];
    let ymax_len = push_u32_ascii(&mut ymax_buf, 0, display_max_m);
    lcd.draw_str_4x6(1, y_top, ascii_as_str(&ymax_buf[..ymax_len]), false);
    lcd.draw_str_4x6(5, 51, "0", false);

    // --- 4. Coordinate Axes & Timeline Grid (5m, 10m, 15m) ---
    lcd.draw_vline(x_axis_left, y_top, (y_axis_base - y_top + 1) as u32, true);
    lcd.draw_hline(
        x_axis_left,
        y_axis_base,
        (x_axis_right - x_axis_left + 1) as u32,
        true,
    );

    let x_5m = graph_x_start + (plot_w / 3);
    let x_10m = graph_x_start + (plot_w * 2 / 3);
    let x_15m = x_axis_right;

    // 5m tick + label
    lcd.draw_vline(x_5m, y_axis_base, 2, true);
    lcd.draw_str_4x6(x_5m - 3, 59, "5m", false);

    // 10m dotted vertical guide + tick + label
    let mut gy = y_top + 1;
    while gy <= y_bottom {
        lcd.set_pixel(x_10m, gy, true);
        if gy < y_bottom {
            lcd.set_pixel(x_10m, gy + 1, true);
        }
        gy += 4;
    }
    lcd.draw_vline(x_10m, y_axis_base, 2, true);
    lcd.draw_str_4x6(x_10m - 5, 59, "10m", false);

    // 15m terminal tick + label
    lcd.draw_vline(x_15m, y_axis_base, 2, true);
    lcd.draw_str_4x6(x_15m - 12, 59, "15m", false);

    // --- 5. Post-Flight Net Score + Landing Bonus OR Live RX Voltage Overlay ---
    if f5j.landed {
        let target_sec = (storage.active_model().soaring.target_time_mins.max(1) as u32) * 60;
        let score_tenths = crate::soaring::f5j::score::calculate_net_score_tenths(
            f5j.final_flight_sec,
            target_sec,
            f5j.f5j_start_height_m,
            f5j.landing_bonus_pts,
            f5j.is_restart,
        );
        let mut s_buf = [b' '; 20];
        let mut sp = push_str_ascii(&mut s_buf, 0, b"B:");
        sp = push_u32_ascii(&mut s_buf, sp, f5j.landing_bonus_pts as u32);
        sp = push_str_ascii(&mut s_buf, sp, b" S:");
        sp = crate::soaring::f5j::score::push_tenths(&mut s_buf, sp, score_tenths);
        lcd.draw_str_4x6(
            w - (sp as i32 * 4) - 2,
            y_top,
            ascii_as_str(&s_buf[..sp]),
            false,
        );
    } else if telem.rx_voltage_mv > 0 {
        let mut rx_buf = [0u8; 6];
        let rx_str = format_vbat(telem.rx_voltage_mv, &mut rx_buf);
        let mut lbl = [b' '; 8];
        let mut lp = push_str_ascii(&mut lbl, 0, b"R:");
        lp = push_str_ascii(&mut lbl, lp, rx_str.as_bytes());
        lcd.draw_str_4x6(
            w - (lp as i32 * 4) - 2,
            y_top,
            ascii_as_str(&lbl[..lp]),
            false,
        );
    }

    // --- 6. Helper to map sample index & decimeter alt to pixel coordinates ---
    let map_x = |idx: usize| -> i32 {
        if MAX_SAMPLES <= 1 {
            graph_x_start
        } else {
            graph_x_start + ((idx as i32 * plot_w) / (MAX_SAMPLES as i32 - 1))
        }
    };
    let map_y = |alt_dm: i16| -> i32 {
        let clamped_dm = (alt_dm as i32).max(0);
        let y = y_bottom - ((clamped_dm * chart_h) / max_dm);
        y.clamp(y_top, y_bottom)
    };

    // --- 7. Plot Connected 110-Sample Altitude Curve ---
    let count = f5j.graph.sample_count.min(MAX_SAMPLES);
    if count >= 1 {
        for i in 0..count.saturating_sub(1) {
            let x1 = map_x(i);
            let y1 = map_y(f5j.graph.avg_samples_dm[i]);
            let x2 = map_x(i + 1);
            let y2 = map_y(f5j.graph.avg_samples_dm[i + 1]);
            lcd.draw_line(x1, y1, x2, y2, true);
        }

        // Emphasize current leading point
        let cur_idx = count - 1;
        let cx = map_x(cur_idx);
        let cy = map_y(f5j.graph.avg_samples_dm[cur_idx]);
        lcd.set_pixel(cx, (cy - 1).max(y_top), true);
        lcd.set_pixel(cx, (cy + 1).min(y_bottom), true);
    }

    // --- 8. Visual Marker for MAX Thermal Peak ---
    if let Some(max_idx) = f5j.graph.max_val_idx {
        if max_idx < count && f5j.graph.max_val_dm > 0 {
            let x_max = map_x(max_idx);
            let y_max = map_y(f5j.graph.avg_samples_dm[max_idx]);

            let mut my = y_top;
            while my <= y_bottom {
                lcd.set_pixel(x_max, my, true);
                my += 3;
            }
            lcd.draw_rect(x_max - 1, y_max - 1, 3, 3, true);
            lcd.draw_hline(x_max - 1, y_top, 3, true);
        }
    }

    // --- 9. Visual Marker for MIN Real-Flight Altitude (>90s) ---
    if let Some(min_idx) = f5j.graph.min_val_idx {
        if min_idx < count && f5j.graph.min_val_dm < i16::MAX {
            let x_min = map_x(min_idx);
            let y_min = map_y(f5j.graph.min_samples_dm[min_idx]);

            let mut my = y_top;
            while my <= y_bottom {
                lcd.set_pixel(x_min, my, true);
                my += 3;
            }
            lcd.draw_rect(x_min - 1, y_min - 1, 3, 3, true);
            lcd.draw_hline(x_min - 1, y_axis_base, 3, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buzzer::Buzzer;
    use crate::mixer::NUM_CHANNELS;
    use crate::soaring::SoaringConfig;

    #[test]
    fn test_f5j_dashboard_pixel_rendering() {
        let mut lcd = St7567::new();
        let storage = RadioStorage::default_factory();
        let mut f5j = F5jEngine::new();
        let mut buzzer = Buzzer::new();
        let cfg = SoaringConfig::default_f5j();
        let mut chs = [988u16; NUM_CHANNELS];

        // Launch and populate 25 samples (~204s flight)
        let mut telem = TelemetryData::default();
        telem.connected = true;
        telem.rx_voltage_mv = 5050;
        chs[2] = 2000;
        f5j.update(0, &chs, &telem, &cfg, &mut buzzer);
        chs[2] = 988;

        for i in 1..=25 {
            let t = i * crate::soaring::f5j::graph::SAMPLE_INTERVAL_MS;
            telem.alt_dm = ((i % 12) as i16 + 1) * 150;
            f5j.update(t, &chs, &telem, &cfg, &mut buzzer);
        }

        lcd.clear_buffer();
        render(&mut lcd, &storage, &telem, &f5j, 210_000);

        // Verify framebuffer has active pixels rendered (axes, curve, labels) without panics
        let lit = lcd
            .framebuffer()
            .iter()
            .map(|b| b.count_ones())
            .sum::<u32>();
        assert!(lit > 100);
    }
}
