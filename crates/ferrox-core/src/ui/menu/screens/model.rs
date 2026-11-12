//! Model selection and configuration screens.

use crate::board::BoardProfile;
use crate::buzzer::Buzzer;
use crate::display::St7567;
use crate::menu::format::{ascii_as_str, next_ascii, prev_ascii};
use crate::menu::widgets;
use crate::menu::{MenuController, MenuState, NavKeys};
use crate::rf;
use crate::storage::{self, ModelConfig, RadioStorage, NUM_MODELS};
use crate::trim::TrimController;
use crate::ui::format::{format_timer, write_dec2};

#[inline(never)]
pub fn update_select(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    storage: &mut RadioStorage,
    trims: &mut TrimController,
    buzzer: &mut Buzzer,
) {
    if keys.cancel {
        ctrl.return_to_main_menu();
        buzzer.click();
        return;
    }

    widgets::navigate_4slot_list(
        &mut ctrl.selected_item,
        &mut ctrl.scroll_offset,
        NUM_MODELS,
        keys.up,
        keys.down,
        buzzer,
    );

    if keys.ok {
        // Save active trims into current model before switching
        storage.active_model.trims = [
            trims.values.roll,
            trims.values.pitch,
            trims.values.throttle,
            trims.values.yaw,
        ];
        storage::save_active_model(storage);

        // Switch active model
        let new_idx = ctrl.selected_item;
        storage.radio.active_model = new_idx as u8;
        storage::save_radio_config(storage);

        // Load new model into active cache
        storage::load_active_model(storage);

        // Load new model's trims
        trims.values.roll = storage.active_model.trims[0];
        trims.values.pitch = storage.active_model.trims[1];
        trims.values.throttle = storage.active_model.trims[2];
        trims.values.yaw = storage.active_model.trims[3];

        // Update RF driver receiver ID, RF power, and receiver settings
        rf::set_rx_id(storage.active_model.rx_id);
        rf::set_rf_power(storage.active_model.rf_power);
        rf::set_rx_settings(
            storage.active_model.servo_rate_hz,
            storage.active_model.rx_out_mode,
            storage.active_model.rx_serial_proto,
        );

        buzzer.play_tone_pattern(2400, 60, 40, 2);
        ctrl.waiting_release = true;
    }

    // Render Header
    widgets::draw_header(lcd, "SELECT MODEL (1-20)");

    for slot in 0..4 {
        let idx = ctrl.scroll_offset + slot;
        if idx >= NUM_MODELS {
            break;
        }
        let is_cursor = idx == ctrl.selected_item;
        let is_active = idx == (storage.radio.active_model as usize);

        let mut line_buf = [b' '; 18];
        line_buf[0] = b'M';
        write_dec2((idx + 1) as u8, &mut line_buf[1..3]);
        line_buf[3] = b':';
        if is_active {
            line_buf[4..14].copy_from_slice(&storage.active_model.name);
        } else if let Some(peeked) = storage::peek_model_name(idx) {
            line_buf[4..14].copy_from_slice(&peeked);
        } else {
            let default_name = ModelConfig::default_for_index(idx).name;
            line_buf[4..14].copy_from_slice(&default_name);
        }
        if is_active {
            line_buf[15..18].copy_from_slice(b"[*]");
        }
        let line_str = ascii_as_str(&line_buf);

        widgets::draw_list_row(lcd, slot, is_cursor, line_str, None, 0);
    }

    widgets::draw_footer(lcd, "[OK] Load     [ESC] Back");
}

#[inline(never)]
fn render_setup_row(
    lcd: &mut St7567,
    slot: usize,
    idx: usize,
    is_sel: bool,
    is_edit: bool,
    sub_idx: usize,
    storage: &RadioStorage,
) {
    let inverted = is_sel || is_edit;
    let y = 14 + (slot as i32 * 9);
    match idx {
        0 => {
            widgets::draw_list_row(
                lcd,
                slot,
                inverted,
                "Name:",
                Some(ascii_as_str(&storage.active_model.name)),
                40,
            );
            if is_edit {
                let char_x = 40 + (sub_idx as i32 * 6);
                lcd.fill_rect(char_x - 1, y, 8, 9, true);
                let single_char = [storage.active_model.name[sub_idx]];
                lcd.draw_str_6x10(char_x, y, ascii_as_str(&single_char), true);
            }
        }
        1 => {
            widgets::draw_list_row(
                lcd,
                slot,
                inverted,
                "Type:",
                Some(storage.active_model.model_type().as_str()),
                40,
            );
        }
        2 => {
            let arm_idx = (storage.active_model.arm_switch as usize)
                .min(crate::ui::format::SWITCH_COND_NAMES.len() - 1);
            let arm_str = if arm_idx == 0 {
                "NONE"
            } else {
                crate::ui::format::SWITCH_COND_NAMES[arm_idx]
            };
            widgets::draw_list_row(lcd, slot, inverted, "Arm Sw:", Some(arm_str), 52);
        }
        3 => {
            widgets::draw_list_row(lcd, slot, inverted, "Timers: [SETUP]", None, 0);
        }
        4 => {
            if is_edit {
                let mut cp_buf = *b"Copy -> M00";
                write_dec2((sub_idx + 1) as u8, &mut cp_buf[9..11]);
                widgets::draw_list_row(lcd, slot, inverted, ascii_as_str(&cp_buf), None, 0);
            } else {
                widgets::draw_list_row(lcd, slot, inverted, "Copy: [OK Duplicate]", None, 0);
            }
        }
        5 => {
            let proto = crate::board::ActiveBoard::resolve_protocol(storage.active_model.rf_protocol);
            widgets::draw_list_row(lcd, slot, inverted, "Protocol:", Some(proto.name()), 64);
        }
        6 => {
            widgets::draw_list_row(lcd, slot, inverted, "Telemetry: [SETUP]", None, 0);
        }
        7 => {
            widgets::draw_list_row(lcd, slot, inverted, "Vario: [SETUP]", None, 0);
        }
        8 => {
            widgets::draw_list_row(
                lcd,
                slot,
                inverted,
                "Soaring:",
                Some(storage.active_model.soaring.discipline_enum().as_str()),
                64,
            );
        }
        9 => {
            widgets::draw_list_row(lcd, slot, inverted, "Reset: [OK Defaults]", None, 0);
        }
        _ => {}
    }
}

#[inline(never)]
pub fn update_setup(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    storage: &mut RadioStorage,
    buzzer: &mut Buzzer,
) {
    let active_idx = storage.radio.active_model as usize;
    const FIELD_COUNT: usize = 10;

    if !ctrl.editing {
        if keys.cancel {
            storage::save_active_model(storage);
            ctrl.return_to_main_menu();
            buzzer.click();
            return;
        }

        widgets::navigate_4slot_list(
            &mut ctrl.selected_item,
            &mut ctrl.scroll_offset,
            FIELD_COUNT,
            keys.up,
            keys.down || keys.bind,
            buzzer,
        );

        if keys.ok {
            match ctrl.selected_item {
                0 => {
                    ctrl.editing = true;
                    ctrl.sub_idx = 0;
                    buzzer.click();
                }
                1 => {
                    let next_type = storage.active_model.model_type().next();
                    storage.active_model.set_model_type(next_type);
                    storage::save_active_model(storage);
                    buzzer.click();
                }
                2 => {
                    // Arm switch edit
                    ctrl.editing = true;
                    buzzer.click();
                }
                3 => {
                    // Open Timers (1..4) submenu
                    ctrl.state = MenuState::TimerSetup;
                    ctrl.selected_item = 0;
                    ctrl.scroll_offset = 0;
                    ctrl.sub_idx = 0; // Timer 1..4 selector (0 = Timer 1)
                    buzzer.click();
                }
                4 => {
                    // Model duplicate/copy edit
                    ctrl.editing = true;
                    ctrl.sub_idx = (active_idx + 1) % NUM_MODELS;
                    buzzer.click();
                }
                5 => {
                    // Open Protocol Setup submenu (RxSetup)
                    ctrl.state = MenuState::RxSetup;
                    ctrl.selected_item = 0;
                    ctrl.scroll_offset = 0;
                    buzzer.click();
                }
                6 => {
                    // Open Telemetry Setup submenu
                    ctrl.state = MenuState::TelemSetup;
                    ctrl.selected_item = 0;
                    ctrl.scroll_offset = 0;
                    buzzer.click();
                }
                7 => {
                    // Open Vario Setup submenu
                    ctrl.state = MenuState::VarioSetup;
                    ctrl.selected_item = 0;
                    ctrl.scroll_offset = 0;
                    buzzer.click();
                }
                8 => {
                    // Open Soaring / F5J Setup submenu
                    ctrl.state = MenuState::SoaringSetup;
                    ctrl.selected_item = 0;
                    ctrl.scroll_offset = 0;
                    buzzer.click();
                }
                9 => {
                    storage.active_model = ModelConfig::default_for_index(active_idx);
                    storage::save_active_model(storage);
                    buzzer.play_tone_pattern(2200, 80, 50, 2);
                    ctrl.waiting_release = true;
                }
                _ => {}
            }
        }
    } else {
        match ctrl.selected_item {
            0 => {
                // Name editing mode
                if keys.cancel {
                    ctrl.editing = false;
                    storage::save_active_model(storage);
                    buzzer.click();
                } else if keys.bind {
                    ctrl.sub_idx = (ctrl.sub_idx + 1) % 10;
                    buzzer.click();
                } else if keys.ok {
                    if ctrl.sub_idx + 1 < 10 {
                        ctrl.sub_idx += 1;
                        buzzer.click();
                    } else {
                        ctrl.editing = false;
                        ctrl.selected_item = 1;
                        storage::save_active_model(storage);
                        buzzer.play_tone(2600, 40);
                    }
                } else if keys.up {
                    let c = storage.active_model.name[ctrl.sub_idx];
                    storage.active_model.name[ctrl.sub_idx] = next_ascii(c);
                    buzzer.play_tone(2200, 20);
                } else if keys.down {
                    let c = storage.active_model.name[ctrl.sub_idx];
                    storage.active_model.name[ctrl.sub_idx] = prev_ascii(c);
                    buzzer.play_tone(2200, 20);
                }
            }
            2 => {
                // Arm switch editing mode (with switch auto-detection)
                if let Some(sw) = keys.sw_change {
                    if storage.is_arm_switch_available(sw) {
                        storage.active_model.arm_switch = sw;
                        buzzer.chime_armed();
                    }
                } else if keys.up {
                    storage.active_model.arm_switch =
                        storage.next_available_arm_switch(storage.active_model.arm_switch);
                    buzzer.play_tone(2200, 20);
                } else if keys.down {
                    storage.active_model.arm_switch =
                        storage.prev_available_arm_switch(storage.active_model.arm_switch);
                    buzzer.play_tone(2200, 20);
                }
                if keys.ok || keys.cancel {
                    ctrl.editing = false;
                    storage::save_active_model(storage);
                    buzzer.click();
                }
            }
            4 => {
                // Model duplicate target selection
                if keys.up {
                    ctrl.sub_idx = (ctrl.sub_idx + 1) % NUM_MODELS;
                    buzzer.play_tone(2200, 20);
                } else if keys.down {
                    ctrl.sub_idx = if ctrl.sub_idx == 0 {
                        NUM_MODELS - 1
                    } else {
                        ctrl.sub_idx - 1
                    };
                    buzzer.play_tone(2200, 20);
                }
                if keys.cancel {
                    ctrl.editing = false;
                    buzzer.click();
                } else if keys.ok {
                    let dst = ctrl.sub_idx.min(NUM_MODELS - 1);
                    storage::save_model_to_slot(dst, &storage.active_model);
                    buzzer.play_tone_pattern(2400, 60, 40, 2);
                    ctrl.editing = false;
                }
            }
            _ => {
                ctrl.editing = false;
            }
        }
    }

    widgets::draw_header(lcd, "MODEL SETUP");

    for slot in 0..4 {
        let idx = ctrl.scroll_offset + slot;
        if idx >= FIELD_COUNT {
            break;
        }
        let is_sel = !ctrl.editing && idx == ctrl.selected_item;
        let is_edit = ctrl.editing && idx == ctrl.selected_item;
        render_setup_row(lcd, slot, idx, is_sel, is_edit, ctrl.sub_idx, storage);
    }

    if ctrl.editing {
        if ctrl.selected_item == 0 {
            widgets::draw_footer(lcd, "[OK] Next Char [ESC] Done");
        } else if ctrl.selected_item == 5 {
            widgets::draw_footer(lcd, "[OK] Duplicate [ESC] Cancel");
        } else {
            widgets::draw_footer(lcd, "[OK] Done   [UP/DN] Value");
        }
    } else {
        widgets::draw_footer(lcd, "[OK] Select    [ESC] Back");
    }
}

#[inline(never)]
pub fn update_vario_setup(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    storage: &mut RadioStorage,
    buzzer: &mut Buzzer,
) {
    const FIELD_COUNT: usize = 5;

    if !ctrl.editing {
        if keys.cancel {
            ctrl.state = MenuState::ModelSetup;
            ctrl.selected_item = 7;
            ctrl.scroll_offset = 4;
            buzzer.click();
            return;
        }

        widgets::navigate_4slot_list(
            &mut ctrl.selected_item,
            &mut ctrl.scroll_offset,
            FIELD_COUNT,
            keys.up,
            keys.down || keys.bind,
            buzzer,
        );

        if keys.ok {
            match ctrl.selected_item {
                0 => {
                    // Edit activation switch
                    ctrl.editing = true;
                    buzzer.click();
                }
                1 => {
                    // Climb deadband: cycle 1..10 (0.1..1.0 m/s)
                    let cur = storage.active_model.vario_climb_deadband;
                    storage.active_model.vario_climb_deadband = if cur >= 10 { 1 } else { cur + 1 };
                    storage::save_active_model(storage);
                    buzzer.click();
                }
                2 => {
                    // Sink deadband: cycle 1..20 (0.1..2.0 m/s)
                    let cur = storage.active_model.vario_sink_deadband;
                    storage.active_model.vario_sink_deadband = if cur >= 20 { 2 } else { cur + 2 };
                    storage::save_active_model(storage);
                    buzzer.click();
                }
                3 => {
                    // Sink mode: cycle Pulse(1) -> Continuous(2) -> Mute(0)
                    let cur = storage.active_model.vario_sink_mode;
                    storage.active_model.vario_sink_mode = (cur + 1) % 3;
                    storage::save_active_model(storage);
                    buzzer.click();
                }
                4 => {
                    // Motor mute toggle
                    storage.active_model.vario_motor_mute =
                        if storage.active_model.vario_motor_mute == 0 {
                            1
                        } else {
                            0
                        };
                    storage::save_active_model(storage);
                    buzzer.click();
                }
                _ => {}
            }
        }
    } else {
        // Switch editing mode: 0: OFF, 1: ON, 2..=29: SWITCH_COND_NAMES[1..=28]
        if let Some(sw) = keys.sw_change {
            if sw > 0 && storage.is_switch_cond_available(sw) {
                storage.active_model.vario_switch = sw + 1;
                buzzer.chime_armed();
            }
        } else if keys.up {
            let cur = storage.active_model.vario_switch;
            let mut cand = (cur + 1) % 30;
            while cand != cur {
                if cand <= 1 || storage.is_switch_cond_available(cand - 1) {
                    break;
                }
                cand = (cand + 1) % 30;
            }
            storage.active_model.vario_switch = cand;
            buzzer.play_tone(2200, 20);
        } else if keys.down {
            let cur = storage.active_model.vario_switch;
            let mut cand = if cur == 0 { 29 } else { cur - 1 };
            while cand != cur {
                if cand <= 1 || storage.is_switch_cond_available(cand - 1) {
                    break;
                }
                cand = if cand == 0 { 29 } else { cand - 1 };
            }
            storage.active_model.vario_switch = cand;
            buzzer.play_tone(2200, 20);
        }
        if keys.ok || keys.cancel {
            ctrl.editing = false;
            storage::save_active_model(storage);
            buzzer.click();
        }
    }

    widgets::draw_header(lcd, "VARIO SETUP");

    for slot in 0..4 {
        let idx = ctrl.scroll_offset + slot;
        if idx >= FIELD_COUNT {
            break;
        }

        match idx {
            0 => {
                let sw_str = match storage.active_model.vario_switch {
                    0 => "OFF",
                    1 => "ON",
                    s if s <= 29 => crate::ui::format::SWITCH_COND_NAMES[(s - 1) as usize],
                    _ => "OFF",
                };
                let is_sel = idx == ctrl.selected_item;
                widgets::draw_list_row_right(lcd, slot, is_sel, "Switch:", Some(sw_str));
            }
            1 => {
                let climb_val = storage.active_model.vario_climb_deadband;
                let mut buf = *b"+0.0m/s";
                buf[1] = b'0' + (climb_val / 10);
                buf[3] = b'0' + (climb_val % 10);
                let s = ascii_as_str(&buf);
                let is_sel = idx == ctrl.selected_item;
                widgets::draw_list_row_right(lcd, slot, is_sel, "Climb Dead:", Some(s));
            }
            2 => {
                let sink_val = storage.active_model.vario_sink_deadband;
                let mut buf = *b"-0.0m/s";
                buf[1] = b'0' + (sink_val / 10);
                buf[3] = b'0' + (sink_val % 10);
                let s = ascii_as_str(&buf);
                let is_sel = idx == ctrl.selected_item;
                widgets::draw_list_row_right(lcd, slot, is_sel, "Sink Dead:", Some(s));
            }
            3 => {
                let mode_str = match storage.active_model.vario_sink_mode {
                    0 => "MUTE",
                    1 => "PULSE",
                    _ => "DRONE",
                };
                let is_sel = idx == ctrl.selected_item;
                widgets::draw_list_row_right(lcd, slot, is_sel, "Sink Mode:", Some(mode_str));
            }
            4 => {
                let mute_str = if storage.active_model.vario_motor_mute != 0 {
                    "ENABLED"
                } else {
                    "DISABLED"
                };
                let is_sel = idx == ctrl.selected_item;
                widgets::draw_list_row_right(lcd, slot, is_sel, "Motor Mute:", Some(mute_str));
            }
            _ => {}
        }
    }

    if ctrl.editing {
        widgets::draw_footer(lcd, "[OK] Done   [UP/DN] Switch");
    } else {
        widgets::draw_footer(lcd, "[OK] Edit/Cycle    [ESC] Back");
    }
}

/// Interactive 4-Timer Configuration Screen.
///
/// Features:
/// - Timer Select: T1, T2, T3, T4
/// - Target duration: 00:00 (Count-Up) .. 60:00 (Countdown) in 30s steps
/// - Mode: Level (Run while active) vs Latch (Start/Stop switches)
/// - Start Sw: OFF, THs, THt, ALWAYS, SA^..SWMv (with live switch flick detection)
/// - Stop Sw: NONE, SA^..SWMv (with live switch flick detection)
/// - Reset Sw: NONE, SA^..SWMv (with live switch flick detection)
/// - Beeps: ON / OFF
/// - [Reset Timer] action
#[inline(never)]
pub fn update_timer_setup(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    storage: &mut RadioStorage,
    buzzer: &mut Buzzer,
) {
    const FIELD_COUNT: usize = 8;
    let timer_idx = ctrl.sub_idx.min(3);
    let mut cfg = storage.active_model.timer_config(timer_idx);

    if keys.cancel {
        if ctrl.editing {
            ctrl.editing = false;
            storage.active_model.set_timer_config(timer_idx, cfg);
            storage::save_active_model(storage);
            buzzer.click();
        } else {
            ctrl.state = MenuState::ModelSetup;
            ctrl.selected_item = 3;
            ctrl.scroll_offset = 0;
            storage::save_active_model(storage);
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
            match ctrl.selected_item {
                0 => {
                    // Timer Selector: cycle T1 -> T2 -> T3 -> T4 -> T1
                    ctrl.sub_idx = (ctrl.sub_idx + 1) % 4;
                    buzzer.click();
                }
                1 => {
                    // Target duration edit
                    ctrl.editing = true;
                    buzzer.click();
                }
                2 => {
                    // Toggle latch mode vs level mode
                    let is_latched = cfg.is_latch_mode();
                    cfg.set_latch_mode(!is_latched);
                    storage.active_model.set_timer_config(timer_idx, cfg);
                    storage::save_active_model(storage);
                    buzzer.click();
                }
                3 => {
                    // Start Switch edit
                    ctrl.editing = true;
                    buzzer.click();
                }
                4 => {
                    // Stop Switch edit
                    ctrl.editing = true;
                    buzzer.click();
                }
                5 => {
                    // Reset Switch edit
                    ctrl.editing = true;
                    buzzer.click();
                }
                6 => {
                    // Toggle audio beeps
                    let beeps = cfg.has_audio_beeps();
                    cfg.set_audio_beeps(!beeps);
                    storage.active_model.set_timer_config(timer_idx, cfg);
                    storage::save_active_model(storage);
                    buzzer.click();
                }
                7 => {
                    // Manual reset action
                    buzzer.play_tone(2400, 100);
                    ctrl.waiting_release = true;
                }
                _ => {}
            }
        }
    } else {
        match ctrl.selected_item {
            1 => {
                // Target duration edit in 30-sec increments
                if keys.up && cfg.secs <= 3570 {
                    cfg.secs += 30;
                    storage.active_model.set_timer_config(timer_idx, cfg);
                    buzzer.play_tone(2200, 20);
                } else if keys.down && cfg.secs >= 30 {
                    cfg.secs -= 30;
                    storage.active_model.set_timer_config(timer_idx, cfg);
                    buzzer.play_tone(2200, 20);
                }
                if keys.ok {
                    ctrl.editing = false;
                    storage::save_active_model(storage);
                    buzzer.click();
                }
            }
            3 => {
                // Start switch selection with switch flick detection
                if let Some(sw) = keys.sw_change {
                    let ts = sw + 3; // 1..28 maps to 4..31
                    cfg.start_src = ts;
                    storage.active_model.set_timer_config(timer_idx, cfg);
                    buzzer.play_tone(2400, 40);
                } else if keys.up {
                    cfg.start_src = (cfg.start_src + 1) % 32;
                    storage.active_model.set_timer_config(timer_idx, cfg);
                    buzzer.play_tone(2200, 20);
                } else if keys.down {
                    cfg.start_src = if cfg.start_src == 0 {
                        31
                    } else {
                        cfg.start_src - 1
                    };
                    storage.active_model.set_timer_config(timer_idx, cfg);
                    buzzer.play_tone(2200, 20);
                }
                if keys.ok {
                    ctrl.editing = false;
                    storage::save_active_model(storage);
                    buzzer.click();
                }
            }
            4 => {
                // Stop switch selection with switch flick detection
                if let Some(sw) = keys.sw_change {
                    cfg.stop_src = sw;
                    storage.active_model.set_timer_config(timer_idx, cfg);
                    buzzer.play_tone(2400, 40);
                } else if keys.up {
                    cfg.stop_src = (cfg.stop_src + 1) % 29;
                    storage.active_model.set_timer_config(timer_idx, cfg);
                    buzzer.play_tone(2200, 20);
                } else if keys.down {
                    cfg.stop_src = if cfg.stop_src == 0 {
                        28
                    } else {
                        cfg.stop_src - 1
                    };
                    storage.active_model.set_timer_config(timer_idx, cfg);
                    buzzer.play_tone(2200, 20);
                }
                if keys.ok {
                    ctrl.editing = false;
                    storage::save_active_model(storage);
                    buzzer.click();
                }
            }
            5 => {
                // Reset switch selection with switch flick detection
                if let Some(sw) = keys.sw_change {
                    cfg.reset_src = sw;
                    storage.active_model.set_timer_config(timer_idx, cfg);
                    buzzer.play_tone(2400, 40);
                } else if keys.up {
                    cfg.reset_src = (cfg.reset_src + 1) % 29;
                    storage.active_model.set_timer_config(timer_idx, cfg);
                    buzzer.play_tone(2200, 20);
                } else if keys.down {
                    cfg.reset_src = if cfg.reset_src == 0 {
                        28
                    } else {
                        cfg.reset_src - 1
                    };
                    storage.active_model.set_timer_config(timer_idx, cfg);
                    buzzer.play_tone(2200, 20);
                }
                if keys.ok {
                    ctrl.editing = false;
                    storage::save_active_model(storage);
                    buzzer.click();
                }
            }
            _ => {
                ctrl.editing = false;
            }
        }
    }

    let mut header_buf = *b"TIMER 1 SETUP   ";
    header_buf[6] = b'1' + timer_idx as u8;
    widgets::draw_header(lcd, ascii_as_str(&header_buf));

    for slot in 0..4 {
        let idx = ctrl.scroll_offset + slot;
        if idx >= FIELD_COUNT {
            break;
        }

        let is_sel = idx == ctrl.selected_item;
        match idx {
            0 => {
                let mut sel_buf = *b"TIMER 1";
                sel_buf[6] = b'1' + timer_idx as u8;
                widgets::draw_list_row_right(
                    lcd,
                    slot,
                    is_sel,
                    "Timer:",
                    Some(ascii_as_str(&sel_buf)),
                );
            }
            1 => {
                let mut t_buf = [0u8; 8];
                let t_str = if cfg.secs == 0 {
                    "00:00 (UP)"
                } else {
                    format_timer(cfg.secs, false, &mut t_buf)
                };
                widgets::draw_list_row_right(lcd, slot, is_sel, "Target:", Some(t_str));
            }
            2 => {
                let mode_str = if cfg.is_latch_mode() {
                    "LATCH"
                } else {
                    "LEVEL"
                };
                widgets::draw_list_row_right(lcd, slot, is_sel, "Mode:", Some(mode_str));
            }
            3 => {
                let trig_str = match cfg.start_src {
                    0 => "OFF",
                    1 => "THs (RUN)",
                    2 => "THt (LTCH)",
                    3 => "ALWAYS",
                    s if (4..=31).contains(&s) => {
                        crate::ui::format::SWITCH_COND_NAMES[(s - 3) as usize]
                    }
                    _ => "OFF",
                };
                widgets::draw_list_row_right(lcd, slot, is_sel, "Start:", Some(trig_str));
            }
            4 => {
                let stop_str = if cfg.stop_src == 0 {
                    "NONE"
                } else {
                    crate::ui::format::SWITCH_COND_NAMES[cfg.stop_src as usize]
                };
                widgets::draw_list_row_right(lcd, slot, is_sel, "Stop:", Some(stop_str));
            }
            5 => {
                let rst_str = if cfg.reset_src == 0 {
                    "NONE"
                } else {
                    crate::ui::format::SWITCH_COND_NAMES[cfg.reset_src as usize]
                };
                widgets::draw_list_row_right(lcd, slot, is_sel, "Reset:", Some(rst_str));
            }
            6 => {
                let beep_str = if cfg.has_audio_beeps() { "ON" } else { "OFF" };
                widgets::draw_list_row_right(lcd, slot, is_sel, "Beeps:", Some(beep_str));
            }
            7 => {
                widgets::draw_list_row_right(lcd, slot, is_sel, "Action:", Some("[Reset]"));
            }
            _ => {}
        }
    }

    if ctrl.editing {
        widgets::draw_footer(lcd, "[OK] Save   [UP/DN] Value");
    } else {
        widgets::draw_footer(lcd, "[OK] Edit/Cycle    [ESC] Back");
    }
}
