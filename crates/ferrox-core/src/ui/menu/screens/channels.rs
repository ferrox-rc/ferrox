//! Auxiliary channels assignment, Channel reversing, and Channel monitor screens.

use crate::board::BoardProfile;
use crate::buzzer::Buzzer;
use crate::display::St7567;
use crate::menu::format::{ascii_as_str, SOURCE_NAMES};
use crate::menu::widgets;
use crate::menu::{MenuController, NavKeys};
use crate::mixer::{CHANNEL_MAX_US, CHANNEL_MIN_US, CHANNEL_SPAN_US, NUM_CHANNELS};
use crate::storage::{self, RadioStorage};

const CH_NAMES: [&str; 18] = [
    "CH1 (AIL)",
    "CH2 (ELE)",
    "CH3 (THR)",
    "CH4 (RUD)",
    "CH5 (SA) ",
    "CH6 (SB) ",
    "CH7 (VR1)",
    "CH8 (VR2)",
    "CH9 (SC) ",
    "CH10(SD) ",
    "CH11     ",
    "CH12     ",
    "CH13     ",
    "CH14     ",
    "CH15     ",
    "CH16     ",
    "CH17     ",
    "CH18     ",
];

#[inline(never)]
pub fn update_aux_channels(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    storage: &mut RadioStorage,
    buzzer: &mut Buzzer,
) {
    let is_freeform = storage.active_model.wing_tail_mix == 4;
    let is_tank = crate::mixer::is_tank(&storage.active_model);
    let mut channels = [0; 18];
    let mut ch_count = 0;
    for ch in (if is_freeform || is_tank { 0 } else { 4 })..18 {
        if !crate::mixer::is_tank_motor(ch, &storage.active_model) {
            channels[ch_count] = ch;
            ch_count += 1;
        }
    }

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

    if !ctrl.editing {
        widgets::navigate_4slot_list(
            &mut ctrl.selected_item,
            &mut ctrl.scroll_offset,
            ch_count,
            keys.up,
            keys.down,
            buzzer,
        );

        if keys.ok {
            ctrl.editing = true;
            buzzer.click();
        }
    } else {
        let ch_idx = channels[ctrl.selected_item];
        let aux_sources = crate::board::ActiveBoard::AUX_SOURCES;
        if let Some(pot) = keys.pot_change {
            storage.active_model.set_channel_source(ch_idx, pot);
            buzzer.play_tone(2400, 40);
        } else {
            let cur_val = storage.active_model.channel_source(ch_idx);
            if keys.up {
                let next_val = storage.next_available_aux_source(cur_val, aux_sources);
                storage.active_model.set_channel_source(ch_idx, next_val);
                buzzer.play_tone(2200, 20);
            } else if keys.down {
                let next_val = storage.prev_available_aux_source(cur_val, aux_sources);
                storage.active_model.set_channel_source(ch_idx, next_val);
                buzzer.play_tone(2200, 20);
            }
        }
        if keys.ok {
            ctrl.editing = false;
            buzzer.click();
        }
    }

    let header_title = if is_freeform {
        "CHANNELS (1..18)"
    } else {
        "CHANNELS"
    };
    widgets::draw_header(lcd, header_title);

    for slot in 0..4 {
        let list_idx = ctrl.scroll_offset + slot;
        if list_idx >= ch_count {
            break;
        }
        let is_sel = list_idx == ctrl.selected_item;

        let ch_idx = channels[list_idx];
        let ch_num = 1 + ch_idx;
        let mut ch_buf = *b"CH00: ";
        if ch_num >= 10 {
            ch_buf[2] = b'0' + (ch_num / 10) as u8;
            ch_buf[3] = b'0' + (ch_num % 10) as u8;
        } else {
            ch_buf[2] = b'0' + ch_num as u8;
            ch_buf[3] = b' ';
        }
        let ch_label = ascii_as_str(&ch_buf);

        let src_val = storage.active_model.channel_source(ch_idx);
        let src_idx = (src_val as usize).min(SOURCE_NAMES.len() - 1);
        widgets::draw_list_row(lcd, slot, is_sel, ch_label, Some(SOURCE_NAMES[src_idx]), 48);
    }

    let footer = if ctrl.editing {
        "[OK] Done   [UP/DN] Source"
    } else {
        "[OK] Edit   [ESC] Exit"
    };
    widgets::draw_footer(lcd, footer);
}

#[inline(never)]
pub fn update_channel_reverse(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    storage: &mut RadioStorage,
    buzzer: &mut Buzzer,
) {
    const CH_COUNT: usize = 18;

    if keys.cancel {
        storage::save_active_model(storage);
        ctrl.return_to_main_menu();
        buzzer.click();
        return;
    }

    widgets::navigate_4slot_list(
        &mut ctrl.selected_item,
        &mut ctrl.scroll_offset,
        CH_COUNT,
        keys.up,
        keys.down,
        buzzer,
    );

    if keys.ok {
        storage.active_model.channel_reverse ^= 1 << ctrl.selected_item;
        buzzer.click();
    }

    widgets::draw_header(lcd, "CHANNEL REVERSE");

    for slot in 0..4 {
        let ch = ctrl.scroll_offset + slot;
        if ch >= CH_COUNT {
            break;
        }
        let is_sel = ch == ctrl.selected_item;
        let is_rev = (storage.active_model.channel_reverse & (1 << ch)) != 0;
        let status_str = if is_rev { "REVERSE" } else { "NORMAL " };

        widgets::draw_list_row(lcd, slot, is_sel, CH_NAMES[ch], Some(status_str), 76);
    }

    widgets::draw_footer(lcd, "[OK] Toggle   [ESC] Back");
}

/// Format dynamic channel label into a fixed buffer, e.g. "1:AIL", "2:L.ELV", "6:R.AIL", "10:VR1", "18:None".
pub fn format_channel_label<'a>(
    ch: usize,
    model: &crate::storage::ModelConfig,
    buf: &'a mut [u8; 10],
) -> &'a str {
    let mut len = 0;
    let ch_num = ch + 1;
    if ch_num >= 10 {
        buf[len] = b'0' + (ch_num / 10) as u8;
        len += 1;
        buf[len] = b'0' + (ch_num % 10) as u8;
        len += 1;
    } else {
        buf[len] = b'0' + ch_num as u8;
        len += 1;
    }
    buf[len] = b':';
    len += 1;

    let name = match ch {
        0 => match model.wing_tail_mix {
            1 => "L.ELV",
            3 => "L.AIL",
            4 | 6 => {
                let direct_src = model.freeform_ch1_4[0] as usize;
                if direct_src != 0 && direct_src < SOURCE_NAMES.len() {
                    SOURCE_NAMES[direct_src]
                } else if let Some(m) = model.mixes.iter().find(|m| m.target_ch == 1) {
                    let src = m.source as usize;
                    if src < SOURCE_NAMES.len() {
                        SOURCE_NAMES[src]
                    } else {
                        "MIX"
                    }
                } else {
                    "None"
                }
            }
            _ => "AIL",
        },
        1 => match model.wing_tail_mix {
            1 => "R.ELV",
            2 => "L.VTL",
            4 | 6 => {
                let direct_src = model.freeform_ch1_4[1] as usize;
                if direct_src != 0 && direct_src < SOURCE_NAMES.len() {
                    SOURCE_NAMES[direct_src]
                } else if let Some(m) = model.mixes.iter().find(|m| m.target_ch == 2) {
                    let src = m.source as usize;
                    if src < SOURCE_NAMES.len() {
                        SOURCE_NAMES[src]
                    } else {
                        "MIX"
                    }
                } else {
                    "None"
                }
            }
            _ => "ELE",
        },
        2 => match model.wing_tail_mix {
            4 | 6 => {
                let direct_src = model.freeform_ch1_4[2] as usize;
                if direct_src != 0 && direct_src < SOURCE_NAMES.len() {
                    SOURCE_NAMES[direct_src]
                } else if let Some(m) = model.mixes.iter().find(|m| m.target_ch == 3) {
                    let src = m.source as usize;
                    if src < SOURCE_NAMES.len() {
                        SOURCE_NAMES[src]
                    } else {
                        "MIX"
                    }
                } else {
                    "None"
                }
            }
            _ => "THR",
        },
        3 => match model.wing_tail_mix {
            2 => "R.VTL",
            4 | 6 => {
                let direct_src = model.freeform_ch1_4[3] as usize;
                if direct_src != 0 && direct_src < SOURCE_NAMES.len() {
                    SOURCE_NAMES[direct_src]
                } else if let Some(m) = model.mixes.iter().find(|m| m.target_ch == 4) {
                    let src = m.source as usize;
                    if src < SOURCE_NAMES.len() {
                        SOURCE_NAMES[src]
                    } else {
                        "MIX"
                    }
                } else {
                    "None"
                }
            }
            _ => "RUD",
        },
        5 if model.wing_tail_mix == 3 => "R.AIL",
        4..=17 => {
            let aux_src = model.aux_channels[ch - 4] as usize;
            if aux_src < SOURCE_NAMES.len() {
                SOURCE_NAMES[aux_src]
            } else {
                "None"
            }
        }
        _ => "CH",
    };
    let name = if crate::mixer::is_tank(model) && ch_num == model.tank.left_channel as usize {
        "L.MOT"
    } else if crate::mixer::is_tank(model) && ch_num == model.tank.right_channel as usize {
        "R.MOT"
    } else {
        name
    };

    for &b in name.as_bytes() {
        if len < buf.len() {
            buf[len] = b;
            len += 1;
        }
    }

    ascii_as_str(&buf[..len])
}

#[inline(never)]
pub fn update_channel_monitor(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    storage: &RadioStorage,
    rf_chs: &[u16; NUM_CHANNELS],
    buzzer: &mut Buzzer,
) {
    if keys.cancel {
        ctrl.return_to_main_menu();
        buzzer.click();
        return;
    }

    if keys.up {
        ctrl.page_idx = (ctrl.page_idx + 1) % 3;
        buzzer.play_tone(2200, 30);
    } else if keys.down {
        ctrl.page_idx = if ctrl.page_idx == 0 {
            2
        } else {
            ctrl.page_idx - 1
        };
        buzzer.play_tone(2200, 30);
    }

    let title = match ctrl.page_idx {
        0 => "CHANNELS (1-6)",
        1 => "CHANNELS (7-12)",
        _ => "CHANNELS (13-18)",
    };
    widgets::draw_header(lcd, title);

    let start_ch: usize = match ctrl.page_idx {
        0 => 0,
        1 => 6,
        _ => 12,
    };

    let active_model = storage.active_model();

    const MONITOR_TOP_Y: i32 = 13;
    const MONITOR_ROW_STRIDE: i32 = 7;

    for i in 0..6 {
        let ch = start_ch + i;
        let y = MONITOR_TOP_Y + (i as i32 * MONITOR_ROW_STRIDE);

        let mut name_buf = [0u8; 10];
        let name = format_channel_label(ch, active_model, &mut name_buf);

        let us = rf_chs[ch].clamp(CHANNEL_MIN_US, CHANNEL_MAX_US);
        let fill_w = (((us - CHANNEL_MIN_US) as u32 * 38) / CHANNEL_SPAN_US).min(38);
        widgets::draw_gauge_row(lcd, y, name, fill_w, us);
    }

    widgets::draw_footer(lcd, "[UP/DN] Page  [ESC] Back");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::ModelConfig;

    #[test]
    fn test_format_channel_label_normal_and_aux() {
        let mut model = ModelConfig::default_for_index(0);
        model.wing_tail_mix = 0; // Normal
        model.aux_channels[0] = 7; // CH5 -> SA
        model.aux_channels[1] = 8; // CH6 -> SB
        model.aux_channels[5] = 10; // CH10 -> SD
        model.aux_channels[13] = 0; // CH18 -> None

        let mut buf = [0u8; 10];
        assert_eq!(format_channel_label(0, &model, &mut buf), "1:AIL");
        assert_eq!(format_channel_label(1, &model, &mut buf), "2:ELE");
        assert_eq!(format_channel_label(2, &model, &mut buf), "3:THR");
        assert_eq!(format_channel_label(3, &model, &mut buf), "4:RUD");
        assert_eq!(format_channel_label(4, &model, &mut buf), "5:SA");
        assert_eq!(format_channel_label(5, &model, &mut buf), "6:SB");
        assert_eq!(format_channel_label(9, &model, &mut buf), "10:SD");
        assert_eq!(format_channel_label(17, &model, &mut buf), "18:None");
    }

    #[test]
    fn test_format_channel_label_tank_primary_and_aux_motors() {
        let mut model = crate::storage::ModelConfig::default_for_index(0);
        model.wing_tail_mix = crate::mixer::WingTailTemplate::Tank as u8;
        let mut buf = [0; 10];
        assert_eq!(format_channel_label(0, &model, &mut buf), "1:L.MOT");
        assert_eq!(format_channel_label(1, &model, &mut buf), "2:R.MOT");
        model.tank.left_channel = 17;
        model.tank.right_channel = 18;
        assert_eq!(format_channel_label(16, &model, &mut buf), "17:L.MOT");
        assert_eq!(format_channel_label(17, &model, &mut buf), "18:R.MOT");
        assert_eq!(format_channel_label(0, &model, &mut buf), "1:None");
        model.set_channel_source(0, 7);
        assert_eq!(format_channel_label(0, &model, &mut buf), "1:SA");
    }

    #[test]
    fn tank_channel_assignments_skip_only_motor_outputs() {
        for (left, right) in [(1, 2), (18, 3), (17, 18)] {
            let mut storage = RadioStorage::default_factory();
            storage.active_model.wing_tail_mix = crate::mixer::WingTailTemplate::Tank as u8;
            storage.active_model.tank.left_channel = left;
            storage.active_model.tank.right_channel = right;
            storage.active_model.freeform_ch1_4.fill(0);
            storage.active_model.aux_channels.fill(0);
            let mut ctrl = MenuController::new();
            let mut lcd = St7567::new();
            let mut buzzer = Buzzer::new();
            let mut keys = NavKeys {
                ok: false,
                cancel: false,
                up: false,
                down: false,
                bind: false,
                sw_change: None,
                dr_change: None,
                pot_change: None,
            };
            for ch in 1..=18 {
                if ch == left || ch == right {
                    continue;
                }
                assert!(!storage.is_channel_configured(ch));
                keys.down = false;
                keys.ok = true;
                update_aux_channels(&mut ctrl, &mut lcd, &keys, &mut storage, &mut buzzer);
                keys.ok = false;
                keys.pot_change = Some(5);
                update_aux_channels(&mut ctrl, &mut lcd, &keys, &mut storage, &mut buzzer);
                keys.pot_change = None;
                assert_eq!(storage.active_model.channel_source(ch as usize - 1), 5);
                assert!(storage.is_channel_configured(ch));
                keys.ok = true;
                update_aux_channels(&mut ctrl, &mut lcd, &keys, &mut storage, &mut buzzer);
                keys.ok = false;
                keys.down = true;
                update_aux_channels(&mut ctrl, &mut lcd, &keys, &mut storage, &mut buzzer);
            }
            assert_eq!(ctrl.selected_item, 0);
            for ch in [left, right] {
                assert_eq!(storage.active_model.channel_source(ch as usize - 1), 0);
                assert!(storage.is_channel_configured(ch));
            }
        }
    }

    #[test]
    fn test_format_channel_label_wing_tail_templates() {
        let mut model = ModelConfig::default_for_index(0);
        let mut buf = [0u8; 10];

        // Elevon
        model.wing_tail_mix = 1;
        assert_eq!(format_channel_label(0, &model, &mut buf), "1:L.ELV");
        assert_eq!(format_channel_label(1, &model, &mut buf), "2:R.ELV");
        assert_eq!(format_channel_label(2, &model, &mut buf), "3:THR");
        assert_eq!(format_channel_label(3, &model, &mut buf), "4:RUD");

        // V-Tail
        model.wing_tail_mix = 2;
        assert_eq!(format_channel_label(0, &model, &mut buf), "1:AIL");
        assert_eq!(format_channel_label(1, &model, &mut buf), "2:L.VTL");
        assert_eq!(format_channel_label(2, &model, &mut buf), "3:THR");
        assert_eq!(format_channel_label(3, &model, &mut buf), "4:R.VTL");

        // Flaperon
        model.wing_tail_mix = 3;
        assert_eq!(format_channel_label(0, &model, &mut buf), "1:L.AIL");
        assert_eq!(format_channel_label(1, &model, &mut buf), "2:ELE");
        assert_eq!(format_channel_label(2, &model, &mut buf), "3:THR");
        assert_eq!(format_channel_label(3, &model, &mut buf), "4:RUD");
        assert_eq!(format_channel_label(5, &model, &mut buf), "6:R.AIL");

        // Freeform (released gimbals)
        model.wing_tail_mix = 4;
        assert_eq!(format_channel_label(0, &model, &mut buf), "1:None");
        assert_eq!(format_channel_label(1, &model, &mut buf), "2:None");
        assert_eq!(format_channel_label(2, &model, &mut buf), "3:None");
        assert_eq!(format_channel_label(3, &model, &mut buf), "4:None");

        // Freeform with mix line driving CH1 from Throttle (source 3)
        model.mixes[0].target_ch = 1;
        model.mixes[0].source = 3;
        assert_eq!(format_channel_label(0, &model, &mut buf), "1:Thr");

        // Freeform with direct source assigned to CH1 (e.g. SA = source 7)
        model.freeform_ch1_4[0] = 7;
        assert_eq!(format_channel_label(0, &model, &mut buf), "1:SA");
    }

    #[test]
    fn channel_assignment_can_select_curved_throttle() {
        let mut storage = RadioStorage::default_factory();
        storage.active_model.aux_channels[0] = 3;
        let mut ctrl = MenuController::new();
        ctrl.editing = true;
        let mut lcd = St7567::new();
        let mut buzzer = Buzzer::new();
        let keys = NavKeys {
            ok: false,
            cancel: false,
            up: true,
            down: false,
            bind: false,
            sw_change: None,
            dr_change: None,
            pot_change: None,
        };
        update_aux_channels(&mut ctrl, &mut lcd, &keys, &mut storage, &mut buzzer);
        assert_eq!(storage.active_model.aux_channels[0], 58);
    }

    #[test]
    fn test_aux_channels_screen_freeform_vs_normal() {
        let mut ctrl = MenuController::new();
        let mut buzzer = Buzzer::new();
        let mut lcd = St7567::new();
        let mut storage = RadioStorage::default_factory();

        // 1. Normal mode: 14 channels (CH5..CH18)
        storage.active_model.wing_tail_mix = 0;
        let keys_down = NavKeys {
            ok: false,
            cancel: false,
            up: false,
            down: true,
            bind: false,
            sw_change: None,
            dr_change: None,
            pot_change: None,
        };

        // Scroll to item 13 (the last item in normal 14-item list)
        for _ in 0..13 {
            update_aux_channels(&mut ctrl, &mut lcd, &keys_down, &mut storage, &mut buzzer);
        }
        assert_eq!(ctrl.selected_item, 13);
        // Next down wraps to 0
        update_aux_channels(&mut ctrl, &mut lcd, &keys_down, &mut storage, &mut buzzer);
        assert_eq!(ctrl.selected_item, 0);

        // 2. Freeform mode: 18 channels (CH1..CH18)
        storage.active_model.wing_tail_mix = 4;
        for _ in 0..17 {
            update_aux_channels(&mut ctrl, &mut lcd, &keys_down, &mut storage, &mut buzzer);
        }
        assert_eq!(ctrl.selected_item, 17);
        // Next down wraps to 0
        update_aux_channels(&mut ctrl, &mut lcd, &keys_down, &mut storage, &mut buzzer);
        assert_eq!(ctrl.selected_item, 0);

        // Test editing CH1 (item 0) directly in Freeform mode
        let keys_ok = NavKeys {
            ok: true,
            cancel: false,
            up: false,
            down: false,
            bind: false,
            sw_change: None,
            dr_change: None,
            pot_change: None,
        };
        update_aux_channels(&mut ctrl, &mut lcd, &keys_ok, &mut storage, &mut buzzer);
        assert!(ctrl.editing);

        let keys_up = NavKeys {
            ok: false,
            cancel: false,
            up: true,
            down: false,
            bind: false,
            sw_change: None,
            dr_change: None,
            pot_change: None,
        };
        let expected_next =
            storage.next_available_aux_source(0, crate::board::ActiveBoard::AUX_SOURCES);
        update_aux_channels(&mut ctrl, &mut lcd, &keys_up, &mut storage, &mut buzzer);
        assert_eq!(storage.active_model.freeform_ch1_4[0], expected_next);
    }
}
