//! Radio settings and Protocol configuration screens.

use crate::board::{
    ActiveBoard, BoardProfile, DsmSubProtocol, HardwareExtItem, RadioSetupItem, RfProtocol,
};
use crate::buzzer::Buzzer;
use crate::display::St7567;
use crate::menu::format::{
    ascii_as_str, format_deci_volt, format_pct_3, format_servo_hz, format_u8_2, u32_to_hex,
};
use crate::menu::widgets;
use crate::menu::{MenuController, MenuState, NavKeys};
use crate::storage::{self, RadioStorage};
use crate::trim::TrimController;

#[inline(never)]
pub fn update_radio_setup(
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

    let items = ActiveBoard::RADIO_SETUP_ITEMS;
    let item_count = items.len();

    widgets::navigate_4slot_list(
        &mut ctrl.selected_item,
        &mut ctrl.scroll_offset,
        item_count,
        keys.up,
        keys.down,
        buzzer,
    );

    if keys.ok && ctrl.selected_item < item_count {
        match items[ctrl.selected_item] {
            RadioSetupItem::ThrottleTrim => {
                buzzer.click();
                storage.radio.throttle_trim = (storage.radio.throttle_trim + 1) % 3;
                trims.throttle_enabled = storage.radio.throttle_trim != 0;
                storage::save_radio_config(storage);
            }
            RadioSetupItem::Beeper => {
                storage.radio.audio_enabled = if storage.radio.audio_enabled == 0 {
                    1
                } else {
                    0
                };
                buzzer.enabled = storage.radio.audio_enabled != 0;
                if buzzer.enabled {
                    buzzer.click();
                }
                storage::save_radio_config(storage);
            }
            RadioSetupItem::ToneStyle => {
                storage.radio.tone_style = if storage.radio.tone_style == 0 { 1 } else { 0 };
                buzzer.tone_style = crate::buzzer::ToneStyle::from_u8(storage.radio.tone_style);
                storage::save_radio_config(storage);
                if buzzer.tone_style == crate::buzzer::ToneStyle::Rich {
                    buzzer.chime_armed();
                } else {
                    buzzer.click();
                }
            }
            RadioSetupItem::BacklightTimeout => {
                buzzer.click();
                storage.radio.backlight_timeout = (storage.radio.backlight_timeout + 1) % 4;
                storage::save_radio_config(storage);
            }
            RadioSetupItem::BacklightBrightness => {
                buzzer.click();
                storage.radio.backlight_brightness = if storage.radio.backlight_brightness >= 10 {
                    1
                } else {
                    storage.radio.backlight_brightness + 1
                };
                lcd.set_backlight_level(storage.radio.backlight_brightness * 10);
                storage::save_radio_config(storage);
            }
            RadioSetupItem::LcdContrast => {
                buzzer.click();
                storage.radio.lcd_contrast = if storage.radio.lcd_contrast >= 50 {
                    20
                } else {
                    (storage.radio.lcd_contrast + 3).min(50)
                };
                lcd.set_contrast(storage.radio.lcd_contrast);
                storage::save_radio_config(storage);
            }
            RadioSetupItem::BatteryWarn => {
                buzzer.click();
                storage.radio.vbat_warn_deci = if storage.radio.vbat_warn_deci >= 50 {
                    40
                } else {
                    storage.radio.vbat_warn_deci + 1
                };
                storage::save_radio_config(storage);
            }
            RadioSetupItem::UsbMode => {
                buzzer.click();
                storage.radio.usb_mode = (storage.radio.usb_mode + 1) % 4;
                storage::save_radio_config(storage);
                crate::usb::request_init(storage.radio.usb_mode);
            }
            RadioSetupItem::HardwareSetupSubmenu => {
                buzzer.click();
                ctrl.state = MenuState::HardwareSetup;
                ctrl.selected_item = 0;
                ctrl.scroll_offset = 0;
                return;
            }
            RadioSetupItem::RearLeftFunc => {
                buzzer.click();
                storage.radio.rear_left_func = (storage.radio.rear_left_func + 1) % 3;
                storage::save_radio_config(storage);
            }
            RadioSetupItem::RearRightFunc => {
                buzzer.click();
                storage.radio.rear_right_func = (storage.radio.rear_right_func + 1) % 3;
                storage::save_radio_config(storage);
            }
            RadioSetupItem::TouchEnabled => {
                buzzer.click();
                storage.radio.touch_disabled = if storage.radio.touch_disabled == 0 {
                    1
                } else {
                    0
                };
                storage::save_radio_config(storage);
            }
            RadioSetupItem::StickMode => {
                buzzer.click();
                storage.radio.stick_mode = (storage.radio.stick_mode + 1) % 4;
                storage::save_radio_config(storage);
            }
            RadioSetupItem::GimbalType => {
                buzzer.click();
                storage.radio.gimbal_type = if storage.radio.gimbal_type == 0 { 1 } else { 0 };
                crate::input::apply_calibration(&storage.radio);
                storage::save_radio_config(storage);
            }
            RadioSetupItem::SwitchBeep => {
                buzzer.click();
                storage.radio.switch_beep = if storage.radio.switch_beep == 0 { 1 } else { 0 };
                storage::save_radio_config(storage);
            }
            RadioSetupItem::InputSetupSubmenu => {
                buzzer.click();
                ctrl.state = MenuState::InputSetup;
                ctrl.selected_item = 0;
                ctrl.scroll_offset = 0;
                return;
            }
        }
    }

    widgets::draw_header(lcd, "RADIO SETUP");

    for slot in 0..4 {
        let idx = ctrl.scroll_offset + slot;
        if idx >= item_count {
            break;
        }
        let is_sel = idx == ctrl.selected_item;
        render_radio_setup_item(lcd, slot, items[idx], is_sel, storage);
    }

    widgets::draw_footer(lcd, "[OK] Toggle/Cycle   [ESC] Back");
}

#[inline(never)]
fn render_radio_setup_item(
    lcd: &mut St7567,
    slot: usize,
    item: RadioSetupItem,
    is_sel: bool,
    storage: &RadioStorage,
) {
    match item {
        RadioSetupItem::ThrottleTrim => {
            let val_str = match storage.radio.throttle_trim {
                1 => "IDLE",
                2 => "LINEAR",
                _ => "OFF (Lock)",
            };
            widgets::draw_list_row_right(lcd, slot, is_sel, "Thr Trim:", Some(val_str));
        }
        RadioSetupItem::Beeper => {
            let beeper_str = if storage.radio.audio_enabled != 0 {
                "ENABLED"
            } else {
                "MUTED"
            };
            widgets::draw_list_row_right(lcd, slot, is_sel, "Beeper:", Some(beeper_str));
        }
        RadioSetupItem::ToneStyle => {
            let tone_str = if storage.radio.tone_style != 0 {
                "RICH"
            } else {
                "SIMPLE"
            };
            widgets::draw_list_row_right(lcd, slot, is_sel, "Tones:", Some(tone_str));
        }
        RadioSetupItem::BacklightTimeout => {
            let timer_str = match storage.radio.backlight_timeout {
                1 => "15 SEC",
                2 => "30 SEC",
                3 => "60 SEC",
                _ => "ALWAYS ON",
            };
            widgets::draw_list_row_right(lcd, slot, is_sel, "BL Timer:", Some(timer_str));
        }
        RadioSetupItem::BacklightBrightness => {
            let mut b_buf = [0u8; 4];
            let pct = (storage.radio.backlight_brightness * 10).min(100);
            let b_str = format_pct_3(pct, &mut b_buf);
            widgets::draw_list_row_right(lcd, slot, is_sel, "BL Level:", Some(b_str));
        }
        RadioSetupItem::LcdContrast => {
            let mut c_buf = [0u8; 2];
            let c_str = format_u8_2(storage.radio.lcd_contrast, &mut c_buf);
            widgets::draw_list_row_right(lcd, slot, is_sel, "Contrast:", Some(c_str));
        }
        RadioSetupItem::BatteryWarn => {
            let mut v_buf = [0u8; 4];
            let v_str = format_deci_volt(storage.radio.vbat_warn_deci, &mut v_buf);
            widgets::draw_list_row_right(lcd, slot, is_sel, "Bat Warn:", Some(v_str));
        }
        RadioSetupItem::UsbMode => {
            let usb_str = match storage.radio.usb_mode {
                1 => "JOYSTICK",
                2 => "SERIAL",
                3 => "COMPOSITE",
                _ => "OFF",
            };
            widgets::draw_list_row_right(lcd, slot, is_sel, "USB Mode:", Some(usb_str));
        }
        RadioSetupItem::HardwareSetupSubmenu => {
            widgets::draw_list_row_right(lcd, slot, is_sel, "Hardware Ext", Some(">"));
        }
        RadioSetupItem::RearLeftFunc => {
            let s = match storage.radio.rear_left_func {
                1 => "TIMR RST",
                2 => "TRIM MOD",
                _ => "SWE (2P)",
            };
            widgets::draw_list_row_right(lcd, slot, is_sel, "Rear L (SWE):", Some(s));
        }
        RadioSetupItem::RearRightFunc => {
            let s = match storage.radio.rear_right_func {
                1 => "INST TRIM",
                2 => "TRIM MOD",
                _ => "SWF (MOM)",
            };
            widgets::draw_list_row_right(lcd, slot, is_sel, "Rear R (SWF):", Some(s));
        }
        RadioSetupItem::TouchEnabled => {
            let t_str = if storage.radio.touch_disabled == 0 {
                "ENABLED"
            } else {
                "DISABLED"
            };
            widgets::draw_list_row_right(lcd, slot, is_sel, "Touch Scr:", Some(t_str));
        }
        RadioSetupItem::StickMode => {
            let mode_str = match storage.radio.stick_mode {
                0 => "MODE 1",
                1 => "MODE 2",
                2 => "MODE 3",
                _ => "MODE 4",
            };
            widgets::draw_list_row_right(lcd, slot, is_sel, "Stick Mode:", Some(mode_str));
        }
        RadioSetupItem::GimbalType => {
            let gmb_str = if storage.radio.is_hall_gimbals() {
                "HALL"
            } else {
                "POT"
            };
            widgets::draw_list_row_right(lcd, slot, is_sel, "Gimbals:", Some(gmb_str));
        }
        RadioSetupItem::SwitchBeep => {
            let swb_str = if storage.radio.switch_beep != 0 {
                "ENABLED"
            } else {
                "MUTED"
            };
            widgets::draw_list_row_right(lcd, slot, is_sel, "Switch Beep:", Some(swb_str));
        }
        RadioSetupItem::InputSetupSubmenu => {
            widgets::draw_list_row_right(lcd, slot, is_sel, "Input Modes", Some(">"));
        }
    }
}

/// Standalone declarative Hardware Extensions configuration screen.
#[inline(never)]
pub fn update_hardware_setup(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    storage: &mut RadioStorage,
    buzzer: &mut Buzzer,
) {
    if keys.cancel {
        ctrl.state = MenuState::RadioSetup;
        // Return to the HardwareSetupSubmenu slot in RadioSetup
        let items = ActiveBoard::RADIO_SETUP_ITEMS;
        let pos = items
            .iter()
            .position(|&it| it == RadioSetupItem::HardwareSetupSubmenu)
            .unwrap_or(0);
        ctrl.selected_item = pos;
        ctrl.scroll_offset = pos.saturating_sub(2);
        buzzer.click();
        return;
    }

    let items = ActiveBoard::HARDWARE_EXT_ITEMS;
    let item_count = items.len();

    widgets::navigate_4slot_list(
        &mut ctrl.selected_item,
        &mut ctrl.scroll_offset,
        item_count,
        keys.up,
        keys.down,
        buzzer,
    );

    if keys.ok && ctrl.selected_item < item_count {
        match items[ctrl.selected_item] {
            HardwareExtItem::ExtModulePwr => {
                buzzer.click();
                storage.radio.ext_module_pwr = if storage.radio.ext_module_pwr == 0 {
                    1
                } else {
                    0
                };
                crate::crsf::set_power_polarity(storage.radio.ext_module_pwr == 0);
                storage::save_radio_config(storage);
            }
            HardwareExtItem::ExtSwitches => {
                buzzer.click();
                storage.radio.ext_switches = if storage.radio.ext_switches == 0 {
                    1
                } else {
                    0
                };
                crate::input::set_ext_switches_enabled(storage.radio.ext_switches != 0);
                storage::save_radio_config(storage);
            }
            HardwareExtItem::J4Mode => {
                buzzer.click();
                storage.radio.j4_sg_sh_en = if storage.radio.j4_sg_sh_en == 0 { 1 } else { 0 };
                ActiveBoard::set_j4_mode_enabled(storage.radio.j4_sg_sh_en != 0);
                storage::save_radio_config(storage);
            }
            HardwareExtItem::ExtAdc => {
                buzzer.click();
                storage.radio.ext_adc = if storage.radio.ext_adc == 0 { 1 } else { 0 };
                crate::adc::set_ext_adc_enabled(storage.radio.ext_adc != 0);
                crate::input::set_ext_adc_enabled(storage.radio.ext_adc != 0);
                crate::input::apply_calibration(&storage.radio);
                storage::save_radio_config(storage);
            }
            HardwareExtItem::PpmOut => {
                buzzer.click();
                let en = !storage.radio.is_ppm_out_enabled();
                storage.radio.set_ppm_out_enabled(en);
                crate::trainer::set_ppm_out(en);
                storage::save_radio_config(storage);
            }
            HardwareExtItem::PpmIn => {
                buzzer.click();
                storage.radio.cycle_trainer_in_mode();
                let ppm_en = storage.radio.is_ppm_in_enabled();
                crate::trainer::set_ppm_in(ppm_en);
                storage::save_radio_config(storage);
            }
        }
    }

    widgets::draw_header(lcd, "HARDWARE EXT");

    for slot in 0..4 {
        let idx = ctrl.scroll_offset + slot;
        if idx >= item_count {
            break;
        }
        let is_sel = idx == ctrl.selected_item;
        render_hardware_ext_item(lcd, slot, items[idx], is_sel, storage);
    }

    widgets::draw_footer(lcd, "[OK] Toggle/Cycle   [ESC] Back");
}

#[inline(never)]
fn render_hardware_ext_item(
    lcd: &mut St7567,
    slot: usize,
    item: HardwareExtItem,
    is_sel: bool,
    storage: &RadioStorage,
) {
    match item {
        HardwareExtItem::ExtModulePwr => {
            let pwr_str = if storage.radio.ext_module_pwr == 0 {
                "HIGH (N)"
            } else {
                "LOW (P)"
            };
            widgets::draw_list_row_right(
                lcd,
                slot,
                is_sel,
                ActiveBoard::EXT_PWR_LABEL,
                Some(pwr_str),
            );
        }
        HardwareExtItem::ExtSwitches => {
            let sw_str = if storage.radio.ext_switches != 0 {
                "PC12+PC15"
            } else {
                "OFF"
            };
            widgets::draw_list_row_right(lcd, slot, is_sel, "Ext Sw:", Some(sw_str));
        }
        HardwareExtItem::J4Mode => {
            let j4_str = if storage.radio.j4_sg_sh_en != 0 {
                "SG/SH BTN"
            } else {
                "SWD (DBG)"
            };
            widgets::draw_list_row_right(lcd, slot, is_sel, "J4 Mode:", Some(j4_str));
        }
        HardwareExtItem::ExtAdc => {
            let adc_str = if storage.radio.ext_adc != 0 {
                ActiveBoard::EXT_ADC_ON_STR
            } else {
                ActiveBoard::EXT_ADC_OFF_STR
            };
            widgets::draw_list_row_right(
                lcd,
                slot,
                is_sel,
                ActiveBoard::EXT_ADC_LABEL,
                Some(adc_str),
            );
        }
        HardwareExtItem::PpmOut => {
            let out_str = if storage.radio.is_ppm_out_enabled() {
                ActiveBoard::PPM_OUT_ON_STR
            } else {
                "OFF"
            };
            widgets::draw_list_row_right(lcd, slot, is_sel, "PPM Out:", Some(out_str));
        }
        HardwareExtItem::PpmIn => {
            let in_str = if storage.radio.is_ppm_in_enabled() {
                ActiveBoard::PPM_IN_ON_STR
            } else if storage.radio.is_dsm_sat_in_enabled() {
                "DSMX SAT"
            } else {
                "OFF"
            };
            widgets::draw_list_row_right(lcd, slot, is_sel, "Trainer In:", Some(in_str));
        }
    }
}

const MODE_STRS: [&str; 8] = [
    "DEFAULT",
    "OFF",
    "2-POS",
    "3-POS",
    "6-POS",
    "POT",
    "POT-D",
    "INST-TRIM",
];

#[inline(never)]
pub fn update_input_setup(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    storage: &mut RadioStorage,
    buzzer: &mut Buzzer,
) {
    if keys.cancel {
        ctrl.state = MenuState::RadioSetup;
        let items = ActiveBoard::RADIO_SETUP_ITEMS;
        let pos = items
            .iter()
            .position(|&it| it == RadioSetupItem::InputSetupSubmenu)
            .unwrap_or(0);
        ctrl.selected_item = pos;
        ctrl.scroll_offset = pos.saturating_sub(2);
        buzzer.click();
        return;
    }

    let input_items = ActiveBoard::input_setup_items(storage.radio.ext_adc != 0);

    widgets::navigate_4slot_list(
        &mut ctrl.selected_item,
        &mut ctrl.scroll_offset,
        input_items,
        keys.up,
        keys.down,
        buzzer,
    );

    if keys.ok {
        let ch = ctrl.selected_item;
        if ch < input_items {
            buzzer.click();
            let current = storage.radio.get_adc_mode_val(ch);
            storage
                .radio
                .set_adc_mode_val(ch, (current + 1) % (MODE_STRS.len() as u8));
            crate::input::apply_calibration(&storage.radio);
            storage::save_radio_config(storage);
        }
    }

    widgets::draw_header(lcd, "INPUT MODES");

    for slot in 0..4 {
        let idx = ctrl.scroll_offset + slot;
        if idx >= input_items {
            break;
        }
        let is_sel = idx == ctrl.selected_item;
        let mode_idx = (storage.radio.get_adc_mode_val(idx) as usize).min(MODE_STRS.len() - 1);
        let mode_str = MODE_STRS[mode_idx];
        let name = if idx < ActiveBoard::AUX_NAMES.len() {
            ActiveBoard::AUX_NAMES[idx]
        } else {
            "AUX:"
        };
        widgets::draw_list_row_right(lcd, slot, is_sel, name, Some(mode_str));
    }

    widgets::draw_footer(lcd, "[OK] Cycle Mode    [ESC] Back");
}

pub const TELEM_SENSOR_NAMES: [&str; 23] = [
    "RX BAT", "LQ/RSSI", "TX BAT", "CURRENT", "CAP mAh", "TX PWR", "SNR", "PKT RATE", "ANTENNA",
    "PKTS TX", "PKTS RX", "TIMER", "VSPD", "ALT", "CELLS", "RPM", "TEMP", "GSPD", "SATS", "HDG",
    "PITCH", "ROLL", "OFF",
];

pub const SLOT_NAMES: [&str; 4] = ["Top-L:", "Top-R:", "Btm-L:", "Btm-R:"];

#[inline(never)]
pub fn update_telem_setup(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    storage: &mut RadioStorage,
    buzzer: &mut Buzzer,
) {
    if keys.cancel {
        ctrl.state = MenuState::ModelSetup;
        ctrl.selected_item = 7;
        ctrl.scroll_offset = 4;
        buzzer.click();
        return;
    }

    const SLOT_COUNT: usize = 4;

    widgets::navigate_4slot_list(
        &mut ctrl.selected_item,
        &mut ctrl.scroll_offset,
        SLOT_COUNT,
        keys.up,
        keys.down,
        buzzer,
    );

    if keys.ok {
        let idx = ctrl.selected_item;
        if idx < SLOT_COUNT {
            buzzer.click();
            let current = storage.active_model.telem_slots[idx];
            storage.active_model.telem_slots[idx] =
                (current + 1) % (TELEM_SENSOR_NAMES.len() as u8);
            storage::save_active_model(storage);
        }
    }

    widgets::draw_header(lcd, "TELEM DASH SETUP");

    for slot in 0..4 {
        let idx = ctrl.scroll_offset + slot;
        if idx >= SLOT_COUNT {
            break;
        }
        let is_sel = idx == ctrl.selected_item;
        let sensor_idx =
            (storage.active_model.telem_slots[idx] as usize).min(TELEM_SENSOR_NAMES.len() - 1);
        widgets::draw_list_row_right(
            lcd,
            slot,
            is_sel,
            SLOT_NAMES[idx],
            Some(TELEM_SENSOR_NAMES[sensor_idx]),
        );
    }

    widgets::draw_footer(lcd, "[OK] Cycle Sensor  [ESC] Back");
}

#[inline(never)]
pub fn update_rx_setup(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    storage: &mut RadioStorage,
    buzzer: &mut Buzzer,
) {
    let current_proto = ActiveBoard::resolve_protocol(storage.active_model.rf_protocol);
    if storage.active_model.rf_protocol != current_proto as u8 {
        storage.active_model.rf_protocol = current_proto as u8;
        storage::save_active_model(storage);
    }

    let item_count = match current_proto {
        RfProtocol::Afhds2a => 6,
        RfProtocol::Crsf => 5,
        RfProtocol::Cc2500 => 4,
        RfProtocol::Dsm => {
            #[cfg(feature = "dsm-fwd-prog")]
            {
                5
            }
            #[cfg(not(feature = "dsm-fwd-prog"))]
            {
                4
            }
        }
    };

    if keys.cancel {
        if ctrl.editing {
            ctrl.editing = false;
            buzzer.click();
        } else {
            ctrl.state = MenuState::ModelSetup;
            ctrl.selected_item = 6;
            ctrl.scroll_offset = 3;
            storage::save_active_model(storage);
            buzzer.click();
            return;
        }
    }

    if !ctrl.editing {
        widgets::navigate_4slot_list(
            &mut ctrl.selected_item,
            &mut ctrl.scroll_offset,
            item_count,
            keys.up,
            keys.down,
            buzzer,
        );

        if keys.ok {
            match (current_proto, ctrl.selected_item) {
                (_, 0) => {
                    // Enter edit mode for Protocol
                    ctrl.editing = true;
                    buzzer.click();
                }
                (RfProtocol::Afhds2a, 1) => {
                    ctrl.request_bind = true;
                    ctrl.state = MenuState::Closed;
                    buzzer.click();
                    return;
                }
                (RfProtocol::Afhds2a, 2) => {
                    buzzer.click();
                    storage.active_model.rf_power = (storage.active_model.rf_power + 1) % 4;
                    storage::save_active_model(storage);
                }
                (RfProtocol::Afhds2a, 3) => {
                    buzzer.click();
                    storage.active_model.servo_rate_hz = match storage.active_model.servo_rate_hz {
                        50 => 100,
                        100 => 200,
                        200 => 333,
                        333 => 400,
                        _ => 50,
                    };
                    storage::save_active_model(storage);
                }
                (RfProtocol::Afhds2a, 4) => {
                    buzzer.click();
                    storage.active_model.rx_out_mode = if storage.active_model.rx_out_mode == 0 {
                        1
                    } else {
                        0
                    };
                    storage::save_active_model(storage);
                }
                (RfProtocol::Afhds2a, 5) => {
                    buzzer.click();
                    storage.active_model.rx_serial_proto =
                        if storage.active_model.rx_serial_proto == 0 {
                            1
                        } else {
                            0
                        };
                    storage::save_active_model(storage);
                }
                (RfProtocol::Crsf, 1) => {
                    ctrl.editing = true;
                    buzzer.click();
                }
                (RfProtocol::Crsf, 2) => {
                    buzzer.click();
                    storage.active_model.crsf_half_duplex =
                        if storage.active_model.crsf_half_duplex == 0 {
                            1
                        } else {
                            0
                        };
                    storage::save_active_model(storage);
                }
                (RfProtocol::Crsf, 3) => {
                    buzzer.click();
                    storage.active_model.crsf_arm_mode = if storage.active_model.crsf_arm_mode == 0
                    {
                        1
                    } else {
                        0
                    };
                    storage::save_active_model(storage);
                }
                (RfProtocol::Crsf, 4) => {
                    buzzer.click();
                    ctrl.state = MenuState::CrsfSetup;
                    ctrl.selected_item = 0;
                    ctrl.scroll_offset = 0;
                    return;
                }
                (RfProtocol::Dsm, 1) => {
                    buzzer.click();
                    let cur = DsmSubProtocol::from_u8(storage.active_model.dsm_sub_proto);
                    storage.active_model.dsm_sub_proto = cur.next() as u8;
                    storage::save_active_model(storage);
                }
                (RfProtocol::Dsm, 2) => {
                    buzzer.click();
                    ctrl.request_bind = true;
                    ctrl.state = MenuState::Closed;
                    return;
                }
                (RfProtocol::Dsm, 3) => {
                    buzzer.click();
                    storage.active_model.rf_power = if storage.active_model.rf_power == 0 {
                        1
                    } else {
                        0
                    };
                    storage::save_active_model(storage);
                }
                (RfProtocol::Dsm, 4) => {
                    #[cfg(feature = "dsm-fwd-prog")]
                    {
                        ctrl.state = MenuState::DsmSetup;
                        ctrl.return_state = MenuState::RxSetup;
                        ctrl.selected_item = 0;
                        ctrl.scroll_offset = 0;
                        ctrl.waiting_release = true;
                        crate::dsm::start_forward_prog();
                        buzzer.click();
                        return;
                    }
                    #[cfg(not(feature = "dsm-fwd-prog"))]
                    {
                        buzzer.click();
                    }
                }
                (RfProtocol::Cc2500, 1) => {
                    buzzer.click();
                    let cur = crate::rf::mpm::Cc2500Protocol::from_u8(
                        storage.active_model.rx_serial_proto,
                    );
                    storage.active_model.rx_serial_proto = cur.next() as u8;
                    storage::save_active_model(storage);
                }
                (RfProtocol::Cc2500, 2) => {
                    buzzer.click();
                    ctrl.request_bind = true;
                    ctrl.state = MenuState::Closed;
                    return;
                }
                (RfProtocol::Cc2500, 3) => {
                    buzzer.click();
                    storage.active_model.rf_power = if storage.active_model.rf_power == 0 {
                        1
                    } else {
                        0
                    };
                    storage::save_active_model(storage);
                }
                _ => {}
            }
        }
    } else {
        // Editing Phase
        match ctrl.selected_item {
            0 => {
                if keys.up {
                    let next = ActiveBoard::next_protocol(storage.active_model.rf_protocol);
                    storage.active_model.rf_protocol = next as u8;
                    buzzer.play_tone(2200, 30);
                } else if keys.down {
                    let prev = ActiveBoard::prev_protocol(storage.active_model.rf_protocol);
                    storage.active_model.rf_protocol = prev as u8;
                    buzzer.play_tone(2200, 30);
                }
            }
            1 if current_proto == RfProtocol::Crsf => {
                if keys.up {
                    storage.active_model.crsf_baud = (storage.active_model.crsf_baud + 1) % 5;
                    buzzer.play_tone(2200, 30);
                } else if keys.down {
                    storage.active_model.crsf_baud = if storage.active_model.crsf_baud > 0 {
                        storage.active_model.crsf_baud - 1
                    } else {
                        4
                    };
                    buzzer.play_tone(2200, 30);
                }
            }
            _ => {}
        }

        if keys.ok {
            ctrl.editing = false;
            storage::save_storage(storage);
            buzzer.click();
        }
    }

    widgets::draw_header(lcd, "PROTOCOL SETUP");

    match current_proto {
        RfProtocol::Afhds2a => {
            for slot in 0..4 {
                let idx = ctrl.scroll_offset + slot;
                if idx >= item_count {
                    break;
                }
                render_afhds_row(
                    lcd,
                    slot,
                    idx,
                    idx == ctrl.selected_item,
                    ctrl.editing,
                    storage,
                );
            }
            let footer = if ctrl.editing {
                "[OK] Save   [UP/DN] Change"
            } else if ctrl.selected_item == 1 {
                "[OK] Start Bind   [ESC] Exit"
            } else if ctrl.selected_item >= 2 {
                "[OK] Toggle/Cycle [ESC] Exit"
            } else {
                "[OK] Edit   [ESC] Exit"
            };
            widgets::draw_footer(lcd, footer);
        }
        RfProtocol::Crsf => {
            for slot in 0..4 {
                let idx = ctrl.scroll_offset + slot;
                if idx >= item_count {
                    break;
                }
                render_crsf_row(
                    lcd,
                    slot,
                    idx,
                    idx == ctrl.selected_item,
                    ctrl.editing,
                    storage,
                );
            }
            let footer = if ctrl.editing {
                "[OK] Save   [UP/DN] Change"
            } else if ctrl.selected_item == 2 || ctrl.selected_item == 3 {
                "[OK] Toggle Mode  [ESC] Exit"
            } else if ctrl.selected_item == 4 {
                "[OK] Open Config  [ESC] Exit"
            } else {
                "[OK] Edit   [ESC] Exit"
            };
            widgets::draw_footer(lcd, footer);
        }
        RfProtocol::Cc2500 => {
            for slot in 0..4 {
                let idx = ctrl.scroll_offset + slot;
                if idx >= item_count {
                    break;
                }
                match idx {
                    0 => {
                        let proto_val = if ctrl.editing && idx == ctrl.selected_item {
                            "[CC2500]"
                        } else {
                            "CC2500"
                        };
                        widgets::draw_list_row_right(
                            lcd,
                            slot,
                            idx == ctrl.selected_item,
                            "Proto:",
                            Some(proto_val),
                        );
                    }
                    1 => {
                        let proto_mode = crate::rf::mpm::Cc2500Protocol::from_u8(
                            storage.active_model.rx_serial_proto,
                        );
                        widgets::draw_list_row_right(
                            lcd,
                            slot,
                            idx == ctrl.selected_item,
                            "Mode:",
                            Some(proto_mode.name()),
                        );
                    }
                    2 => widgets::draw_list_row_right(
                        lcd,
                        slot,
                        idx == ctrl.selected_item,
                        "[Bind Receiver]",
                        None,
                    ),
                    3 => {
                        let pwr_str = if storage.active_model.rf_power == 0 {
                            "100mW (HI)"
                        } else {
                            "10mW (LOW)"
                        };
                        widgets::draw_list_row_right(
                            lcd,
                            slot,
                            idx == ctrl.selected_item,
                            "Power:",
                            Some(pwr_str),
                        );
                    }
                    _ => {}
                }
            }
            widgets::draw_footer(lcd, "[OK] Select   [ESC] Exit");
        }
        RfProtocol::Dsm => {
            for slot in 0..4 {
                let idx = ctrl.scroll_offset + slot;
                if idx >= item_count {
                    break;
                }
                match idx {
                    0 => {
                        let proto_val = if ctrl.editing && idx == ctrl.selected_item {
                            "[SPEKTRUM DSM]"
                        } else {
                            "SPEKTRUM DSM"
                        };
                        widgets::draw_list_row_right(
                            lcd,
                            slot,
                            idx == ctrl.selected_item,
                            "Proto:",
                            Some(proto_val),
                        );
                    }
                    1 => {
                        let sub = DsmSubProtocol::from_u8(storage.active_model.dsm_sub_proto);
                        widgets::draw_list_row_right(
                            lcd,
                            slot,
                            idx == ctrl.selected_item,
                            "Mode:",
                            Some(sub.name()),
                        );
                    }
                    2 => {
                        widgets::draw_list_row_right(
                            lcd,
                            slot,
                            idx == ctrl.selected_item,
                            "[Bind DSM Receiver]",
                            None,
                        );
                    }
                    3 => {
                        let pwr = if storage.active_model.rf_power == 0 {
                            "NORMAL"
                        } else {
                            "RANGE TEST"
                        };
                        widgets::draw_list_row_right(
                            lcd,
                            slot,
                            idx == ctrl.selected_item,
                            "Power:",
                            Some(pwr),
                        );
                    }
                    4 => {
                        widgets::draw_list_row_right(
                            lcd,
                            slot,
                            idx == ctrl.selected_item,
                            "[Forward Prog]",
                            Some(">"),
                        );
                    }
                    _ => {}
                }
            }
            let footer = if ctrl.editing {
                "[OK] Save   [UP/DN] Change"
            } else if ctrl.selected_item == 1 || ctrl.selected_item == 3 {
                "[OK] Cycle Mode   [ESC] Exit"
            } else if ctrl.selected_item == 2 {
                "[OK] Start Bind   [ESC] Exit"
            } else if ctrl.selected_item == 4 {
                "[OK] Open Config  [ESC] Exit"
            } else {
                "[OK] Edit   [ESC] Exit"
            };
            widgets::draw_footer(lcd, footer);
        }
    }
}

const CRSF_BAUD_NAMES: [&str; 5] = [
    "115.2k (Low)",
    "416.6k (TBS)",
    "420k (ELRS)",
    "921.6k (Fast)",
    "1.875M (Max)",
];

#[inline(never)]
fn render_afhds_row(
    lcd: &mut St7567,
    slot: usize,
    idx: usize,
    is_sel: bool,
    editing: bool,
    storage: &RadioStorage,
) {
    match idx {
        0 => {
            let val_str = if editing && is_sel {
                "[AFHDS 2A]"
            } else {
                "AFHDS 2A"
            };
            widgets::draw_list_row_right(lcd, slot, is_sel, "Proto:", Some(val_str));
        }
        1 => {
            let mut hex_buf = [b'0'; 8];
            u32_to_hex(storage.active_model.rx_id, &mut hex_buf);
            let mut bind_buf = *b"[Bind: 00000000]";
            bind_buf[7..15].copy_from_slice(&hex_buf);
            let bind_str = ascii_as_str(&bind_buf);
            widgets::draw_list_row_right(lcd, slot, is_sel, bind_str, None);
        }
        2 => {
            let pwr_str = match storage.active_model.rf_power {
                0 => "100mW (HI)",
                1 => "10mW (MID)",
                2 => "1mW (LOW)",
                3 => "100uW (TST)",
                _ => "100mW (HI)",
            };
            widgets::draw_list_row_right(lcd, slot, is_sel, "Power:", Some(pwr_str));
        }
        3 => {
            let mut hz_buf = [0u8; 8];
            let hz_str = format_servo_hz(storage.active_model.servo_rate_hz, &mut hz_buf);
            widgets::draw_list_row_right(lcd, slot, is_sel, "Servo Hz:", Some(hz_str));
        }
        4 => {
            let out_str = if storage.active_model.rx_out_mode == 0 {
                "PWM"
            } else {
                "PPM"
            };
            widgets::draw_list_row_right(lcd, slot, is_sel, "RX Out:", Some(out_str));
        }
        5 => {
            let serial_str = if storage.active_model.rx_serial_proto == 0 {
                "i-BUS"
            } else {
                "S.BUS"
            };
            widgets::draw_list_row_right(lcd, slot, is_sel, "Serial:", Some(serial_str));
        }
        _ => {}
    }
}

#[inline(never)]
fn render_crsf_row(
    lcd: &mut St7567,
    slot: usize,
    idx: usize,
    is_sel: bool,
    editing: bool,
    storage: &RadioStorage,
) {
    match idx {
        0 => {
            let proto_val = if editing && is_sel { "[CRSF]" } else { "CRSF" };
            widgets::draw_list_row_right(lcd, slot, is_sel, "Proto:", Some(proto_val));
        }
        1 => {
            let baud_idx = (storage.active_model.crsf_baud as usize).min(CRSF_BAUD_NAMES.len() - 1);
            let name = CRSF_BAUD_NAMES[baud_idx];
            if editing && is_sel {
                let mut b_buf = [0u8; 20];
                b_buf[0] = b'[';
                let len = name.len().min(18);
                b_buf[1..1 + len].copy_from_slice(&name.as_bytes()[..len]);
                b_buf[1 + len] = b']';
                let s = ascii_as_str(&b_buf[..2 + len]);
                widgets::draw_list_row_right(lcd, slot, is_sel, "Baud:", Some(s));
            } else {
                widgets::draw_list_row_right(lcd, slot, is_sel, "Baud:", Some(name));
            }
        }
        2 => {
            let duplex_val = if storage.active_model.crsf_half_duplex == 0 {
                "Full (2W)"
            } else {
                "Half (1W)"
            };
            widgets::draw_list_row_right(lcd, slot, is_sel, "Duplex:", Some(duplex_val));
        }
        3 => {
            let arm_val = if storage.active_model.crsf_arm_mode == 0 {
                "CH5 (Std)"
            } else {
                "Switch"
            };
            widgets::draw_list_row_right(lcd, slot, is_sel, "Arm Mode:", Some(arm_val));
        }
        4 => {
            widgets::draw_list_row_right(lcd, slot, is_sel, "[Configure Module]", None);
        }
        _ => {}
    }
}
