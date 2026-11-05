//! Native Spektrum Forward Programming (AS3X / SAFE) & TextGen Menu Screen.
//! Gated by `#[cfg(feature = "dsm-fwd-prog")]`.

use crate::buzzer::Buzzer;
use crate::display::St7567;
use crate::dsm::{self, fwd_prog::DsmConfigMode};
use crate::menu::format::ascii_as_str;
use crate::menu::widgets;
use crate::menu::{MenuController, MenuState, NavKeys};

#[inline(never)]
pub fn update(ctrl: &mut MenuController, lcd: &mut St7567, keys: &NavKeys, buzzer: &mut Buzzer) {
    let engine = dsm::get_config_engine();

    if keys.cancel && !ctrl.editing {
        ctrl.state = MenuState::RxSetup;
        ctrl.selected_item = 4;
        ctrl.scroll_offset = 1;
        ctrl.waiting_release = true;
        buzzer.click();
        return;
    }

    match engine.mode {
        DsmConfigMode::Disconnected | DsmConfigMode::Connecting => {
            if keys.ok {
                engine.start();
                buzzer.click();
            }
            widgets::draw_header(lcd, "DSM FORWARD PROG");
            lcd.draw_str_6x10(4, 19, "Connecting to RX...", false);
            lcd.draw_str_4x6(4, 33, "Ensure DSMX telemetry RX is bound", false);
            widgets::draw_footer(lcd, "[OK] Retry   [ESC] Back");
        }
        DsmConfigMode::ForwardProgMenu => {
            let count = engine.line_count.max(1);
            if !ctrl.editing {
                widgets::navigate_4slot_list(
                    &mut ctrl.selected_item,
                    &mut ctrl.scroll_offset,
                    count,
                    keys.up,
                    keys.down,
                    buzzer,
                );
                if keys.ok && ctrl.selected_item < engine.line_count {
                    let line = &engine.lines[ctrl.selected_item];
                    if line.editable {
                        ctrl.editing = true;
                        ctrl.edit_val = line.cur_val as i32;
                    } else {
                        engine.send_action(line.id, 0);
                    }
                    buzzer.click();
                }
            } else {
                let sel = ctrl.selected_item;
                if keys.up {
                    ctrl.edit_val += 1;
                    buzzer.play_tone(2200, 20);
                } else if keys.down {
                    ctrl.edit_val -= 1;
                    buzzer.play_tone(2200, 20);
                } else if keys.ok {
                    let line_id = engine.lines[sel].id;
                    engine.lines[sel].cur_val = ctrl.edit_val as i16;
                    engine.send_action(line_id, ctrl.edit_val as i16);
                    ctrl.editing = false;
                    buzzer.click();
                } else if keys.cancel {
                    ctrl.editing = false;
                    buzzer.click();
                }
            }

            let title = if engine.title_len > 0 {
                ascii_as_str(&engine.title[..engine.title_len as usize])
            } else {
                "FORWARD PROGRAMMING"
            };
            widgets::draw_header(lcd, title);

            for slot in 0..4 {
                let idx = ctrl.scroll_offset + slot;
                if idx >= engine.line_count {
                    break;
                }
                let line = &engine.lines[idx];
                let lbl = ascii_as_str(&line.text[..line.text_len as usize]);
                widgets::draw_list_row_right(
                    lcd,
                    slot,
                    idx == ctrl.selected_item,
                    lbl,
                    if line.editable { Some("[EDIT]") } else { Some(">") },
                );
            }

            widgets::draw_footer(
                lcd,
                if ctrl.editing {
                    "[OK] Commit  [UP/DN] Value"
                } else {
                    "[OK] Select  [ESC] Back"
                },
            );
        }
        DsmConfigMode::TextGenScreen => {
            if keys.up {
                engine.send_action(0x01, 1);
            } else if keys.down {
                engine.send_action(0x02, 1);
            } else if keys.ok {
                engine.send_action(0x03, 1);
            }
            let hdr = ascii_as_str(&engine.textgen_rows[0]);
            widgets::draw_header(lcd, hdr);
            for row in 1..6 {
                let y = 12 + ((row as i32 - 1) * 8);
                lcd.draw_str_4x6(4, y, ascii_as_str(&engine.textgen_rows[row]), false);
            }
            widgets::draw_footer(lcd, "[UP/DN/OK] Nav  [ESC] Exit");
        }
    }
}
