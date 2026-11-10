use core::marker::PhantomData;
use ferrox_core::hal::KeyDriver;
use ferrox_core::input::SwitchPos;
use stm32f4::stm32f407 as pac;

use crate::descriptor::{PocketHal, Qx7HalDescriptor};

pub struct Qx7KeyDriver<H: Qx7HalDescriptor = PocketHal> {
    prev_enc_state: u8,
    _phantom: PhantomData<H>,
}

impl<H: Qx7HalDescriptor> Qx7KeyDriver<H> {
    pub const fn new() -> Self {
        Self {
            prev_enc_state: 0,
            _phantom: PhantomData,
        }
    }
}

impl<H: Qx7HalDescriptor> Default for Qx7KeyDriver<H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H: Qx7HalDescriptor> KeyDriver for Qx7KeyDriver<H> {
    fn init(&mut self) {
        let rcc = unsafe { &*pac::RCC::ptr() };
        let gpioe = unsafe { &*pac::GPIOE::ptr() };

        // 1. Enable AHB1 GPIO clocks
        unsafe {
            rcc.ahb1enr.modify(|r, w| w.bits(r.bits() | 0x7F)); // GPIOA..GPIOG
        }

        // 2. Configure Trim Switches
        H::TRIM_T1.0.enable_clock_and_input_pullup();
        H::TRIM_T1.1.enable_clock_and_input_pullup();
        H::TRIM_T2.0.enable_clock_and_input_pullup();
        H::TRIM_T2.1.enable_clock_and_input_pullup();
        H::TRIM_T3.0.enable_clock_and_input_pullup();
        H::TRIM_T3.1.enable_clock_and_input_pullup();
        H::TRIM_T4.0.enable_clock_and_input_pullup();
        H::TRIM_T4.1.enable_clock_and_input_pullup();

        // 3. Configure Navigation Buttons
        H::KEY_ENTER.enable_clock_and_input_pullup();
        H::KEY_EXIT.enable_clock_and_input_pullup();
        H::KEY_PAGE_PREV.enable_clock_and_input_pullup();
        H::KEY_PAGE_NEXT.enable_clock_and_input_pullup();

        if let Some(pin) = H::KEY_SYS {
            pin.enable_clock_and_input_pullup();
        }
        if let Some(pin) = H::KEY_MDL {
            pin.enable_clock_and_input_pullup();
        }
        if let Some(pin) = H::KEY_TELE {
            pin.enable_clock_and_input_pullup();
        }

        // 4. Configure Rotary Encoder Pins on GPIOE: PE9 (Enc A) + PE{ROTENC_PIN_B} (Enc B)
        unsafe {
            let pin_b = H::ROTENC_PIN_B;
            gpioe.moder.modify(|r, w| {
                w.bits(r.bits() & !((0x03 << 18) | (0x03 << (pin_b * 2))))
            });
            gpioe.pupdr.modify(|r, w| {
                w.bits(
                    (r.bits() & !((0x03 << 18) | (0x03 << (pin_b * 2))))
                        | (0x01 << 18)
                        | (0x01 << (pin_b * 2)),
                )
            });

            // Read initial encoder state
            let e_idr = gpioe.idr.read().bits();
            let a = (e_idr >> 9) & 1;
            let b = (e_idr >> pin_b) & 1;
            self.prev_enc_state = (a | (b << 1)) as u8;
        }

        // 5. Configure Extended Switches (SE, SF, SG, SH)
        H::SW_SE.init();
        H::SW_SF.init();
        H::SW_SG.init();
        H::SW_SH.init();
    }

    fn scan_keys(&mut self) -> u16 {
        let mut keys = 0u16;

        // Trims (active LOW):
        // T4: Dec (bit 0), Inc (bit 1)
        if H::TRIM_T4.0.is_low() {
            keys |= 1 << 0;
        }
        if H::TRIM_T4.1.is_low() {
            keys |= 1 << 1;
        }
        // T3: Dec (bit 2), Inc (bit 3)
        if H::TRIM_T3.0.is_low() {
            keys |= 1 << 2;
        }
        if H::TRIM_T3.1.is_low() {
            keys |= 1 << 3;
        }
        // T2: Dec (bit 4), Inc (bit 5)
        if H::TRIM_T2.0.is_low() {
            keys |= 1 << 4;
        }
        if H::TRIM_T2.1.is_low() {
            keys |= 1 << 5;
        }
        // T1: Dec (bit 6), Inc (bit 7)
        if H::TRIM_T1.0.is_low() {
            keys |= 1 << 6;
        }
        if H::TRIM_T1.1.is_low() {
            keys |= 1 << 7;
        }

        // Rotary Encoder Navigation (PE9 / PE{H::ROTENC_PIN_B}):
        unsafe {
            let gpioe = &*pac::GPIOE::ptr();
            let e_idr = gpioe.idr.read().bits();
            let a = (e_idr >> 9) & 1;
            let b = (e_idr >> H::ROTENC_PIN_B) & 1;
            let enc_cur = (a | (b << 1)) as u8;

            if enc_cur != self.prev_enc_state {
                let transition = (self.prev_enc_state << 2) | enc_cur;
                self.prev_enc_state = enc_cur;
                if H::ROTENC_INVERT {
                    match transition {
                        0b0001 | 0b0111 | 0b1110 | 0b1000 => keys |= 1 << 8, // Down
                        0b0010 | 0b1011 | 0b1101 | 0b0100 => keys |= 1 << 9, // Up
                        _ => {}
                    }
                } else {
                    match transition {
                        0b0001 | 0b0111 | 0b1110 | 0b1000 => keys |= 1 << 9, // Up
                        0b0010 | 0b1011 | 0b1101 | 0b0100 => keys |= 1 << 8, // Down
                        _ => {}
                    }
                }
            }
        }

        // Navigation Buttons:
        // Down (bit 8): PAGE> / PAGE
        if H::KEY_PAGE_NEXT.is_low() {
            keys |= 1 << 8;
        }
        // Up (bit 9): PAGE< / MENU
        if H::KEY_PAGE_PREV.is_low() {
            keys |= 1 << 9;
        }
        // OK (bit 10): ENTER or MDL
        if H::KEY_ENTER.is_low() || H::KEY_MDL.map_or(false, |p| p.is_low()) {
            keys |= 1 << 10;
        }
        // Cancel (bit 11): RTN/EXIT
        if H::KEY_EXIT.is_low() {
            keys |= 1 << 11;
        }
        // Bind / System (bit 12): SYS or TELE
        if H::KEY_SYS.map_or(false, |p| p.is_low()) || H::KEY_TELE.map_or(false, |p| p.is_low()) {
            keys |= 1 << 12;
        }

        keys
    }

    fn read_ext_switches(&mut self) -> (SwitchPos, SwitchPos, SwitchPos, SwitchPos) {
        let se = H::SW_SE.read_switch_pos();
        let sf = H::SW_SF.read_switch_pos();
        let sg = H::SW_SG.read_switch_pos();
        let sh = H::SW_SH.read_switch_pos();
        (se, sf, sg, sh)
    }
}

pub type PocketKeyDriver = Qx7KeyDriver<PocketHal>;
