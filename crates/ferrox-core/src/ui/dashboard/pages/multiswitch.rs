use crate::board::{ActiveBoard, BoardProfile, RfProtocol};
use crate::display::St7567;
use crate::multiswitch::{self, label_text};
use crate::storage::RadioStorage;
use crate::ui::{format, widgets};

pub fn render(
    lcd: &mut St7567,
    storage: &RadioStorage,
    switches: u8,
    slot: usize,
    page: usize,
    page_count: usize,
) {
    let model = storage.active_model();
    let sim = crate::usb::is_sim_mode();
    if sim || ActiveBoard::resolve_protocol(model.rf_protocol) != RfProtocol::Crsf {
        lcd.draw_str_6x10(
            10,
            25,
            if sim {
                "USB SIM: NO RF"
            } else {
                "CRSF REQUIRED"
            },
            false,
        );
    } else {
        for index in 0..8 {
            let (x, y, w, h) = multiswitch::switch_rect(index);
            let on = switches & (1 << index) != 0;
            lcd.fill_rect(x, y, w, h, on);
            lcd.draw_rect(x, y, w, h, true);
            if model.multiswitch_momentary[slot] & (1 << index) != 0 {
                lcd.draw_rect(x + 2, y + 2, w - 4, h - 4, !on);
            }
            let mut buf = [0; 6];
            let label = label_text(
                &model.multiswitch_labels[slot][index as usize],
                index,
                &mut buf,
            );
            lcd.draw_str_4x6(
                x + (w as i32 - label.len() as i32 * 4) / 2,
                y + 5,
                label,
                on,
            );
        }
    }
    let page_buf = format::dashboard_page_label(page, page_count);
    let mut address_buf = [0; 3];
    let address = format::format_fixed(
        model.multiswitch_addresses[slot] as i32,
        0,
        0,
        false,
        format::SignMode::None,
        "",
        &mut address_buf,
    );
    let mut footer = [b' '; 20];
    footer[..3].copy_from_slice(b"MS ");
    footer[3..3 + address.len()].copy_from_slice(address.as_bytes());
    let end = 3 + address.len();
    footer[end..end + 10].copy_from_slice(b" Hold:Menu");
    widgets::draw_footer_split(
        lcd,
        format::ascii_as_str(&page_buf),
        format::ascii_as_str(&footer[..end + 10]),
    );
}
