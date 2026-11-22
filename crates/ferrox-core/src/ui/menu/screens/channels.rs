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
    let ch_count = NUM_CHANNELS;

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
        let ch_idx = ctrl.selected_item;
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

    widgets::draw_header(lcd, "CHANNELS (1..18)");

    for slot in 0..4 {
        let list_idx = ctrl.scroll_offset + slot;
        if list_idx >= ch_count {
            break;
        }
        let is_sel = list_idx == ctrl.selected_item;

        let ch_idx = list_idx;
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

    let src = if ch < NUM_CHANNELS {
        model.channels[ch].source
    } else {
        0
    };
    let name: &str = match src {
        60..=71 => {
            let inst = ((src - 60) / 3) as usize;
            let out_idx = ((src - 60) % 3) as usize;
            let kind = crate::mixer::MixerKind::from_u8(model.mixers[inst].kind);
            kind.outputs().get(out_idx).copied().unwrap_or("MIX")
        }
        58 => "THC",
        s if (s as usize) < SOURCE_NAMES.len() && s != 0 => SOURCE_NAMES[s as usize],
        _ => {
            if let Some(m) = model.mixes.iter().find(|m| m.target_ch == (ch + 1) as u8) {
                let ms = m.source as usize;
                if ms < SOURCE_NAMES.len() {
                    SOURCE_NAMES[ms]
                } else {
                    "MIX"
                }
            } else {
                "None"
            }
        }
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
        model.channels[4].source = 7; // CH5 -> SA
        model.channels[5].source = 8; // CH6 -> SB
        model.channels[9].source = 10; // CH10 -> SD
        model.channels[17].source = 0; // CH18 -> None

        let mut buf = [0u8; 10];
        assert_eq!(format_channel_label(0, &model, &mut buf), "1:Roll");
        assert_eq!(format_channel_label(1, &model, &mut buf), "2:Pitch");
        assert_eq!(format_channel_label(2, &model, &mut buf), "3:THC");
        assert_eq!(format_channel_label(3, &model, &mut buf), "4:Yaw");
        assert_eq!(format_channel_label(4, &model, &mut buf), "5:SA");
        assert_eq!(format_channel_label(5, &model, &mut buf), "6:SB");
        assert_eq!(format_channel_label(9, &model, &mut buf), "10:SD");
        assert_eq!(format_channel_label(17, &model, &mut buf), "18:None");
    }

    #[test]
    fn test_format_channel_label_wing_tail_templates() {
        let mut model = ModelConfig::default_for_index(0);
        let mut buf = [0u8; 10];

        // Delta / Elevon preset
        model.apply_mixer_preset(crate::mixer::MixerKind::Delta);
        assert_eq!(format_channel_label(0, &model, &mut buf), "1:L");
        assert_eq!(format_channel_label(1, &model, &mut buf), "2:R");
        assert_eq!(format_channel_label(2, &model, &mut buf), "3:THC");
        assert_eq!(format_channel_label(3, &model, &mut buf), "4:Yaw");

        // Tank preset
        model.apply_mixer_preset(crate::mixer::MixerKind::Tank);
        assert_eq!(format_channel_label(0, &model, &mut buf), "1:L");
        assert_eq!(format_channel_label(1, &model, &mut buf), "2:R");
        assert!(model.is_bidir_motor(0));
        assert!(model.is_bidir_motor(1));

        // Unassigned channel with mix line driving CH1 from Throttle (source 3)
        model.channels[0].source = 0;
        model.mixes[0].target_ch = 1;
        model.mixes[0].source = 3;
        assert_eq!(format_channel_label(0, &model, &mut buf), "1:Thr");

        // Direct source assigned to CH1 (e.g. SA = source 7)
        model.channels[0].source = 7;
        assert_eq!(format_channel_label(0, &model, &mut buf), "1:SA");
    }

    #[test]
    fn test_aux_channels_screen_freeform_vs_normal() {
        let mut ctrl = MenuController::new();
        let mut buzzer = Buzzer::new();
        let mut lcd = St7567::new();
        let mut storage = RadioStorage::default_factory();

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

        // Uniform 18-channel list (CH1..CH18)
        for _ in 0..17 {
            update_aux_channels(&mut ctrl, &mut lcd, &keys_down, &mut storage, &mut buzzer);
        }
        assert_eq!(ctrl.selected_item, 17);
        update_aux_channels(&mut ctrl, &mut lcd, &keys_down, &mut storage, &mut buzzer);
        assert_eq!(ctrl.selected_item, 0);

        // Test editing CH1 (item 0) directly
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
        let expected_next = storage.next_available_aux_source(
            storage.active_model.channels[0].source,
            crate::board::ActiveBoard::AUX_SOURCES,
        );
        update_aux_channels(&mut ctrl, &mut lcd, &keys_up, &mut storage, &mut buzzer);
        assert_eq!(storage.active_model.channels[0].source, expected_next);
    }
}
