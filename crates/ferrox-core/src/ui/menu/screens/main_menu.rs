use crate::board::{ActiveBoard, BoardProfile, MainMenuItem};
use crate::buzzer::Buzzer;
use crate::display::St7567;
use crate::menu::widgets;
use crate::menu::{MenuController, MenuState, NavKeys};
use crate::storage::{RadioStorage, NUM_MODELS};
use crate::ui::glyphs::draw_glyph_12x12;

#[inline(never)]
pub fn update(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    storage: &mut RadioStorage,
    buzzer: &mut Buzzer,
) {
    let items = ActiveBoard::MAIN_MENU_ITEMS;
    let item_count = items.len();

    if keys.cancel {
        ctrl.state = MenuState::Closed;
        buzzer.click();
        return;
    }

    widgets::navigate_3slot_list(
        &mut ctrl.selected_item,
        &mut ctrl.scroll_offset,
        item_count,
        keys.up,
        keys.down,
        buzzer,
    );

    if keys.ok && ctrl.selected_item < item_count {
        buzzer.click();
        ctrl.waiting_release = true;
        ctrl.main_menu_selected = ctrl.selected_item;
        ctrl.main_menu_scroll = ctrl.scroll_offset;

        match items[ctrl.selected_item] {
            #[cfg(feature = "multiswitch")]
            MainMenuItem::MultiSwitch => {
                ctrl.state = MenuState::MultiSwitchSetup;
                ctrl.selected_item = 0;
                ctrl.scroll_offset = 0;
            }
            MainMenuItem::ModelSelect => {
                ctrl.state = MenuState::ModelSelect;
                ctrl.selected_item = storage.radio.active_model as usize;
                ctrl.scroll_offset = ctrl
                    .selected_item
                    .saturating_sub(2)
                    .min(NUM_MODELS.saturating_sub(4));
            }
            MainMenuItem::ModelSetup => {
                ctrl.state = MenuState::ModelSetup;
                ctrl.selected_item = 0;
                ctrl.scroll_offset = 0;
                ctrl.sub_idx = 0;
            }
            MainMenuItem::DualRateExpo => {
                ctrl.state = MenuState::DualRateExpo;
                ctrl.selected_item = 0;
                ctrl.scroll_offset = 0;
                ctrl.sub_idx = 0;
            }
            MainMenuItem::ThrottleCurve => {
                ctrl.state = MenuState::ThrottleCurve;
                ctrl.selected_item = 0;
                ctrl.scroll_offset = 0;
            }
            MainMenuItem::WingMixer => {
                ctrl.state = MenuState::WingMixer;
                ctrl.selected_item = 0;
                ctrl.scroll_offset = 0;
            }
            MainMenuItem::AuxChannels => {
                ctrl.state = MenuState::AuxChannels;
                ctrl.selected_item = 0;
                ctrl.scroll_offset = 0;
            }
            MainMenuItem::ChannelReverse => {
                ctrl.state = MenuState::ChannelReverse;
                ctrl.selected_item = 0;
                ctrl.scroll_offset = 0;
            }
            MainMenuItem::RadioSetup => {
                ctrl.state = MenuState::RadioSetup;
                ctrl.selected_item = 0;
                ctrl.scroll_offset = 0;
            }
            MainMenuItem::ChannelMonitor => {
                ctrl.state = MenuState::ChannelMonitor;
                ctrl.page_idx = 0;
            }
            MainMenuItem::Calibration => {
                ctrl.request_calibration = true;
                ctrl.state = MenuState::Closed;
                return;
            }
            MainMenuItem::DiagAnas => {
                ctrl.state = MenuState::DiagAnas;
                ctrl.page_idx = 0;
                ctrl.selected_item = 0;
                ctrl.scroll_offset = 0;
            }
            MainMenuItem::SystemInfo => {
                ctrl.state = MenuState::SystemInfo;
                ctrl.selected_item = 0;
                ctrl.scroll_offset = 0;
            }
        }
        return;
    }

    // Render Header
    widgets::draw_header(lcd, "SETTINGS MENU");

    // Render 3 visible items
    for slot in 0..3 {
        let idx = ctrl.scroll_offset + slot;
        if idx >= item_count {
            break;
        }
        let is_selected = idx == ctrl.selected_item;
        let item = items[idx];
        let glyph_data = item.glyph();
        widgets::draw_icon_row(
            lcd,
            slot,
            is_selected,
            Some(|lcd: &mut St7567, x, y, on| draw_glyph_12x12(lcd, x, y, glyph_data, on)),
            item.title(),
            None,
            0,
        );
    }

    // Render Scrollbar on right edge
    widgets::draw_scrollbar(lcd, ctrl.selected_item, item_count, 13, 40);

    // Format footer with dynamic position indicator (e.g. "[OK] Select   1/13")
    let mut pos_buf = [0u8; 8];
    let pos_str = {
        let cur = ctrl.selected_item + 1;
        let total = item_count;
        let mut i = 0;
        if cur >= 10 {
            pos_buf[i] = b'0' + (cur / 10) as u8;
            i += 1;
        }
        pos_buf[i] = b'0' + (cur % 10) as u8;
        i += 1;
        pos_buf[i] = b'/';
        i += 1;
        if total >= 10 {
            pos_buf[i] = b'0' + (total / 10) as u8;
            i += 1;
        }
        pos_buf[i] = b'0' + (total % 10) as u8;
        i += 1;
        core::str::from_utf8(&pos_buf[..i]).unwrap_or("")
    };

    widgets::draw_footer_split(lcd, "[OK] Select   [ESC] Exit", pos_str);
}
