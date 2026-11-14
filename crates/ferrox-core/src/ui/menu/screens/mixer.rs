//! Dual Rate / Expo, Throttle Curve, Wing / Tank Templates, and Mixer Line Editor screens.

use crate::buzzer::Buzzer;
#[cfg(feature = "tank-mix")]
use crate::board::{ActiveBoard, BoardProfile};
use crate::curve;
use crate::display::St7567;
#[cfg(any(feature = "logical-switches", test))]
use crate::menu::format::LOGICAL_FUNC_NAMES;
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

fn wing_gvars_row(template: u8) -> usize {
    if matches!(template, 1 | 3)
        || (cfg!(feature = "tank-mix") && template == crate::mixer::WingTailTemplate::Tank as u8)
    {
        2
    } else {
        1
    }
}

#[cfg(all(test, not(feature = "tank-mix")))]
mod disabled_tank_tests {
    use super::*;

    #[test]
    fn template_selection_skips_disabled_tank_but_preserves_its_configuration() {
        let mut storage = RadioStorage::default_factory();
        let mut ctrl = MenuController::new();
        let mut lcd = St7567::new();
        let mut buzzer = Buzzer::new();
        let keys = NavKeys {
            ok: true,
            cancel: false,
            up: false,
            down: false,
            bind: false,
            sw_change: None,
            dr_change: None,
            pot_change: None,
        };
        ctrl.state = MenuState::WingMixer;
        storage.active_model.wing_tail_mix = 4;
        update_wing_mixer(&mut ctrl, &mut lcd, &keys, &mut storage, &mut buzzer);
        if cfg!(feature = "baitboat") {
            assert_eq!(storage.active_model.wing_tail_mix, 5);
            update_wing_mixer(&mut ctrl, &mut lcd, &keys, &mut storage, &mut buzzer);
        }
        assert_eq!(storage.active_model.wing_tail_mix, 0);
        storage.active_model.wing_tail_mix = crate::mixer::WingTailTemplate::Tank as u8;
        storage.active_model.tank.steer_weight = -75;
        storage.sanitize();
        assert!(crate::mixer::is_tank(&storage.active_model));
        assert_eq!(wing_gvars_row(storage.active_model.wing_tail_mix), 1);
        update_wing_mixer(&mut ctrl, &mut lcd, &keys, &mut storage, &mut buzzer);
        assert_eq!(storage.active_model.wing_tail_mix, 0);
        assert_eq!(storage.active_model.tank.steer_weight, -75);
    }
}

#[cfg(all(test, feature = "tank-mix"))]
mod tank_tests {
    use super::*;

    fn keys() -> NavKeys {
        NavKeys {
            ok: false, cancel: false, up: false, down: false, bind: false,
            sw_change: None, dr_change: None, pot_change: None,
        }
    }

    #[test]
    fn tank_five_fields_and_available_sources() {
        let mut storage = RadioStorage::default_factory();
        let mut nav = keys();
        nav.pot_change = Some(45);
        assert!(edit_tank_field(&mut storage, 0, &nav));
        assert_eq!(storage.active_model.tank.drive_source, 45);
        nav.pot_change = Some(57);
        assert!(edit_tank_field(&mut storage, 1, &nav));
        assert_eq!(storage.active_model.tank.steer_source, 57);
        nav.pot_change = Some(58);
        assert!(!edit_tank_field(&mut storage, 0, &nav));
        storage.radio.adc_modes[4] = storage::AdcInputMode::Off as u8;
        nav.pot_change = Some(5);
        assert!(!edit_tank_field(&mut storage, 0, &nav));
        nav.pot_change = None;
        nav.up = true;
        storage.active_model.tank.drive_source = 44;
        assert!(edit_tank_field(&mut storage, 0, &nav));
        assert_eq!(storage.active_model.tank.drive_source, 45);
        assert!(edit_tank_field(&mut storage, 2, &nav));
        assert_eq!(storage.active_model.tank.left_channel, 3);
        nav.up = false;
        nav.down = true;
        assert!(edit_tank_field(&mut storage, 3, &nav));
        assert_eq!(storage.active_model.tank.right_channel, 1);
        assert!(edit_tank_field(&mut storage, 4, &nav));
        assert_eq!(storage.active_model.tank.steer_weight, 99);
        storage.active_model.tank.steer_weight = -100;
        assert!(!edit_tank_field(&mut storage, 4, &nav));
    }

    #[test]
    fn tank_dynamic_rows_and_submenu_returns() {
        let mut storage = RadioStorage::default_factory();
        let mut ctrl = MenuController::new();
        let mut lcd = St7567::new();
        let mut buzzer = Buzzer::new();
        ctrl.state = MenuState::WingMixer;
        storage.active_model.wing_tail_mix = 4;
        let mut nav = keys();
        nav.ok = true;
        update_wing_mixer(&mut ctrl, &mut lcd, &nav, &mut storage, &mut buzzer);
        if cfg!(feature = "baitboat") {
            assert_eq!(storage.active_model.wing_tail_mix, 5);
            assert_eq!(wing_gvars_row(5), 1);
            update_wing_mixer(&mut ctrl, &mut lcd, &nav, &mut storage, &mut buzzer);
        }
        assert_eq!(
            storage.active_model.wing_tail_mix,
            crate::mixer::WingTailTemplate::Tank as u8
        );
        assert_eq!(wing_gvars_row(4), 1);
        assert_eq!(
            wing_gvars_row(crate::mixer::WingTailTemplate::Tank as u8),
            2
        );
        ctrl.selected_item = 1;
        update_wing_mixer(&mut ctrl, &mut lcd, &nav, &mut storage, &mut buzzer);
        assert_eq!(ctrl.state, MenuState::TankSetup);
        let mut cancel = keys();
        cancel.cancel = true;
        update_tank_setup(&mut ctrl, &mut lcd, &cancel, &mut storage, &mut buzzer);
        assert_eq!((ctrl.state, ctrl.selected_item), (MenuState::WingMixer, 1));
        ctrl.selected_item = 2;
        update_wing_mixer(&mut ctrl, &mut lcd, &nav, &mut storage, &mut buzzer);
        assert_eq!(ctrl.state, MenuState::GlobalVars);
        update_global_vars(&mut ctrl, &mut lcd, &cancel, &mut storage, &mut buzzer);
        assert_eq!((ctrl.state, ctrl.selected_item), (MenuState::WingMixer, 2));
        ctrl.selected_item = 34;
        update_wing_mixer(&mut ctrl, &mut lcd, &nav, &mut storage, &mut buzzer);
        assert_eq!((ctrl.state, ctrl.page_idx), (MenuState::MixerLineEdit, 31));
        update_mixer_line_edit(&mut ctrl, &mut lcd, &cancel, &mut storage, &mut buzzer);
        assert_eq!((ctrl.state, ctrl.selected_item, ctrl.scroll_offset),
            (MenuState::WingMixer, 34, 31));
        ctrl.selected_item = 0;
        update_wing_mixer(&mut ctrl, &mut lcd, &nav, &mut storage, &mut buzzer);
        assert_eq!(storage.active_model.wing_tail_mix, 0);
    }
}

#[cfg(feature = "tank-mix")]
const TANK_LABELS: [&str; 5] = [
    "Drive Src:", "Steer Src:", "Left Out:", "Right Out:", "Steer Wt:",
];

#[cfg(feature = "tank-mix")]
fn tank_source_available(storage: &RadioStorage, source: u8) -> bool {
    (source as usize) < SOURCE_NAMES.len()
        && ((45..=57).contains(&source)
            || ActiveBoard::is_source_available(&storage.radio, &storage.active_model, source))
}

#[cfg(feature = "tank-mix")]
fn step_tank_source(storage: &RadioStorage, source: u8, down: bool) -> u8 {
    let count = SOURCE_NAMES.len() as u16;
    let start = source.min((count - 1) as u8);
    for step in 1..=count {
        let candidate = if down {
            (start as u16 + count - step) % count
        } else {
            (start as u16 + step) % count
        } as u8;
        if tank_source_available(storage, candidate) {
            return candidate;
        }
    }
    source
}

#[cfg(feature = "tank-mix")]
fn edit_tank_field(storage: &mut RadioStorage, row: usize, keys: &NavKeys) -> bool {
    if row <= 1 {
        let current = if row == 0 {
            storage.active_model.tank.drive_source
        } else {
            storage.active_model.tank.steer_source
        };
        let next = if let Some(source) = keys.pot_change {
            if !tank_source_available(storage, source) {
                return false;
            }
            source
        } else if keys.up != keys.down {
            step_tank_source(storage, current, keys.down)
        } else {
            return false;
        };
        if row == 0 {
            storage.active_model.tank.drive_source = next;
        } else {
            storage.active_model.tank.steer_source = next;
        }
        return next != current;
    }
    let tank = &mut storage.active_model.tank;
    if keys.up == keys.down {
        return false;
    }
    match row {
        2 | 3 => {
            let (channel, other) = if row == 2 {
                (&mut tank.left_channel, tank.right_channel)
            } else {
                (&mut tank.right_channel, tank.left_channel)
            };
            loop {
                *channel = if keys.down {
                    if *channel <= 1 { 18 } else { *channel - 1 }
                } else {
                    if *channel >= 18 { 1 } else { *channel + 1 }
                };
                if *channel != other {
                    break;
                }
            }
            true
        }
        4 => {
            let next = (tank.steer_weight as i16 + if keys.down { -1 } else { 1 })
                .clamp(-100, 100) as i8;
            let changed = next != tank.steer_weight;
            tank.steer_weight = next;
            changed
        }
        _ => false,
    }
}

#[inline(never)]
#[cfg(feature = "tank-mix")]
pub fn update_tank_setup(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    storage: &mut RadioStorage,
    buzzer: &mut Buzzer,
) {
    if keys.cancel || (ctrl.editing && keys.ok) {
        if !storage::save_active_model(storage) {
            widgets::draw_footer(lcd, "SAVE ERROR - retry");
            return;
        }
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
        buzzer.click();
    } else if ctrl.editing {
        if edit_tank_field(storage, ctrl.selected_item, keys) {
            buzzer.play_tone(2200, 20);
        }
    } else {
        widgets::navigate_4slot_list(
            &mut ctrl.selected_item, &mut ctrl.scroll_offset, TANK_LABELS.len(),
            keys.up, keys.down, buzzer,
        );
        if keys.ok {
            ctrl.editing = true;
            buzzer.click();
        }
    }

    widgets::draw_header(lcd, "TANK SETUP");
    let tank = &storage.active_model.tank;
    for slot in 0..4 {
        let row = ctrl.scroll_offset + slot;
        if row >= TANK_LABELS.len() {
            break;
        }
        let mut channel_buf = [0; 4];
        let mut weight_buf = [0; 6];
        let value = match row {
            0 => SOURCE_NAMES[(tank.drive_source as usize).min(SOURCE_NAMES.len() - 1)],
            1 => SOURCE_NAMES[(tank.steer_source as usize).min(SOURCE_NAMES.len() - 1)],
            2 => format_ch_str(tank.left_channel, &mut channel_buf),
            3 => format_ch_str(tank.right_channel, &mut channel_buf),
            _ => i8_to_dec(tank.steer_weight, &mut weight_buf),
        };
        widgets::draw_list_row(
            lcd, slot, row == ctrl.selected_item, TANK_LABELS[row], Some(value), 68,
        );
    }
    widgets::draw_footer(lcd, if ctrl.editing {
        "[OK] Done   [UP/DN] Value"
    } else {
        "[OK] Edit   [ESC] Back"
    });
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
    let gvar_item_idx = wing_gvars_row(template);
    let mix_items = gvar_item_idx + 1 + storage::NUM_MIX_LINES;

    if ctrl.editing {
        if keys.cancel {
            ctrl.editing = false;
            storage::save_active_model(storage);
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
            #[cfg(feature = "tank-mix")]
            if crate::mixer::is_tank(&storage.active_model) && ctrl.selected_item == 1 {
                ctrl.selected_item = 0;
                ctrl.scroll_offset = 0;
                ctrl.editing = false;
                ctrl.state = MenuState::TankSetup;
                ctrl.waiting_release = true;
                return;
            }
            if ctrl.selected_item == 0 {
                let next = storage.active_model.wing_tail_mix as usize + 1;
                let next = if next >= TEMPLATE_NAMES.len() {
                    0
                } else {
                    next as u8
                };
                storage.active_model.wing_tail_mix = if next == 5 && !cfg!(feature = "baitboat") {
                    next + 1
                } else {
                    next
                };
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

    let template = storage.active_model.wing_tail_mix;
    let has_diff = matches!(template, 1 | 3);
    let gvar_item_idx = wing_gvars_row(template);
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
    #[cfg(feature = "tank-mix")]
    if crate::mixer::is_tank(&storage.active_model) && idx == 1 {
        lcd.draw_str_6x10(4, y, "Tank Setup: [SETUP]", is_sel);
        return;
    }
    if idx == 0 {
        let t_idx = (storage.active_model.wing_tail_mix as usize).min(TEMPLATE_NAMES.len() - 1);
        lcd.draw_str_6x10(4, y, "Wing:", is_sel);
        let name = if !cfg!(feature = "tank-mix") && crate::mixer::is_tank(&storage.active_model) {
            "TANK OFF"
        } else {
            TEMPLATE_NAMES[t_idx]
        };
        lcd.draw_str_6x10(36, y, name, is_sel);
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
            let template = storage.active_model.wing_tail_mix;
            let first_mix_row = wing_gvars_row(template) + 1;
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
            ctrl.editing = false;
            storage::save_active_model(storage);
            buzzer.click();
        }
    } else {
        if keys.cancel {
            storage::save_active_model(storage);
            let template = storage.active_model.wing_tail_mix;
            let gvar_item_idx = wing_gvars_row(template);
            ctrl.state = MenuState::WingMixer;
            ctrl.selected_item = gvar_item_idx;
            ctrl.scroll_offset = gvar_item_idx.saturating_sub(GVAR_COUNT);
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

#[cfg(any(feature = "logical-switches", test))]
#[inline(never)]
pub fn update_logical_switches(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    switches: &crate::input::Switches,
    storage: &mut RadioStorage,
    buzzer: &mut Buzzer,
) {
    const FIELD_COUNT: usize = 7;
    let ls_idx = ctrl.sub_idx.min(crate::storage::NUM_LOGICAL_SWITCHES - 1);

    if !ctrl.editing {
        if keys.cancel {
            storage::save_active_model(storage);
            ctrl.state = MenuState::ModelSetup;
            ctrl.selected_item = 9;
            ctrl.scroll_offset = 6;
            ctrl.waiting_release = true;
            buzzer.click();
            return;
        }

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
        match ctrl.selected_item {
            0 => {
                // Select Logical Switch L1..L8
                if keys.up {
                    ctrl.sub_idx = (ls_idx + 1) % crate::storage::NUM_LOGICAL_SWITCHES;
                    buzzer.play_tone(2200, 20);
                } else if keys.down {
                    ctrl.sub_idx = if ls_idx == 0 {
                        crate::storage::NUM_LOGICAL_SWITCHES - 1
                    } else {
                        ls_idx - 1
                    };
                    buzzer.play_tone(2200, 20);
                }
            }
            1 => {
                // Function: 0..=10
                let ls = &mut storage.active_model.logical_switches[ls_idx];
                if keys.up {
                    ls.func = (ls.func + 1) % (LOGICAL_FUNC_NAMES.len() as u8);
                    ls.v1 = 0;
                    ls.v2 = 0;
                    buzzer.play_tone(2200, 20);
                } else if keys.down {
                    ls.func = if ls.func == 0 {
                        (LOGICAL_FUNC_NAMES.len() - 1) as u8
                    } else {
                        ls.func - 1
                    };
                    ls.v1 = 0;
                    ls.v2 = 0;
                    buzzer.play_tone(2200, 20);
                }
            }
            2 => {
                // V1: Source or Switch Condition
                let is_bool = storage.active_model.logical_switches[ls_idx].is_boolean_func();
                let cur = storage.active_model.logical_switches[ls_idx].v1;
                if is_bool {
                    if let Some(sw) = keys.sw_change {
                        if storage.is_switch_cond_available(sw) {
                            storage.active_model.logical_switches[ls_idx].v1 = sw;
                            buzzer.play_tone(2400, 40);
                        }
                    } else if keys.up {
                        storage.active_model.logical_switches[ls_idx].v1 =
                            storage.next_available_switch_cond(cur, SWITCH_COND_NAMES.len());
                        buzzer.play_tone(2200, 20);
                    } else if keys.down {
                        storage.active_model.logical_switches[ls_idx].v1 =
                            storage.prev_available_switch_cond(cur, SWITCH_COND_NAMES.len());
                        buzzer.play_tone(2200, 20);
                    }
                } else if keys.up {
                    storage.active_model.logical_switches[ls_idx].v1 =
                        storage.next_available_source(cur, SOURCE_NAMES.len());
                    buzzer.play_tone(2200, 20);
                } else if keys.down {
                    storage.active_model.logical_switches[ls_idx].v1 =
                        storage.prev_available_source(cur, SOURCE_NAMES.len());
                    buzzer.play_tone(2200, 20);
                }
            }
            3 => {
                // V2: Threshold (-100..+100%) or secondary Source/Switch
                let ls_copy = storage.active_model.logical_switches[ls_idx];
                if ls_copy.is_boolean_func() {
                    let cur = ls_copy.v2.max(0) as u8;
                    if let Some(sw) = keys.sw_change {
                        if storage.is_switch_cond_available(sw) {
                            storage.active_model.logical_switches[ls_idx].v2 = sw as i8;
                            buzzer.play_tone(2400, 40);
                        }
                    } else if keys.up {
                        storage.active_model.logical_switches[ls_idx].v2 =
                            storage.next_available_switch_cond(cur, SWITCH_COND_NAMES.len()) as i8;
                        buzzer.play_tone(2200, 20);
                    } else if keys.down {
                        storage.active_model.logical_switches[ls_idx].v2 =
                            storage.prev_available_switch_cond(cur, SWITCH_COND_NAMES.len()) as i8;
                        buzzer.play_tone(2200, 20);
                    }
                } else if ls_copy.is_dual_source_func() {
                    let cur = ls_copy.v2.max(0) as u8;
                    if keys.up {
                        storage.active_model.logical_switches[ls_idx].v2 =
                            storage.next_available_source(cur, SOURCE_NAMES.len()) as i8;
                        buzzer.play_tone(2200, 20);
                    } else if keys.down {
                        storage.active_model.logical_switches[ls_idx].v2 =
                            storage.prev_available_source(cur, SOURCE_NAMES.len()) as i8;
                        buzzer.play_tone(2200, 20);
                    }
                } else {
                    adjust_clamped_i8(
                        &mut storage.active_model.logical_switches[ls_idx].v2,
                        -100,
                        100,
                        5,
                        keys.up,
                        keys.down,
                        buzzer,
                    );
                }
            }
            4 => {
                // Secondary AND Switch
                let cur = storage.active_model.logical_switches[ls_idx].and_sw;
                if let Some(sw) = keys.sw_change {
                    if storage.is_switch_cond_available(sw) {
                        storage.active_model.logical_switches[ls_idx].and_sw = sw;
                        buzzer.play_tone(2400, 40);
                    }
                } else if keys.up {
                    storage.active_model.logical_switches[ls_idx].and_sw =
                        storage.next_available_switch_cond(cur, SWITCH_COND_NAMES.len());
                    buzzer.play_tone(2200, 20);
                } else if keys.down {
                    storage.active_model.logical_switches[ls_idx].and_sw =
                        storage.prev_available_switch_cond(cur, SWITCH_COND_NAMES.len());
                    buzzer.play_tone(2200, 20);
                }
            }
            5 => {
                // Delay (0.0s .. 10.0s in 0.1s steps)
                adjust_clamped_u8(
                    &mut storage.active_model.logical_switches[ls_idx].delay_tenths,
                    0,
                    100,
                    1,
                    keys.up,
                    keys.down,
                    buzzer,
                );
            }
            6 => {
                // Duration (0 = Cont., 0.1s .. 10.0s in 0.1s steps)
                adjust_clamped_u8(
                    &mut storage.active_model.logical_switches[ls_idx].duration_tenths,
                    0,
                    100,
                    1,
                    keys.up,
                    keys.down,
                    buzzer,
                );
            }
            _ => {}
        }

        if keys.ok || keys.cancel {
            ctrl.editing = false;
            storage::save_active_model(storage);
            buzzer.click();
        }
    }

    widgets::draw_header(lcd, "LOGICAL SWITCHES");

    let ls = storage.active_model.logical_switches[ls_idx];
    let is_active = (switches.logical & (1 << ls_idx)) != 0;
    let mut b6 = [0u8; 6];
    let mut t_buf = [0u8; 8];

    for slot in 0..4 {
        let idx = ctrl.scroll_offset + slot;
        if idx >= FIELD_COUNT {
            break;
        }
        let is_sel = idx == ctrl.selected_item;
        match idx {
            0 => {
                let mut sw_buf = *b"L1 [OFF]";
                sw_buf[1] = b'1' + (ls_idx as u8);
                if is_active {
                    sw_buf[4..8].copy_from_slice(b"ON] ");
                }
                let s = ascii_as_str(if is_active { &sw_buf[..7] } else { &sw_buf[..8] });
                widgets::draw_list_row(lcd, slot, is_sel, "Switch:", Some(s), 56);
            }
            1 => {
                let f_idx = (ls.func as usize).min(LOGICAL_FUNC_NAMES.len() - 1);
                widgets::draw_list_row(
                    lcd,
                    slot,
                    is_sel,
                    "Func:",
                    Some(LOGICAL_FUNC_NAMES[f_idx]),
                    56,
                );
            }
            2 => {
                let v1_str = if ls.is_boolean_func() {
                    let s_idx = (ls.v1 as usize).min(SWITCH_COND_NAMES.len() - 1);
                    SWITCH_COND_NAMES[s_idx]
                } else {
                    let s_idx = (ls.v1 as usize).min(SOURCE_NAMES.len() - 1);
                    SOURCE_NAMES[s_idx]
                };
                widgets::draw_list_row(lcd, slot, is_sel, "V1:", Some(v1_str), 56);
            }
            3 => {
                let v2_str = if ls.is_boolean_func() {
                    let s_idx = (ls.v2.max(0) as usize).min(SWITCH_COND_NAMES.len() - 1);
                    SWITCH_COND_NAMES[s_idx]
                } else if ls.is_dual_source_func() {
                    let s_idx = (ls.v2.max(0) as usize).min(SOURCE_NAMES.len() - 1);
                    SOURCE_NAMES[s_idx]
                } else {
                    i8_to_dec(ls.v2, &mut b6)
                };
                widgets::draw_list_row(lcd, slot, is_sel, "V2:", Some(v2_str), 56);
            }
            4 => {
                let and_idx = (ls.and_sw as usize).min(SWITCH_COND_NAMES.len() - 1);
                let and_str = if and_idx == 0 {
                    "---"
                } else {
                    SWITCH_COND_NAMES[and_idx]
                };
                widgets::draw_list_row(lcd, slot, is_sel, "AND Sw:", Some(and_str), 56);
            }
            5 => {
                let d_str = crate::ui::format::format_fixed(
                    ls.delay_tenths as i32,
                    1,
                    0,
                    false,
                    crate::ui::format::SignMode::None,
                    "s",
                    &mut t_buf,
                );
                widgets::draw_list_row(lcd, slot, is_sel, "Delay:", Some(d_str), 56);
            }
            6 => {
                let dur_str = if ls.duration_tenths == 0 {
                    "Cont."
                } else {
                    crate::ui::format::format_fixed(
                        ls.duration_tenths as i32,
                        1,
                        0,
                        false,
                        crate::ui::format::SignMode::None,
                        "s",
                        &mut t_buf,
                    )
                };
                widgets::draw_list_row(lcd, slot, is_sel, "Duration:", Some(dur_str), 56);
            }
            _ => {}
        }
    }

    let footer = if ctrl.editing {
        "[OK] Save   [UP/DN] Change"
    } else {
        "[OK] Edit   [ESC] Back"
    };
    widgets::draw_footer(lcd, footer);
}
