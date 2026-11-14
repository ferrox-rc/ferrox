//! Analog diagnostics and System Information screens.

use crate::adc;
use crate::board::{ActiveBoard, BoardProfile, DiagPage};
use crate::buzzer::Buzzer;
use crate::chip;
use crate::display::St7567;
use crate::input::SwitchPos;
use crate::menu::format::{ascii_as_str, u16_to_dec_4};
use crate::menu::widgets;
use crate::menu::{MenuController, NavKeys};

#[inline(never)]
pub fn update_diag_anas(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    raw_adc: &[u16; adc::NUM_CHANNELS],
    switches: &crate::input::Switches,
    stick_mode: u8,
    buzzer: &mut Buzzer,
) {
    if keys.cancel {
        ctrl.return_to_main_menu();
        buzzer.click();
        return;
    }

    let pages = ActiveBoard::DIAG_PAGES;
    let page_count = pages.len();
    if page_count == 0 {
        return;
    }

    if keys.up {
        ctrl.page_idx = if ctrl.page_idx == 0 {
            page_count - 1
        } else {
            ctrl.page_idx - 1
        };
        buzzer.play_tone(2200, 30);
    } else if keys.down || keys.ok {
        ctrl.page_idx = (ctrl.page_idx + 1) % page_count;
        buzzer.play_tone(2200, 30);
    }

    let current_page = pages[ctrl.page_idx];

    match current_page {
        DiagPage::Analog1To6 => {
            widgets::draw_header(lcd, "ANALOG (1-6)");
            let names = match crate::safety::StickMode::from_u8(stick_mode) {
                crate::safety::StickMode::Mode1 => {
                    ["RH:AIL", "RV:THR", "LV:ELE", "LH:RUD", "SW:SA ", "SW:SB "]
                }
                crate::safety::StickMode::Mode2 => {
                    ["RH:AIL", "RV:ELE", "LV:THR", "LH:RUD", "SW:SA ", "SW:SB "]
                }
                crate::safety::StickMode::Mode3 => {
                    ["RH:RUD", "RV:THR", "LV:ELE", "LH:AIL", "SW:SA ", "SW:SB "]
                }
                crate::safety::StickMode::Mode4 => {
                    ["RH:RUD", "RV:ELE", "LV:THR", "LH:AIL", "SW:SA ", "SW:SB "]
                }
            };
            for (i, &name) in names.iter().enumerate() {
                let y = 13 + (i as i32 * 7);
                let raw = raw_adc[i].min(4095);
                let fill_w = ((raw as u32 * 38) / 4095).min(38);
                widgets::draw_gauge_row(lcd, y, name, fill_w, raw);
            }
        }
        DiagPage::Analog7To11 => {
            widgets::draw_header(lcd, "ANALOG (7-11)");
            let names = ["POT:V1", "POT:V2", "SW:SC ", "SW:SD ", "VBAT  "];
            for (i, &name) in names.iter().enumerate() {
                let adc_idx = 6 + i;
                let y = 13 + (i as i32 * 7);
                let raw = raw_adc[adc_idx].min(4095);
                let fill_w = ((raw as u32 * 38) / 4095).min(38);
                widgets::draw_gauge_row(lcd, y, name, fill_w, raw);
            }
        }
        DiagPage::ExtSwitches => {
            widgets::draw_header(lcd, ActiveBoard::DIAG_SW_TITLE);

            #[cfg(not(feature = "board-i6x"))]
            {
                let items = [
                    ("SW:SE", switches.se),
                    ("SW:SF", switches.sf),
                    ("SW:SG", switches.sg),
                    ("SW:SH", switches.sh),
                ];
                for (i, &(label, pos)) in items.iter().enumerate() {
                    let y = 13 + (i as i32 * 9);
                    lcd.draw_str_4x6(2, y, label, false);
                    let (state_str, fill) = match pos {
                        SwitchPos::Up => ("UP (HIGH)", 0),
                        SwitchPos::Mid => ("MID", 15),
                        SwitchPos::Down => ("DN (LOW)", 30),
                    };
                    widgets::draw_bar_gauge(lcd, 32, y, 30, 6, fill);
                    lcd.draw_str_4x6(66, y, state_str, false);
                }
            }

            #[cfg(feature = "board-i6x")]
            {
                let ext_active = crate::input::is_ext_switches_enabled();
                let cfg_label = if ext_active {
                    "EXT SW: ENABLED (PC12/15)"
                } else {
                    "EXT SW: DISABLED (OFF)"
                };
                lcd.draw_str_4x6(2, 13, cfg_label, false);

                // SE (PC12)
                lcd.draw_str_4x6(2, 23, "SW:SE", false);
                let se_state = if switches.se == SwitchPos::Down {
                    "DN (LOW)"
                } else {
                    "UP (HIGH)"
                };
                let fill_se = if switches.se == SwitchPos::Down {
                    30
                } else {
                    0
                };
                widgets::draw_bar_gauge(lcd, 32, 23, 30, 6, fill_se);
                lcd.draw_str_4x6(66, 23, se_state, false);

                // SF (PC15)
                lcd.draw_str_4x6(2, 33, "SW:SF", false);
                let sf_state = if switches.sf == SwitchPos::Down {
                    "DN (LOW)"
                } else {
                    "UP (HIGH)"
                };
                let fill_sf = if switches.sf == SwitchPos::Down {
                    30
                } else {
                    0
                };
                widgets::draw_bar_gauge(lcd, 32, 33, 30, 6, fill_sf);
                lcd.draw_str_4x6(66, 33, sf_state, false);

                lcd.draw_str_4x6(2, 44, "Active-LOW mod to GND", false);
            }
        }
        DiagPage::ExtAnalog => {
            widgets::draw_header(lcd, ActiveBoard::DIAG_EXT_ADC_TITLE);
            let ext_active = crate::input::is_ext_adc_enabled();

            let cfg_label = if ext_active {
                ActiveBoard::DIAG_EXT_ADC_EN_HDR
            } else {
                ActiveBoard::DIAG_EXT_ADC_DIS_HDR
            };
            lcd.draw_str_4x6(2, 13, cfg_label, false);

            let names = ActiveBoard::DIAG_EXT_ADC_NAMES;
            for (i, &name) in names.iter().enumerate() {
                let adc_idx = 11 + i;
                let y = 23 + (i as i32 * 8);
                lcd.draw_str_4x6(2, y, name, false);

                let is_off = crate::input::get_adc_mode(6 + i).is_off();
                let raw = raw_adc[adc_idx].min(4095);
                let fill_w = if ext_active && !is_off {
                    ((raw as u32 * 38) / 4095).min(38)
                } else {
                    0
                };
                widgets::draw_bar_gauge(lcd, 44, y + 1, 40, 5, fill_w);

                let mut val_buf = [0u8; 4];
                if ext_active && !is_off {
                    u16_to_dec_4(raw, &mut val_buf);
                } else {
                    val_buf = *b" OFF";
                }
                let val_str = ascii_as_str(&val_buf);
                lcd.draw_str_4x6(90, y, val_str, false);
            }
        }
        DiagPage::EepromStorage => {
            widgets::draw_header(lcd, "STORAGE (EEPROM)");
            let status = crate::storage::get_eeprom_status();
            if status.detected {
                let bus_str = match status.bus {
                    2 => "BUS: I2C2 (PB10/PB11 Fm+)",
                    1 => "BUS: I2C1 (PB8/PB9)",
                    _ => "BUS: Unknown",
                };
                lcd.draw_str_4x6(2, 13, bus_str, false);

                let mut addr_buf = *b"ADDR: 0x50 (FT24C128A 16KB)";
                addr_buf[8] = b"0123456789ABCDEF"[(status.addr >> 4) as usize];
                addr_buf[9] = b"0123456789ABCDEF"[(status.addr & 0x0F) as usize];
                lcd.draw_str_4x6(2, 23, ascii_as_str(&addr_buf), false);

                let state_str = if status.write_err {
                    "STATUS: WRITE ERR (FAIL)"
                } else if status.read_ok {
                    "STATUS: HEALTHY (OK)"
                } else {
                    "STATUS: READ ERR (FAIL)"
                };
                lcd.draw_str_4x6(2, 33, state_str, false);
            } else {
                lcd.draw_str_4x6(2, 13, "STATUS: NO EEPROM FOUND", false);
            }
        }
    }

    widgets::draw_footer(lcd, "[UP/DN/OK] Page  [ESC] Back");
}

#[inline(never)]
pub fn update_system_info(
    ctrl: &mut MenuController,
    lcd: &mut St7567,
    keys: &NavKeys,
    buzzer: &mut Buzzer,
) {
    if keys.cancel {
        ctrl.return_to_main_menu();
        buzzer.click();
        return;
    }

    widgets::draw_header(lcd, "SYSTEM INFORMATION");

    lcd.draw_str_4x6(4, 14, "MCU:      ", false);
    lcd.draw_str_4x6(48, 14, chip::mcu_name(), false);

    lcd.draw_str_4x6(4, 21, "Firmware: ", false);
    lcd.draw_str_4x6(
        48,
        21,
        concat!(env!("FIRMWARE_VERSION"), " (", env!("GIT_HASH"), ")"),
        false,
    );

    lcd.draw_str_4x6(4, 28, "Flash: 128KB | SRAM: 16KB", false);

    let status = crate::storage::get_eeprom_status();
    let mut st_buf = *b"Storage:  xxxx @ 0x00 (OK) ";
    if status.detected {
        let bus = match status.bus {
            2 => b"I2C2",
            1 => b"I2C1",
            _ => b"I2C?",
        };
        st_buf[10..14].copy_from_slice(bus);
        st_buf[19] = b"0123456789ABCDEF"[(status.addr >> 4) as usize];
        st_buf[20] = b"0123456789ABCDEF"[(status.addr & 0x0F) as usize];
        let state = if status.write_err {
            b"(ERR)"
        } else if status.read_ok {
            b"(OK) "
        } else {
            b"(ERR)"
        };
        st_buf[22..27].copy_from_slice(state);
        lcd.draw_str_4x6(4, 35, ascii_as_str(&st_buf[..27]), false);
    } else {
        lcd.draw_str_4x6(4, 35, "Storage:  None (No EEPROM)", false);
    }

    lcd.draw_str_4x6(4, 42, "Profiles: 20 Models (16KB)", false);
    lcd.draw_str_4x6(4, 49, "Bootloader: [OK] Reboot DFU", false);

    if keys.ok {
        buzzer.play_tone(2400, 100);
        crate::ui::splash::draw_bootloader(lcd);
        lcd.flush();
        #[cfg(not(test))]
        {
            let start = crate::time::millis();
            let mut last = start;
            loop {
                crate::watchdog::feed();
                let now = crate::time::millis();
                let dt = now.wrapping_sub(last) as u16;
                if dt > 0 {
                    last = now;
                    buzzer.tick(dt);
                }
                if now.wrapping_sub(start) >= 120 {
                    break;
                }
            }
        }
        crate::boot::reboot_to_dfu();
    }

    widgets::draw_footer(lcd, "[OK] DFU Mode  [ESC] Back");
}
