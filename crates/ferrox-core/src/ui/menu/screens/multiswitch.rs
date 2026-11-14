use crate::buzzer::Buzzer;
use crate::display::St7567;
use crate::multiswitch::label_text;
use crate::storage::{self, RadioStorage};
use crate::ui::format::{self, next_ascii, prev_ascii, SignMode};
use crate::ui::menu::{MenuController, MenuState, NavKeys};
use crate::ui::widgets;

fn address_text(address: i16, buf: &mut [u8; 3]) -> &str {
    if address < 0 {
        "OFF"
    } else {
        format::format_fixed(address as i32, 0, 0, false, SignMode::None, "", buf)
    }
}

pub fn update(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    storage: &mut RadioStorage,
    buzzer: &mut Buzzer,
) {
    match ctrl.state {
        MenuState::MultiSwitchSetup => {
            if keys.cancel {
                ctrl.return_to_main_menu();
                buzzer.click();
                return;
            }
            widgets::navigate_4slot_list(
                &mut ctrl.selected_item,
                &mut ctrl.scroll_offset,
                4,
                keys.up,
                keys.down,
                buzzer,
            );
            if keys.ok {
                ctrl.page_idx = ctrl.selected_item;
                ctrl.selected_item = 0;
                ctrl.scroll_offset = 0;
                ctrl.state = MenuState::MultiSwitchPageSetup;
                ctrl.waiting_release = true;
                buzzer.click();
                return;
            }
            widgets::draw_header(lcd, "MULTISWITCH");
            for slot in 0..4 {
                let title = [b'M', b'S', b' ', b'1' + slot as u8];
                let mut buf = [0; 3];
                widgets::draw_list_row(
                    lcd,
                    slot,
                    ctrl.selected_item == slot,
                    format::ascii_as_str(&title),
                    Some(address_text(
                        storage.active_model.multiswitch_addresses[slot],
                        &mut buf,
                    )),
                    72,
                );
            }
        }
        MenuState::MultiSwitchPageSetup => {
            if ctrl.editing {
                if keys.cancel || keys.ok {
                    ctrl.editing = false;
                    ctrl.waiting_release = true;
                    buzzer.click();
                } else if keys.up || keys.down {
                    let address = &mut storage.active_model.multiswitch_addresses[ctrl.page_idx];
                    let next = (*address + if keys.up { 1 } else { -1 }).clamp(-1, 255);
                    ctrl.multiswitch_dirty |= next != *address;
                    *address = next;
                    buzzer.click();
                }
            } else {
                if keys.cancel {
                    if ctrl.multiswitch_dirty && !storage::save_active_model(storage) {
                        buzzer.warn_critical();
                        widgets::draw_header(lcd, "SAVE FAILED");
                        widgets::draw_footer(lcd, "[ESC] Retry");
                        return;
                    }
                    ctrl.multiswitch_dirty = false;
                    ctrl.state = MenuState::MultiSwitchSetup;
                    ctrl.selected_item = ctrl.page_idx;
                    ctrl.scroll_offset = 0;
                    ctrl.waiting_release = true;
                    buzzer.click();
                    return;
                }
                widgets::navigate_4slot_list(
                    &mut ctrl.selected_item,
                    &mut ctrl.scroll_offset,
                    9,
                    keys.up,
                    keys.down,
                    buzzer,
                );
                if keys.ok {
                    ctrl.waiting_release = true;
                    if ctrl.selected_item == 0 {
                        ctrl.editing = true;
                    } else {
                        ctrl.edit_val = (ctrl.selected_item - 1) as i32;
                        ctrl.state = MenuState::MultiSwitchButtonSetup;
                        ctrl.selected_item = 0;
                        ctrl.scroll_offset = 0;
                        ctrl.sub_idx = 0;
                    }
                    buzzer.click();
                    return;
                }
            }
            let mut title = *b"MULTISWITCH 1";
            title[12] += ctrl.page_idx as u8;
            widgets::draw_header(lcd, format::ascii_as_str(&title));
            for line in 0..4 {
                let row = ctrl.scroll_offset + line;
                if row >= 9 {
                    break;
                }
                let selected = row == ctrl.selected_item;
                if row == 0 {
                    let mut buf = [0; 3];
                    widgets::draw_list_row(
                        lcd,
                        line,
                        selected,
                        "Address:",
                        Some(address_text(
                            storage.active_model.multiswitch_addresses[ctrl.page_idx],
                            &mut buf,
                        )),
                        72,
                    );
                } else {
                    let mut buf = [0; 6];
                    let label = label_text(
                        &storage.active_model.multiswitch_labels[ctrl.page_idx][row - 1],
                        (row - 1) as u8,
                        &mut buf,
                    );
                    let prefix = [b'F', b'0' + row as u8, b':'];
                    widgets::draw_list_row(
                        lcd,
                        line,
                        selected,
                        format::ascii_as_str(&prefix),
                        Some(label),
                        30,
                    );
                    let momentary = storage.active_model.multiswitch_momentary[ctrl.page_idx]
                        & (1 << (row - 1))
                        != 0;
                    lcd.draw_str_6x10(
                        108,
                        14 + line as i32 * 9,
                        if momentary { "T" } else { "S" },
                        selected,
                    );
                }
            }
            widgets::draw_scrollbar(lcd, ctrl.selected_item, 9, 14, 36);
        }
        MenuState::MultiSwitchButtonSetup => {
            let button = ctrl.edit_val as usize;
            if ctrl.editing {
                if keys.cancel {
                    ctrl.editing = false;
                    ctrl.waiting_release = true;
                    buzzer.click();
                } else if keys.ok {
                    ctrl.sub_idx = (ctrl.sub_idx + 1) % 6;
                    buzzer.click();
                } else if keys.up || keys.down {
                    let ch = &mut storage.active_model.multiswitch_labels[ctrl.page_idx][button]
                        [ctrl.sub_idx];
                    *ch = if keys.up {
                        next_ascii(*ch)
                    } else {
                        prev_ascii(*ch)
                    };
                    ctrl.multiswitch_dirty = true;
                    buzzer.click();
                }
            } else {
                if keys.cancel {
                    ctrl.state = MenuState::MultiSwitchPageSetup;
                    ctrl.selected_item = button + 1;
                    ctrl.scroll_offset = ctrl.selected_item.saturating_sub(3);
                    ctrl.waiting_release = true;
                    buzzer.click();
                    return;
                }
                widgets::navigate_4slot_list(
                    &mut ctrl.selected_item,
                    &mut ctrl.scroll_offset,
                    2,
                    keys.up,
                    keys.down,
                    buzzer,
                );
                if keys.ok {
                    if ctrl.selected_item == 0 {
                        ctrl.editing = true;
                        ctrl.sub_idx = 0;
                        // Blank persistent labels use numeric fallbacks until edited.
                        let label =
                            &mut storage.active_model.multiswitch_labels[ctrl.page_idx][button];
                        for ch in label {
                            if *ch == 0 {
                                *ch = b' ';
                            }
                        }
                    } else {
                        storage.active_model.multiswitch_momentary[ctrl.page_idx] ^= 1 << button;
                        ctrl.multiswitch_dirty = true;
                    }
                    ctrl.waiting_release = true;
                    buzzer.click();
                }
            }
            widgets::draw_header(lcd, "MULTISWITCH BUTTON");
            let label = &storage.active_model.multiswitch_labels[ctrl.page_idx][button];
            let mut buf = [0; 6];
            let text = if ctrl.editing {
                format::ascii_as_str(label)
            } else {
                label_text(label, button as u8, &mut buf)
            };
            widgets::draw_list_row(
                lcd,
                0,
                ctrl.selected_item == 0 && !ctrl.editing,
                "Text:",
                Some(text),
                48,
            );
            if ctrl.editing {
                let x = 48 + ctrl.sub_idx as i32 * 6;
                lcd.fill_rect(x - 1, 14, 8, 9, true);
                lcd.draw_str_6x10(
                    x,
                    14,
                    format::ascii_as_str(&label[ctrl.sub_idx..ctrl.sub_idx + 1]),
                    true,
                );
            }
            let momentary =
                storage.active_model.multiswitch_momentary[ctrl.page_idx] & (1 << button) != 0;
            widgets::draw_list_row(
                lcd,
                1,
                ctrl.selected_item == 1,
                "Type:",
                Some(if momentary { "MOMENTARY" } else { "TOGGLE" }),
                48,
            );
        }
        _ => unreachable!(),
    }
    widgets::draw_footer(
        lcd,
        if ctrl.editing {
            "[UP/DN] Edit  [ESC] Done"
        } else {
            "[OK] Select  [ESC] Back"
        },
    );
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
    fn configure_address_label_and_momentary_mode() {
        let mut ctrl = MenuController::new();
        let mut lcd = St7567::new();
        let mut storage = RadioStorage::default_factory();
        let mut buzzer = Buzzer::new();
        storage.active_model.multiswitch_addresses = [-1; 4];
        ctrl.state = MenuState::MultiSwitchSetup;
        ctrl.selected_item = 2;
        let mut keys = nav();
        keys.ok = true;
        update(&mut ctrl, &mut lcd, &keys, &mut storage, &mut buzzer);
        assert_eq!(ctrl.page_idx, 2);
        assert_eq!(ctrl.state, MenuState::MultiSwitchPageSetup);
        update(&mut ctrl, &mut lcd, &keys, &mut storage, &mut buzzer);
        assert!(ctrl.editing);
        keys = nav();
        keys.up = true;
        update(&mut ctrl, &mut lcd, &keys, &mut storage, &mut buzzer);
        assert_eq!(storage.active_model.multiswitch_addresses, [-1, -1, 0, -1]);
        keys = nav();
        keys.cancel = true;
        update(&mut ctrl, &mut lcd, &keys, &mut storage, &mut buzzer);
        assert!(!ctrl.editing);
        ctrl.selected_item = 8;
        keys = nav();
        keys.ok = true;
        update(&mut ctrl, &mut lcd, &keys, &mut storage, &mut buzzer);
        assert_eq!(ctrl.state, MenuState::MultiSwitchButtonSetup);
        assert_eq!(ctrl.edit_val, 7);
        update(&mut ctrl, &mut lcd, &keys, &mut storage, &mut buzzer);
        keys = nav();
        keys.up = true;
        update(&mut ctrl, &mut lcd, &keys, &mut storage, &mut buzzer);
        assert_eq!(storage.active_model.multiswitch_labels[2][7][0], b'A');
        keys = nav();
        keys.cancel = true;
        update(&mut ctrl, &mut lcd, &keys, &mut storage, &mut buzzer);
        ctrl.selected_item = 1;
        keys = nav();
        keys.ok = true;
        update(&mut ctrl, &mut lcd, &keys, &mut storage, &mut buzzer);
        assert_eq!(storage.active_model.multiswitch_momentary, [0, 0, 128, 0]);
        assert!(ctrl.multiswitch_dirty);
        keys = nav();
        keys.cancel = true;
        update(&mut ctrl, &mut lcd, &keys, &mut storage, &mut buzzer);
        assert_eq!(ctrl.state, MenuState::MultiSwitchPageSetup);
        assert_eq!(ctrl.selected_item, 8);
    }
}
