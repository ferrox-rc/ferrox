//! Soaring & F5J Competition Settings Screen.

use crate::buzzer::Buzzer;
use crate::display::St7567;
use crate::menu::format::ascii_as_str;
use crate::menu::widgets;
use crate::menu::{MenuController, MenuState, NavKeys};
use crate::soaring::SoaringDiscipline;
use crate::storage::{self, RadioStorage};

fn fmt_ch(ch_idx: u8, buf: &mut [u8; 6]) -> &str {
    let ch_num = ch_idx + 1;
    buf[0] = b'C';
    buf[1] = b'H';
    if ch_num >= 10 {
        buf[2] = b'0' + (ch_num / 10);
        buf[3] = b'0' + (ch_num % 10);
        ascii_as_str(&buf[..4])
    } else {
        buf[2] = b'0' + ch_num;
        ascii_as_str(&buf[..3])
    }
}

fn fmt_u8_unit(val: u8, unit: u8, buf: &mut [u8; 6]) -> &str {
    let mut p = 0;
    if val >= 100 {
        buf[p] = b'0' + (val / 100);
        p += 1;
    }
    if val >= 10 {
        buf[p] = b'0' + ((val / 10) % 10);
        p += 1;
    }
    buf[p] = b'0' + (val % 10);
    p += 1;
    buf[p] = unit;
    p += 1;
    ascii_as_str(&buf[..p])
}

fn fmt_i8_pct(val: i8, buf: &mut [u8; 6]) -> &str {
    let mut p = 0;
    let abs = if val < 0 {
        buf[p] = b'-';
        p += 1;
        val.unsigned_abs()
    } else {
        val as u8
    };
    if abs >= 100 {
        buf[p] = b'0' + (abs / 100);
        p += 1;
    }
    if abs >= 10 {
        buf[p] = b'0' + ((abs / 10) % 10);
        p += 1;
    }
    buf[p] = b'0' + (abs % 10);
    p += 1;
    buf[p] = b'%';
    p += 1;
    ascii_as_str(&buf[..p])
}

#[inline(never)]
pub fn update_soaring_setup(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    storage: &mut RadioStorage,
    buzzer: &mut Buzzer,
) {
    let disc = storage.active_model.soaring.discipline_enum();
    let item_count = if disc == SoaringDiscipline::Off { 1 } else { 9 };

    if !ctrl.editing {
        if keys.cancel {
            storage::save_active_model(storage);
            ctrl.state = MenuState::ModelSetup;
            ctrl.selected_item = 8;
            ctrl.scroll_offset = 5;
            buzzer.click();
            return;
        }

        widgets::navigate_4slot_list(
            &mut ctrl.selected_item,
            &mut ctrl.scroll_offset,
            item_count,
            keys.up,
            keys.down || keys.bind,
            buzzer,
        );

        if keys.ok {
            match ctrl.selected_item {
                0 => {
                    let next = disc.next();
                    storage.active_model.soaring.discipline = next as u8;
                    if next == SoaringDiscipline::Off {
                        ctrl.selected_item = 0;
                        ctrl.scroll_offset = 0;
                    }
                    storage::save_active_model(storage);
                    buzzer.click();
                }
                8 => {
                    storage.active_model.soaring.flags ^=
                        crate::soaring::SoaringConfig::FLAG_AUDIO_CUES;
                    storage::save_active_model(storage);
                    buzzer.click();
                }
                1..=7 => {
                    ctrl.editing = true;
                    buzzer.click();
                }
                _ => {}
            }
        }
    } else if keys.ok || keys.cancel {
        ctrl.editing = false;
        storage::save_active_model(storage);
        buzzer.click();
    } else {
        let dir: i16 = if keys.up {
            1
        } else if keys.down || keys.bind {
            -1
        } else {
            0
        };
        if dir != 0 {
            let s = &mut storage.active_model.soaring;
            match ctrl.selected_item {
                1 => {
                    let max_ch = (crate::mixer::NUM_CHANNELS - 1) as i16;
                    s.motor_ch = ((s.motor_ch as i16 + dir).clamp(0, max_ch)) as u8;
                }
                2 => {
                    s.target_time_mins = ((s.target_time_mins as i16 + dir).clamp(1, 99)) as u8;
                }
                3 => {
                    s.motor_on_thr_pct =
                        ((s.motor_on_thr_pct as i16 + dir * 5).clamp(-90, 90)) as i8;
                    if s.motor_off_thr_pct >= s.motor_on_thr_pct {
                        s.motor_off_thr_pct = (s.motor_on_thr_pct - 5).max(-100);
                    }
                }
                4 => {
                    let max_off = (s.motor_on_thr_pct - 5).max(-100) as i16;
                    s.motor_off_thr_pct =
                        ((s.motor_off_thr_pct as i16 + dir * 5).clamp(-100, max_off)) as i8;
                }
                5 => {
                    s.min_climb_m = ((s.min_climb_m as i16 + dir).clamp(5, 100)) as u8;
                }
                6 => {
                    s.land_alt_max_m = ((s.land_alt_max_m as i16 + dir).clamp(1, 20)) as u8;
                }
                7 => {
                    s.confirm_secs = ((s.confirm_secs as i16 + dir).clamp(1, 15)) as u8;
                }
                _ => {}
            }
            buzzer.click();
        }
    }

    widgets::draw_header(lcd, "SOARING / F5J SETUP");

    let s = &storage.active_model.soaring;
    for slot in 0..4 {
        let idx = ctrl.scroll_offset + slot;
        if idx >= item_count {
            break;
        }
        let is_sel = idx == ctrl.selected_item;
        let is_edit = is_sel && ctrl.editing;
        let inverted = is_sel || is_edit;

        let mut buf = [0u8; 6];
        match idx {
            0 => {
                widgets::draw_list_row(
                    lcd,
                    slot,
                    inverted,
                    "Mode:",
                    Some(s.discipline_enum().as_str()),
                    68,
                );
            }
            1 => {
                let val_str = fmt_ch(s.motor_ch, &mut buf);
                widgets::draw_list_row(lcd, slot, inverted, "Motor Ch:", Some(val_str), 68);
            }
            2 => {
                let val_str = fmt_u8_unit(s.target_time_mins, b'm', &mut buf);
                widgets::draw_list_row(lcd, slot, inverted, "Work Time:", Some(val_str), 68);
            }
            3 => {
                let val_str = fmt_i8_pct(s.motor_on_thr_pct, &mut buf);
                widgets::draw_list_row(lcd, slot, inverted, "On Thr:", Some(val_str), 68);
            }
            4 => {
                let val_str = fmt_i8_pct(s.motor_off_thr_pct, &mut buf);
                widgets::draw_list_row(lcd, slot, inverted, "Off Thr:", Some(val_str), 68);
            }
            5 => {
                let val_str = fmt_u8_unit(s.min_climb_m, b'm', &mut buf);
                widgets::draw_list_row(lcd, slot, inverted, "Min Climb:", Some(val_str), 68);
            }
            6 => {
                let val_str = fmt_u8_unit(s.land_alt_max_m, b'm', &mut buf);
                widgets::draw_list_row(lcd, slot, inverted, "Land Alt:", Some(val_str), 68);
            }
            7 => {
                let val_str = fmt_u8_unit(s.confirm_secs, b's', &mut buf);
                widgets::draw_list_row(lcd, slot, inverted, "Hold Secs:", Some(val_str), 68);
            }
            8 => {
                let val_str = if s.audio_cues_enabled() { "ON" } else { "OFF" };
                widgets::draw_list_row(lcd, slot, inverted, "Audio Cues:", Some(val_str), 68);
            }
            _ => {}
        }
    }

    if ctrl.editing {
        widgets::draw_footer(lcd, "[UP/DN] Val   [OK] Done");
    } else {
        widgets::draw_footer(lcd, "[OK] Edit    [ESC] Back");
    }
}
