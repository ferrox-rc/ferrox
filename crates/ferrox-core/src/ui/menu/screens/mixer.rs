//! Dual Rate / Expo, Throttle Curve, Wing Templates, and Mixer Line Editor screens.

use crate::buzzer::Buzzer;
use crate::curve;
use crate::display::St7567;
use crate::menu::format::{
    ascii_as_str, i8_to_dec, u8_to_dec, AXIS_NAMES, DR_SWITCH_NAMES, MODE_NAMES, SOURCE_NAMES,
    SWITCH_COND_NAMES, TEMPLATE_NAMES, WEIGHT_SRC_NAMES,
};
use crate::menu::widgets;
use crate::menu::{MenuController, MenuState, NavKeys};
use crate::storage::{self, RadioStorage};

#[inline(never)]
pub fn update_dual_rate(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    storage: &mut RadioStorage,
    buzzer: &mut Buzzer,
) {
    const FIELD_COUNT: usize = 6;

    if keys.cancel {
        if ctrl.editing {
            ctrl.editing = false;
        } else {
            storage::save_active_model(storage);
            ctrl.return_to_main_menu();
            buzzer.click();
            return;
        }
    }

    let axis = ctrl.sub_idx.min(2); // 0: Roll, 1: Pitch, 2: Yaw

    if !ctrl.editing {
        widgets::navigate_4slot_list(
            &mut ctrl.selected_item,
            &mut ctrl.scroll_offset,
            FIELD_COUNT,
            keys.up,
            keys.down,
            buzzer,
        );

        if keys.ok {
            ctrl.editing = true;
            buzzer.click();
        }
    } else {
        // Editing value
        match ctrl.selected_item {
            0 => {
                // Switch (0..6)
                if let Some(dr) = keys.dr_change {
                    if storage.is_dr_switch_available(dr) {
                        storage.active_model.dr_switch = dr;
                        buzzer.play_tone(2400, 40);
                    }
                } else if keys.up {
                    storage.active_model.dr_switch =
                        storage.next_available_dr_switch(storage.active_model.dr_switch);
                    buzzer.play_tone(2200, 20);
                } else if keys.down {
                    storage.active_model.dr_switch =
                        storage.prev_available_dr_switch(storage.active_model.dr_switch);
                    buzzer.play_tone(2200, 20);
                }
            }
            1 => {
                // Axis (0..2)
                if keys.up || keys.down {
                    ctrl.sub_idx = (ctrl.sub_idx + 1) % 3;
                    buzzer.play_tone(2200, 20);
                }
            }
            2 => {
                adjust_clamped_u8(
                    &mut storage.active_model.dr_high[axis],
                    50,
                    100,
                    5,
                    keys.up,
                    keys.down,
                    buzzer,
                );
            }
            3 => {
                adjust_clamped_u8(
                    &mut storage.active_model.dr_low[axis],
                    30,
                    100,
                    5,
                    keys.up,
                    keys.down,
                    buzzer,
                );
            }
            4 => {
                adjust_clamped_i8(
                    &mut storage.active_model.expo_high[axis],
                    -100,
                    100,
                    5,
                    keys.up,
                    keys.down,
                    buzzer,
                );
            }
            5 => {
                adjust_clamped_i8(
                    &mut storage.active_model.expo_low[axis],
                    -100,
                    100,
                    5,
                    keys.up,
                    keys.down,
                    buzzer,
                );
            }
            _ => {}
        }

        if keys.ok {
            ctrl.editing = false;
            buzzer.click();
        }
    }

    // Render Header
    widgets::draw_header(lcd, "DUAL RATE / EXPO");

    for slot in 0..4 {
        let idx = ctrl.scroll_offset + slot;
        if idx >= FIELD_COUNT {
            break;
        }
        render_dr_expo_row(lcd, slot, idx, idx == ctrl.selected_item, axis, storage);
    }

    let footer = if ctrl.editing {
        "[OK] Done   [UP/DN] Value"
    } else {
        "[OK] Edit   [ESC] Exit"
    };
    widgets::draw_footer(lcd, footer);
}

#[inline(never)]
fn adjust_clamped_i8(
    val: &mut i8,
    min: i8,
    max: i8,
    step: i8,
    up: bool,
    down: bool,
    buzzer: &mut Buzzer,
) {
    if up && *val <= max - step {
        *val += step;
        buzzer.play_tone(2200, 20);
    } else if down && *val >= min + step {
        *val -= step;
        buzzer.play_tone(2200, 20);
    }
}

#[inline(never)]
fn adjust_clamped_u8(
    val: &mut u8,
    min: u8,
    max: u8,
    step: u8,
    up: bool,
    down: bool,
    buzzer: &mut Buzzer,
) {
    if up && *val <= max - step {
        *val += step;
        buzzer.play_tone(2200, 20);
    } else if down && *val >= min + step {
        *val -= step;
        buzzer.play_tone(2200, 20);
    }
}

#[inline(never)]
fn format_ch_str(ch: u8, buf: &mut [u8; 4]) -> &str {
    buf[0] = b'C';
    buf[1] = b'H';
    if ch >= 10 {
        buf[2] = b'0' + (ch / 10);
        buf[3] = b'0' + (ch % 10);
    } else {
        buf[2] = b'0' + ch;
        buf[3] = b' ';
    }
    ascii_as_str(buf)
}

#[inline(never)]
fn render_dr_expo_row(
    lcd: &mut St7567,
    slot: usize,
    idx: usize,
    is_sel: bool,
    axis: usize,
    storage: &RadioStorage,
) {
    let mut b5 = [0u8; 5];
    let mut b6 = [0u8; 6];
    match idx {
        0 => {
            let sw_name = DR_SWITCH_NAMES[(storage.active_model.dr_switch as usize).min(6)];
            widgets::draw_list_row(lcd, slot, is_sel, "Switch:", Some(sw_name), 60);
        }
        1 => {
            widgets::draw_list_row(lcd, slot, is_sel, "Channel:", Some(AXIS_NAMES[axis]), 60);
        }
        2 => {
            let str_val = u8_to_dec(storage.active_model.dr_high[axis], &mut b5);
            widgets::draw_list_row(lcd, slot, is_sel, "Hi Rate:", Some(str_val), 60);
        }
        3 => {
            let str_val = u8_to_dec(storage.active_model.dr_low[axis], &mut b5);
            widgets::draw_list_row(lcd, slot, is_sel, "Lo Rate:", Some(str_val), 60);
        }
        4 => {
            let str_val = i8_to_dec(storage.active_model.expo_high[axis], &mut b6);
            widgets::draw_list_row(lcd, slot, is_sel, "Hi Expo:", Some(str_val), 60);
        }
        5 => {
            let str_val = i8_to_dec(storage.active_model.expo_low[axis], &mut b6);
            widgets::draw_list_row(lcd, slot, is_sel, "Lo Expo:", Some(str_val), 60);
        }
        _ => {}
    }
}

#[inline(never)]
pub fn update_throttle_curve(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    storage: &mut RadioStorage,
    buzzer: &mut Buzzer,
) {
    let pts_count = match storage.active_model.thr_curve_pts {
        9 => 9,
        7 => 7,
        _ => 5,
    };
    let max_items = 2 + pts_count; // Item 0: Pts mode, Item 1: Smooth, Item 2..(2+pts_count-1): Points

    if ctrl.editing {
        let pt_idx = ctrl.selected_item.saturating_sub(2).min(pts_count - 1);

        if keys.cancel {
            ctrl.editing = false;
            storage::save_active_model(storage);
            buzzer.click();
            return;
        }

        if keys.up {
            if storage.active_model.thr_curve[pt_idx] < 100 {
                storage.active_model.thr_curve[pt_idx] += 1;
            }
            buzzer.play_tone(2200, 20);
        }

        if keys.down {
            if storage.active_model.thr_curve[pt_idx] > 0 {
                storage.active_model.thr_curve[pt_idx] -= 1;
            }
            buzzer.play_tone(2200, 20);
        }

        if keys.ok {
            if ctrl.selected_item + 1 < max_items {
                ctrl.selected_item += 1;
            } else {
                ctrl.editing = false;
            }
            storage::save_active_model(storage);
            buzzer.click();
        } else if keys.bind {
            if ctrl.selected_item + 1 < max_items {
                ctrl.selected_item += 1;
            } else {
                ctrl.selected_item = 2; // Wrap back to P1
            }
            storage::save_active_model(storage);
            buzzer.click();
        }
    } else {
        if keys.cancel {
            storage::save_active_model(storage);
            ctrl.return_to_main_menu();
            buzzer.click();
            return;
        }

        if keys.down {
            ctrl.selected_item = (ctrl.selected_item + 1) % max_items;
            buzzer.play_tone(2200, 20);
        }

        if keys.up {
            ctrl.selected_item = if ctrl.selected_item == 0 {
                max_items - 1
            } else {
                ctrl.selected_item - 1
            };
            buzzer.play_tone(2200, 20);
        }

        if keys.bind {
            if ctrl.selected_item < 2 {
                ctrl.selected_item = 2;
            } else {
                ctrl.selected_item = (ctrl.selected_item + 1) % max_items;
                if ctrl.selected_item < 2 {
                    ctrl.selected_item = 2;
                }
            }
            buzzer.click();
        }

        if keys.ok {
            if ctrl.selected_item == 0 {
                let cur_pts = storage.active_model.thr_curve_pts;
                if cur_pts == 5 {
                    // Transition 5 -> 7
                    // Resample the 5-point curve at 7 evenly spaced fractional indices (0/6, 1/6, ..., 6/6)
                    let c = storage.active_model.thr_curve;
                    let mut new_crv = [0u8; 17];
                    for (i, pt) in new_crv.iter_mut().enumerate().take(7) {
                        let in_pct = (i as u16 * 1000) / 6;
                        *pt =
                            ((curve::evaluate_curve(in_pct, 5, false, &c) + 5) / 10).min(100) as u8;
                    }
                    storage.active_model.thr_curve = new_crv;
                    storage.active_model.thr_curve_pts = 7;
                } else if cur_pts == 7 {
                    // Transition 7 -> 9
                    // Resample the 7-point curve at 9 evenly spaced fractional indices (0/8, 1/8, ..., 8/8)
                    let c = storage.active_model.thr_curve;
                    let mut new_crv = [0u8; 17];
                    for (i, pt) in new_crv.iter_mut().enumerate().take(9) {
                        let in_pct = (i as u16 * 1000) / 8;
                        *pt =
                            ((curve::evaluate_curve(in_pct, 7, false, &c) + 5) / 10).min(100) as u8;
                    }
                    storage.active_model.thr_curve = new_crv;
                    storage.active_model.thr_curve_pts = 9;
                } else {
                    // Transition 9 -> 5 (sample evenly: indices 0, 2, 4, 6, 8)
                    let c = storage.active_model.thr_curve;
                    let mut new_crv = [0u8; 17];
                    new_crv[0] = c[0];
                    new_crv[1] = c[2];
                    new_crv[2] = c[4];
                    new_crv[3] = c[6];
                    new_crv[4] = c[8];
                    storage.active_model.thr_curve = new_crv;
                    storage.active_model.thr_curve_pts = 5;
                }
                storage::save_active_model(storage);
                buzzer.click();
            } else if ctrl.selected_item == 1 {
                storage.active_model.thr_curve_smooth =
                    if storage.active_model.thr_curve_smooth == 0 {
                        1
                    } else {
                        0
                    };
                storage::save_active_model(storage);
                buzzer.click();
            } else {
                ctrl.editing = true;
                buzzer.click();
            }
        }
    }

    widgets::draw_header(lcd, "THROTTLE CURVE");

    // Left side: Mode & active point info
    let mode_str = match storage.active_model.thr_curve_pts {
        9 => "9-PT",
        7 => "7-PT",
        _ => "5-PT",
    };
    let is_sel_pts = !ctrl.editing && ctrl.selected_item == 0;
    if is_sel_pts {
        lcd.fill_rect(2, 13, 70, 9, true);
    }
    lcd.draw_str_6x10(4, 13, "Pts:", is_sel_pts);
    lcd.draw_str_6x10(32, 13, mode_str, is_sel_pts);

    let smooth_str = if storage.active_model.thr_curve_smooth != 0 {
        "SMOOTH"
    } else {
        "LINEAR"
    };
    let is_sel_crv = !ctrl.editing && ctrl.selected_item == 1;
    if is_sel_crv {
        lcd.fill_rect(2, 23, 70, 9, true);
    }
    lcd.draw_str_6x10(4, 23, "Crv:", is_sel_crv);
    lcd.draw_str_6x10(32, 23, smooth_str, is_sel_crv);

    if ctrl.selected_item >= 2 {
        let pt_idx = ctrl.selected_item - 2;
        let val = storage.active_model.thr_curve[pt_idx];
        let mut p_buf = *b"P00: 000%";
        let p_num = (pt_idx + 1) as u8;
        p_buf[1] = b'0' + (p_num / 10);
        p_buf[2] = b'0' + (p_num % 10);
        if val >= 100 {
            p_buf[5] = b'1';
            p_buf[6] = b'0';
            p_buf[7] = b'0';
        } else if val >= 10 {
            p_buf[5] = b' ';
            p_buf[6] = b'0' + (val / 10);
            p_buf[7] = b'0' + (val % 10);
        } else {
            p_buf[5] = b' ';
            p_buf[6] = b' ';
            p_buf[7] = b'0' + val;
        }
        let p_str = ascii_as_str(&p_buf[..9]);

        if ctrl.editing {
            lcd.fill_rect(2, 35, 70, 11, true);
            lcd.draw_str_6x10(4, 37, p_str, true);
        } else {
            lcd.draw_rect(2, 35, 70, 11, true);
            lcd.draw_str_6x10(4, 37, p_str, false);
        }
    } else {
        let pts_range_str = match pts_count {
            9 => "Pts: 1..9",
            7 => "Pts: 1..7",
            _ => "Pts: 1..5",
        };
        lcd.draw_str_6x10(4, 37, pts_range_str, false);
    }

    // Right side: Graph box (x = 76..124, y = 13..49)
    lcd.draw_rect(76, 13, 49, 37, true);

    // Draw curve graph inside box (width 47, height 35)
    let pts_mode = storage.active_model.thr_curve_pts;
    let is_smooth = storage.active_model.thr_curve_smooth != 0;
    let curve_data = storage.active_model.thr_curve;

    for px in 0..47 {
        let input_pct = ((px as u32 * 1000) / 46) as u16;
        let out_pct = curve::evaluate_curve(input_pct, pts_mode, is_smooth, &curve_data);
        let py = 48 - ((out_pct as i32 * 34) / 1000);
        lcd.set_pixel(77 + px, py, true);
    }

    // Draw point indicator dot on graph for selected point (3x3 dot)
    if ctrl.selected_item >= 2 {
        let pt_idx = ctrl.selected_item - 2;
        let input_pct = (pt_idx as u32 * 1000) / (pts_count as u32 - 1);
        let val = storage.active_model.thr_curve[pt_idx];
        let dot_x = 77 + ((input_pct * 46) / 1000) as i32;
        let dot_y = 48 - (((val as i32 * 10) * 34) / 1000);

        lcd.fill_rect(dot_x - 1, dot_y - 1, 3, 3, true);
    }

    if ctrl.editing {
        widgets::draw_footer(lcd, "[OK] Next Pt   [ESC] Done");
    } else if ctrl.selected_item >= 2 {
        widgets::draw_footer(lcd, "[OK] Edit Pt   [ESC] Back");
    } else {
        widgets::draw_footer(lcd, "[OK] Toggle    [ESC] Back");
    }
}

#[inline(never)]
fn wing_gvars_row(model: &crate::storage::ModelConfig) -> usize {
    if matches!(model.wing_tail_mix, 1 | 3 | 4) {
        2
    } else {
        1
    }
}

#[inline(never)]
pub fn update_wing_mixer(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    storage: &mut RadioStorage,
    buzzer: &mut Buzzer,
) {
    let template = storage.active_model.wing_tail_mix;
    let has_diff = template == 1 || template == 3;
    let gvar_item_idx = wing_gvars_row(&storage.active_model);
    let mix_items = gvar_item_idx + 1 + storage::NUM_MIX_LINES;

    if ctrl.editing {
        if keys.cancel {
            ctrl.editing = false;
            buzzer.click();
            return;
        }
        if keys.up && storage.active_model.template_diff <= 95 {
            storage.active_model.template_diff += 5;
            buzzer.play_tone(2200, 20);
        } else if keys.down && storage.active_model.template_diff >= -95 {
            storage.active_model.template_diff -= 5;
            buzzer.play_tone(2200, 20);
        }
        if keys.ok {
            ctrl.editing = false;
            buzzer.click();
        }
    } else {
        if keys.cancel {
            if !storage::save_active_model(storage) {
                widgets::draw_footer(lcd, "Save failed: retry ESC");
                buzzer.play_tone(400, 100);
                return;
            }
            ctrl.return_to_main_menu();
            buzzer.click();
            return;
        }

        widgets::navigate_4slot_list(
            &mut ctrl.selected_item,
            &mut ctrl.scroll_offset,
            mix_items,
            keys.up,
            keys.down,
            buzzer,
        );

        if keys.ok {
            buzzer.click();
            if ctrl.selected_item == 0 {
                // Recompute dynamic rows below after changing the template.
                storage.active_model.wing_tail_mix = (storage.active_model.wing_tail_mix + 1) % 5;
            } else if crate::mixer::is_tank(&storage.active_model) && ctrl.selected_item == 1 {
                ctrl.selected_item = 0;
                ctrl.scroll_offset = 0;
                ctrl.editing = false;
                ctrl.state = MenuState::TankSetup;
                ctrl.waiting_release = true;
                return;
            } else if has_diff && ctrl.selected_item == 1 {
                ctrl.editing = true;
            } else if ctrl.selected_item == gvar_item_idx {
                // Open Global Variables Editor
                ctrl.selected_item = 0;
                ctrl.scroll_offset = 0;
                ctrl.editing = false;
                ctrl.state = MenuState::GlobalVars;
                ctrl.waiting_release = true;
                return;
            } else {
                // Open Mix Line Editor
                let m_idx = ctrl.selected_item - (gvar_item_idx + 1);
                ctrl.page_idx = m_idx.min(31);
                ctrl.selected_item = 0;
                ctrl.scroll_offset = 0;
                ctrl.editing = false;
                ctrl.state = MenuState::MixerLineEdit;
                ctrl.waiting_release = true;
                return;
            }
        }
    }

    let has_diff = matches!(storage.active_model.wing_tail_mix, 1 | 3);
    let gvar_item_idx = wing_gvars_row(&storage.active_model);
    let mix_items = gvar_item_idx + 1 + storage::NUM_MIX_LINES;
    widgets::draw_header(lcd, "WING & MIXER");

    for slot in 0..4 {
        let idx = ctrl.scroll_offset + slot;
        if idx >= mix_items {
            break;
        }
        render_wing_mixer_row(
            lcd,
            slot,
            idx,
            idx == ctrl.selected_item,
            has_diff,
            gvar_item_idx,
            storage,
        );
    }

    let footer = if ctrl.editing {
        "[OK] Done   [UP/DN] Diff"
    } else {
        "[OK] Select/Edit   [ESC] Back"
    };
    widgets::draw_footer(lcd, footer);
}

#[inline(never)]
fn render_wing_mixer_row(
    lcd: &mut St7567,
    slot: usize,
    idx: usize,
    is_sel: bool,
    has_diff: bool,
    gvar_item_idx: usize,
    storage: &RadioStorage,
) {
    let y = 14 + (slot as i32 * 9);
    if is_sel {
        lcd.fill_rect(2, y, 124, 9, true);
    }

    let mut b6 = [0u8; 6];
    let mut ch_buf = [0u8; 4];
    if idx == 0 {
        let t_idx = (storage.active_model.wing_tail_mix as usize).min(4);
        lcd.draw_str_6x10(4, y, "Wing:", is_sel);
        lcd.draw_str_6x10(36, y, TEMPLATE_NAMES[t_idx], is_sel);
    } else if crate::mixer::is_tank(&storage.active_model) && idx == 1 {
        lcd.draw_str_6x10(4, y, "Tank Setup: [SETUP]", is_sel);
    } else if has_diff && idx == 1 {
        let d_str = i8_to_dec(storage.active_model.template_diff, &mut b6);
        lcd.draw_str_6x10(4, y, "Diff:", is_sel);
        lcd.draw_str_6x10(36, y, d_str, is_sel);
    } else if idx == gvar_item_idx {
        lcd.draw_str_6x10(4, y, "GVars: [Edit GV1..5]", is_sel);
    } else {
        let m_idx = idx - (gvar_item_idx + 1);
        let mix = storage.active_model.mixes[m_idx];
        let mut m_buf = *b"M00: ";
        let m_num = (m_idx + 1) as u8;
        m_buf[1] = b'0' + (m_num / 10);
        m_buf[2] = b'0' + (m_num % 10);
        let m_label = ascii_as_str(&m_buf);
        lcd.draw_str_6x10(4, y, m_label, is_sel);

        if mix.target_ch == 0 {
            lcd.draw_str_6x10(32, y, "[DISABLED]", is_sel);
        } else {
            let ch_str = format_ch_str(mix.target_ch, &mut ch_buf);
            lcd.draw_str_6x10(30, y, ch_str, is_sel);

            lcd.draw_str_6x10(58, y, "<-", is_sel);
            let s_idx = (mix.source as usize).min(SOURCE_NAMES.len() - 1);
            lcd.draw_str_6x10(74, y, SOURCE_NAMES[s_idx], is_sel);
        }
    }
}

#[inline(never)]
pub fn update_tank_setup(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    storage: &mut RadioStorage,
    buzzer: &mut Buzzer,
) {
    const LABELS: [&str; 5] = [
        "Drive Src:",
        "Steer Src:",
        "Left Out:",
        "Right Out:",
        "Steer Wt:",
    ];
    if keys.cancel {
        if ctrl.editing {
            ctrl.editing = false;
        } else {
            ctrl.state = MenuState::WingMixer;
            ctrl.selected_item = 1;
            ctrl.scroll_offset = 0;
            ctrl.waiting_release = true;
            buzzer.click();
            return;
        }
    } else if !ctrl.editing {
        widgets::navigate_4slot_list(
            &mut ctrl.selected_item,
            &mut ctrl.scroll_offset,
            LABELS.len(),
            keys.up,
            keys.down,
            buzzer,
        );
        if keys.ok {
            ctrl.editing = true;
            buzzer.click();
        }
    } else {
        let mut tank = storage.active_model.tank;
        match ctrl.selected_item {
            0 | 1 => {
                let source = if ctrl.selected_item == 0 {
                    &mut tank.drive_source
                } else {
                    &mut tank.steer_source
                };
                let source_count = SOURCE_NAMES.len();
                if let Some(pot) = keys.pot_change {
                    if (pot as usize) < source_count && storage.is_source_available(pot) {
                        *source = pot;
                        buzzer.play_tone(2400, 40);
                    }
                } else if keys.up {
                    *source = storage.next_available_source(*source, source_count);
                    buzzer.play_tone(2200, 20);
                } else if keys.down {
                    *source = storage.prev_available_source(*source, source_count);
                    buzzer.play_tone(2200, 20);
                }
            }
            2 | 3 => {
                let other = if ctrl.selected_item == 2 {
                    tank.right_channel
                } else {
                    tank.left_channel
                };
                let output = if ctrl.selected_item == 2 {
                    &mut tank.left_channel
                } else {
                    &mut tank.right_channel
                };
                if keys.up || keys.down {
                    loop {
                        *output = if keys.up {
                            if *output >= 18 {
                                1
                            } else {
                                *output + 1
                            }
                        } else if *output <= 1 {
                            18
                        } else {
                            *output - 1
                        };
                        if *output != other {
                            break;
                        }
                    }
                    buzzer.play_tone(2200, 20);
                }
            }
            4 => adjust_clamped_i8(
                &mut tank.steer_weight,
                -100,
                100,
                1,
                keys.up,
                keys.down,
                buzzer,
            ),
            _ => {}
        }
        storage.active_model.tank = tank;
        if keys.ok {
            ctrl.editing = false;
            buzzer.click();
        }
    }

    widgets::draw_header(lcd, "TANK SETUP");
    let tank = &storage.active_model.tank;
    let mut ch_buf = [0u8; 4];
    let mut weight_buf = [0u8; 6];
    for slot in 0..4 {
        let idx = ctrl.scroll_offset + slot;
        if idx >= LABELS.len() {
            break;
        }
        let value = match idx {
            0 => SOURCE_NAMES[(tank.drive_source as usize).min(SOURCE_NAMES.len() - 1)],
            1 => SOURCE_NAMES[(tank.steer_source as usize).min(SOURCE_NAMES.len() - 1)],
            2 => format_ch_str(tank.left_channel, &mut ch_buf),
            3 => format_ch_str(tank.right_channel, &mut ch_buf),
            _ => i8_to_dec(tank.steer_weight, &mut weight_buf),
        };
        widgets::draw_list_row(
            lcd,
            slot,
            idx == ctrl.selected_item,
            LABELS[idx],
            Some(value),
            68,
        );
    }
    widgets::draw_footer(
        lcd,
        if ctrl.editing {
            "[OK] Done   [UP/DN] Value"
        } else {
            "[OK] Edit   [ESC] Back"
        },
    );
}

#[inline(never)]
pub fn update_mixer_line_edit(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    storage: &mut RadioStorage,
    buzzer: &mut Buzzer,
) {
    let mix_idx = ctrl.page_idx.min(31);
    const FIELD_COUNT: usize = 7;

    if keys.cancel {
        if ctrl.editing {
            ctrl.editing = false;
        } else {
            let first_mix_row = wing_gvars_row(&storage.active_model) + 1;
            let target_row = first_mix_row + mix_idx;
            ctrl.state = MenuState::WingMixer;
            ctrl.selected_item = target_row;
            ctrl.scroll_offset = target_row.saturating_sub(3);
            ctrl.waiting_release = true;
            buzzer.click();
            return;
        }
    }

    if !ctrl.editing {
        widgets::navigate_4slot_list(
            &mut ctrl.selected_item,
            &mut ctrl.scroll_offset,
            FIELD_COUNT,
            keys.up,
            keys.down,
            buzzer,
        );

        if keys.ok {
            ctrl.editing = true;
            buzzer.click();
        }
    } else {
        let mut mix = storage.active_model.mixes[mix_idx];
        match ctrl.selected_item {
            0 => {
                if keys.up {
                    mix.target_ch = (mix.target_ch + 1) % 19;
                    buzzer.play_tone(2200, 20);
                } else if keys.down {
                    mix.target_ch = if mix.target_ch == 0 {
                        18
                    } else {
                        mix.target_ch - 1
                    };
                    buzzer.play_tone(2200, 20);
                }
            }
            1 => {
                if let Some(pot) = keys.pot_change {
                    if storage.is_source_available(pot) {
                        mix.source = pot;
                        buzzer.play_tone(2400, 40);
                    }
                } else if keys.up {
                    mix.source = storage.next_available_source(mix.source, SOURCE_NAMES.len());
                    buzzer.play_tone(2200, 20);
                } else if keys.down {
                    mix.source = storage.prev_available_source(mix.source, SOURCE_NAMES.len());
                    buzzer.play_tone(2200, 20);
                }
            }
            2 => {
                // Weight Modulation Source: 0..=36 (WEIGHT_SRC_NAMES)
                if keys.up {
                    mix.weight_src =
                        storage.next_available_weight_src(mix.weight_src, WEIGHT_SRC_NAMES.len());
                    buzzer.play_tone(2200, 20);
                } else if keys.down {
                    mix.weight_src =
                        storage.prev_available_weight_src(mix.weight_src, WEIGHT_SRC_NAMES.len());
                    buzzer.play_tone(2200, 20);
                }
            }
            3 => {
                adjust_clamped_i8(&mut mix.weight, -100, 100, 5, keys.up, keys.down, buzzer);
            }
            4 => {
                adjust_clamped_i8(&mut mix.offset, -100, 100, 5, keys.up, keys.down, buzzer);
            }
            5 => {
                if let Some(sw) = keys.sw_change {
                    if storage.is_switch_cond_available(sw) {
                        mix.switch = sw;
                        buzzer.play_tone(2400, 40);
                    }
                } else if keys.up {
                    mix.switch =
                        storage.next_available_switch_cond(mix.switch, SWITCH_COND_NAMES.len());
                    buzzer.play_tone(2200, 20);
                } else if keys.down {
                    mix.switch =
                        storage.prev_available_switch_cond(mix.switch, SWITCH_COND_NAMES.len());
                    buzzer.play_tone(2200, 20);
                }
            }
            6 => {
                if keys.up {
                    mix.mode = (mix.mode + 1) % 3;
                    buzzer.play_tone(2200, 20);
                } else if keys.down {
                    mix.mode = if mix.mode == 0 { 2 } else { mix.mode - 1 };
                    buzzer.play_tone(2200, 20);
                }
            }
            _ => {}
        }
        storage.active_model.mixes[mix_idx] = mix;

        if keys.ok {
            ctrl.editing = false;
            buzzer.click();
        }
    }

    let mut title_buf = *b"EDIT MIX 00";
    let m_num = (mix_idx + 1) as u8;
    title_buf[9] = b'0' + (m_num / 10);
    title_buf[10] = b'0' + (m_num % 10);
    let title_str = ascii_as_str(&title_buf);
    widgets::draw_header(lcd, title_str);

    let mix = storage.active_model.mixes[mix_idx];
    for slot in 0..4 {
        let idx = ctrl.scroll_offset + slot;
        if idx >= FIELD_COUNT {
            break;
        }
        render_mixer_line_edit_row(lcd, slot, idx, idx == ctrl.selected_item, &mix);
    }

    let footer = if ctrl.editing {
        "[OK] Done   [UP/DN] Value"
    } else {
        "[OK] Edit   [ESC] Back"
    };
    widgets::draw_footer(lcd, footer);
}

#[inline(never)]
fn render_mixer_line_edit_row(
    lcd: &mut St7567,
    slot: usize,
    idx: usize,
    is_sel: bool,
    mix: &crate::storage::MixLine,
) {
    let mut b6 = [0u8; 6];
    let mut ch_buf = [0u8; 4];
    match idx {
        0 => {
            if mix.target_ch == 0 {
                widgets::draw_list_row(lcd, slot, is_sel, "Target:", Some("Disabled"), 56);
            } else {
                let ch_str = format_ch_str(mix.target_ch, &mut ch_buf);
                widgets::draw_list_row(lcd, slot, is_sel, "Target:", Some(ch_str), 56);
            }
        }
        1 => {
            let s_idx = (mix.source as usize).min(SOURCE_NAMES.len() - 1);
            widgets::draw_list_row(lcd, slot, is_sel, "Source:", Some(SOURCE_NAMES[s_idx]), 56);
        }
        2 => {
            let ws_idx = (mix.weight_src as usize).min(WEIGHT_SRC_NAMES.len() - 1);
            widgets::draw_list_row(
                lcd,
                slot,
                is_sel,
                "Wt Src:",
                Some(WEIGHT_SRC_NAMES[ws_idx]),
                56,
            );
        }
        3 => {
            let w_str = i8_to_dec(mix.weight, &mut b6);
            widgets::draw_list_row(lcd, slot, is_sel, "Weight:", Some(w_str), 56);
        }
        4 => {
            let o_str = i8_to_dec(mix.offset, &mut b6);
            widgets::draw_list_row(lcd, slot, is_sel, "Offset:", Some(o_str), 56);
        }
        5 => {
            let sw_idx = (mix.switch as usize).min(SWITCH_COND_NAMES.len() - 1);
            widgets::draw_list_row(
                lcd,
                slot,
                is_sel,
                "Switch:",
                Some(SWITCH_COND_NAMES[sw_idx]),
                56,
            );
        }
        6 => {
            let m_idx = (mix.mode as usize).min(2);
            widgets::draw_list_row(lcd, slot, is_sel, "Mode:", Some(MODE_NAMES[m_idx]), 56);
        }
        _ => {}
    }
}

#[inline(never)]
pub fn update_global_vars(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    storage: &mut RadioStorage,
    buzzer: &mut Buzzer,
) {
    const GVAR_COUNT: usize = 5;

    if ctrl.editing {
        let gv_idx = ctrl.selected_item.min(GVAR_COUNT - 1);
        let cur = storage.active_model.gvars[gv_idx];
        if keys.up && cur <= 950 {
            storage.active_model.gvars[gv_idx] = cur + 50; // 5% step
            buzzer.play_tone(2200, 20);
        } else if keys.down && cur >= -950 {
            storage.active_model.gvars[gv_idx] = cur - 50; // 5% step
            buzzer.play_tone(2200, 20);
        }
        if keys.ok || keys.cancel {
            if !storage::save_active_model(storage) {
                widgets::draw_footer(lcd, "Save failed: retry OK");
                buzzer.play_tone(400, 100);
                return;
            }
            ctrl.editing = false;
            buzzer.click();
        }
    } else {
        if keys.cancel {
            if !storage::save_active_model(storage) {
                widgets::draw_footer(lcd, "Save failed: retry ESC");
                buzzer.play_tone(400, 100);
                return;
            }
            let gvar_item_idx = wing_gvars_row(&storage.active_model);
            ctrl.state = MenuState::WingMixer;
            ctrl.selected_item = gvar_item_idx;
            ctrl.scroll_offset = gvar_item_idx.saturating_sub(3);
            ctrl.waiting_release = true;
            buzzer.click();
            return;
        }

        widgets::navigate_4slot_list(
            &mut ctrl.selected_item,
            &mut ctrl.scroll_offset,
            GVAR_COUNT,
            keys.up,
            keys.down,
            buzzer,
        );

        if keys.ok {
            ctrl.editing = true;
            buzzer.click();
        }
    }

    widgets::draw_header(lcd, "GLOBAL VARIABLES");

    let mut b5 = [0u8; 5];
    for slot in 0..4 {
        let idx = ctrl.scroll_offset + slot;
        if idx >= GVAR_COUNT {
            break;
        }
        let is_sel = idx == ctrl.selected_item;
        let mut label_buf = *b"GV0:";
        label_buf[2] = b'1' + (idx as u8);
        let label_str = ascii_as_str(&label_buf);
        let val = storage.active_model.gvars[idx];
        let val_str = crate::ui::format::format_percent(val, &mut b5);
        widgets::draw_list_row(lcd, slot, is_sel, label_str, Some(val_str), 56);
    }

    let footer = if ctrl.editing {
        "[OK] Done   [UP/DN] Value"
    } else {
        "[OK] Edit   [ESC] Back"
    };
    widgets::draw_footer(lcd, footer);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nav() -> NavKeys {
        NavKeys {
            ok: false,
            cancel: false,
            up: false,
            down: false,
            bind: false,
            sw_change: None,
            dr_change: None,
            pot_change: None,
        }
    }

    #[test]
    fn test_tank_menu_navigation_and_fields() {
        let mut ctrl = MenuController::new();
        let mut lcd = St7567::new();
        let mut storage = RadioStorage::default_factory();
        let mut buzzer = Buzzer::new();
        ctrl.state = MenuState::WingMixer;
        storage.active_model.wing_tail_mix = 3;
        let ok = NavKeys { ok: true, ..nav() };
        let up = NavKeys { up: true, ..nav() };
        let down = NavKeys {
            down: true,
            ..nav()
        };
        let cancel = NavKeys {
            cancel: true,
            ..nav()
        };
        update_wing_mixer(&mut ctrl, &mut lcd, &ok, &mut storage, &mut buzzer);
        assert_eq!(storage.active_model.wing_tail_mix, 4);
        ctrl.selected_item = 1;
        update_wing_mixer(&mut ctrl, &mut lcd, &ok, &mut storage, &mut buzzer);
        assert_eq!(ctrl.state, MenuState::TankSetup);
        assert_eq!(ctrl.selected_item, 0);
        update_tank_setup(&mut ctrl, &mut lcd, &ok, &mut storage, &mut buzzer);
        update_tank_setup(&mut ctrl, &mut lcd, &up, &mut storage, &mut buzzer);
        assert_eq!(storage.active_model.tank.drive_source, 4);
        storage.radio.adc_modes[4] = storage::AdcInputMode::Off as u8;
        update_tank_setup(&mut ctrl, &mut lcd, &up, &mut storage, &mut buzzer);
        assert_eq!(storage.active_model.tank.drive_source, 6);
        let moved = NavKeys {
            pot_change: Some(5),
            ..nav()
        };
        update_tank_setup(&mut ctrl, &mut lcd, &moved, &mut storage, &mut buzzer);
        assert_eq!(storage.active_model.tank.drive_source, 6);
        storage.radio.adc_modes[4] = storage::AdcInputMode::Pot as u8;
        update_tank_setup(&mut ctrl, &mut lcd, &moved, &mut storage, &mut buzzer);
        assert_eq!(storage.active_model.tank.drive_source, 5);
        storage.active_model.tank.drive_source = crate::mixer::MixSource::Gv1 as u8;
        update_tank_setup(&mut ctrl, &mut lcd, &up, &mut storage, &mut buzzer);
        assert_eq!(storage.active_model.tank.drive_source, 46);
        storage.active_model.tank.drive_source = crate::mixer::MixSource::Gv5 as u8;
        update_tank_setup(&mut ctrl, &mut lcd, &up, &mut storage, &mut buzzer);
        assert_eq!(storage.active_model.tank.drive_source, 0);
        update_tank_setup(&mut ctrl, &mut lcd, &ok, &mut storage, &mut buzzer);
        assert!(!ctrl.editing);
        ctrl.selected_item = 1;
        update_tank_setup(&mut ctrl, &mut lcd, &ok, &mut storage, &mut buzzer);
        update_tank_setup(&mut ctrl, &mut lcd, &down, &mut storage, &mut buzzer);
        assert_eq!(storage.active_model.tank.steer_source, 3);
        update_tank_setup(&mut ctrl, &mut lcd, &cancel, &mut storage, &mut buzzer);
        assert_eq!(ctrl.state, MenuState::TankSetup);
        assert!(!ctrl.editing);
        ctrl.selected_item = 2;
        ctrl.editing = true;
        update_tank_setup(&mut ctrl, &mut lcd, &up, &mut storage, &mut buzzer);
        assert_eq!(storage.active_model.tank.left_channel, 3);
        storage.active_model.tank.left_channel = 18;
        update_tank_setup(&mut ctrl, &mut lcd, &up, &mut storage, &mut buzzer);
        assert_eq!(storage.active_model.tank.left_channel, 1);
        ctrl.selected_item = 3;
        update_tank_setup(&mut ctrl, &mut lcd, &down, &mut storage, &mut buzzer);
        assert_eq!(storage.active_model.tank.right_channel, 18);
        ctrl.selected_item = 4;
        ctrl.scroll_offset = 1;
        update_tank_setup(&mut ctrl, &mut lcd, &up, &mut storage, &mut buzzer);
        assert_eq!(storage.active_model.tank.steer_weight, 100);
        update_tank_setup(&mut ctrl, &mut lcd, &down, &mut storage, &mut buzzer);
        assert_eq!(storage.active_model.tank.steer_weight, 99);
        for _ in 0..200 {
            update_tank_setup(&mut ctrl, &mut lcd, &down, &mut storage, &mut buzzer);
        }
        assert_eq!(storage.active_model.tank.steer_weight, -100);
        update_tank_setup(&mut ctrl, &mut lcd, &ok, &mut storage, &mut buzzer);
        update_tank_setup(&mut ctrl, &mut lcd, &cancel, &mut storage, &mut buzzer);
        assert_eq!(ctrl.state, MenuState::WingMixer);
        assert_eq!(ctrl.selected_item, 1);
        assert_eq!(ctrl.scroll_offset, 0);
        ctrl.selected_item = 2;
        update_wing_mixer(&mut ctrl, &mut lcd, &ok, &mut storage, &mut buzzer);
        assert_eq!(ctrl.state, MenuState::GlobalVars);
        update_global_vars(&mut ctrl, &mut lcd, &cancel, &mut storage, &mut buzzer);
        assert_eq!(ctrl.selected_item, 2);
        ctrl.selected_item = 34;
        update_wing_mixer(&mut ctrl, &mut lcd, &ok, &mut storage, &mut buzzer);
        assert_eq!(ctrl.state, MenuState::MixerLineEdit);
        assert_eq!(ctrl.page_idx, 31);
        update_mixer_line_edit(&mut ctrl, &mut lcd, &cancel, &mut storage, &mut buzzer);
        assert_eq!(ctrl.state, MenuState::WingMixer);
        assert_eq!(ctrl.selected_item, 34);
        assert_eq!(ctrl.scroll_offset, 31);
        ctrl.selected_item = 0;
        ctrl.scroll_offset = 0;
        update_wing_mixer(&mut ctrl, &mut lcd, &ok, &mut storage, &mut buzzer);
        assert_eq!(storage.active_model.wing_tail_mix, 0);
        ctrl.selected_item = 1;
        update_wing_mixer(&mut ctrl, &mut lcd, &ok, &mut storage, &mut buzzer);
        assert_eq!(ctrl.state, MenuState::GlobalVars);
    }
}
