//! Key matrix scanning and switch inputs for STM32F072 transmitters.

use ferrox_core::hal::KeyDriver;
use ferrox_core::input::SwitchPos;
use stm32f0xx_hal::pac;

#[inline(always)]
fn delay_cycles(n: u32) {
    cortex_m::asm::delay(n);
}

/// FS-i6X 3x4 Matrix + PF2 Bind + PC12/PC15 External Switches driver.
pub struct I6xKeyDriver;

impl I6xKeyDriver {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for I6xKeyDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyDriver for I6xKeyDriver {
    fn init(&mut self) {
        unsafe {
            let rcc = &*pac::RCC::ptr();
            let gpioc = &*pac::GPIOC::ptr();
            let gpiod = &*pac::GPIOD::ptr();
            let gpiof = &*pac::GPIOF::ptr();

            // Enable GPIOC, GPIOD, GPIOF clocks
            rcc.ahbenr
                .modify(|r, w| w.bits(r.bits() | (1 << 19) | (1 << 20) | (1 << 22)));

            // Configure PC6, PC7, PC8 as outputs (MODER = 01)
            // Configure PC12 (SE) and PC15 (SF) as inputs (MODER = 00)
            gpioc.moder.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !(0x3F << 12) & !(3 << 24) & !(3 << 30)) | (0x15 << 12))
            });

            // Configure PC12 (SE) and PC15 (SF) with internal pull-up (PUPDR = 01)
            gpioc.pupdr.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !(3 << 24) & !(3 << 30)) | (1 << 24) | (1 << 30))
            });

            // Set PC6, PC7, PC8 initially HIGH
            gpioc.bsrr.write(|w| w.bits((1 << 6) | (1 << 7) | (1 << 8)));

            // Configure PD12..PD15 as inputs with pull-ups
            gpiod.moder.modify(|r, w| w.bits(r.bits() & !(0xFF << 24)));
            gpiod.pupdr.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !(0xFF << 24)) | (0x55 << 24))
            });

            // Configure PF2 (Bind button) as input with pull-up
            gpiof.moder.modify(|r, w| w.bits(r.bits() & !(3 << 4)));
            gpiof.pupdr.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !(3 << 4)) | (1 << 4))
            });
        }
    }

    fn scan_keys(&mut self) -> u16 {
        unsafe {
            let mut result = 0u16;
            let gpioc = &*pac::GPIOC::ptr();
            let gpiod = &*pac::GPIOD::ptr();
            let gpiof = &*pac::GPIOF::ptr();

            let cols = [6, 7, 8];
            for (col_idx, &col) in cols.iter().enumerate() {
                gpioc.bsrr.write(|w| w.bits(1 << (col + 16)));
                delay_cycles(150);

                let idr = gpiod.idr.read().bits();
                let lines = ((idr >> 12) & 0x0F) as u8;
                let pressed = (!lines) & 0x0F;

                result |= (pressed as u16) << (col_idx * 4);

                gpioc.bsrr.write(|w| w.bits(1 << col));
                delay_cycles(100);
            }

            // Check Bind button (PF2, active LOW)
            let fidr = gpiof.idr.read().bits();
            if (fidr & (1 << 2)) == 0 {
                result |= 1 << 12;
            }

            result
        }
    }

    fn read_ext_switches(&mut self) -> (SwitchPos, SwitchPos, SwitchPos, SwitchPos) {
        unsafe {
            let gpioc = &*pac::GPIOC::ptr();
            let idr = gpioc.idr.read().bits();
            let sw_e = if (idr & (1 << 12)) == 0 {
                SwitchPos::Down
            } else {
                SwitchPos::Up
            };
            let sw_f = if (idr & (1 << 15)) == 0 {
                SwitchPos::Down
            } else {
                SwitchPos::Up
            };
            (sw_e, sw_f, SwitchPos::Up, SwitchPos::Up)
        }
    }
}

/// FS-i6S Tactile Buttons & J4 Switches driver.
pub struct I6sKeyDriver;

impl I6sKeyDriver {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for I6sKeyDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyDriver for I6sKeyDriver {
    fn init(&mut self) {
        unsafe {
            let rcc = &*pac::RCC::ptr();
            let gpioa = &*pac::GPIOA::ptr();

            rcc.ahbenr.modify(|r, w| w.bits(r.bits() | (1 << 17)));

            // Configure PA9 and PA10 as inputs with pull-ups
            gpioa.moder.modify(|r, w| w.bits(r.bits() & !(0x0F << 18)));
            gpioa.pupdr.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !(0x0F << 18)) | (0x05 << 18))
            });
        }
    }

    fn scan_keys(&mut self) -> u16 {
        unsafe {
            let mut result = 0u16;
            let gpioa = &*pac::GPIOA::ptr();
            let idr = gpioa.idr.read().bits();

            // PA9 = Rear Left (bit 11: Cancel)
            if (idr & (1 << 9)) == 0 {
                result |= 1 << 11;
            }
            // PA10 = Rear Right (bit 10: OK)
            if (idr & (1 << 10)) == 0 {
                result |= 1 << 10;
            }

            result
        }
    }

    fn read_front_buttons(&mut self) -> (bool, bool) {
        unsafe {
            let gpioa = &*pac::GPIOA::ptr();
            let idr = gpioa.idr.read().bits();
            let left = (idr & (1 << 9)) == 0;
            let right = (idr & (1 << 10)) == 0;
            (left, right)
        }
    }

    fn read_j4_buttons(&mut self) -> (bool, bool) {
        unsafe {
            let gpioa = &*pac::GPIOA::ptr();
            let idr = gpioa.idr.read().bits();
            let sg = (idr & (1 << 13)) == 0;
            let sh = (idr & (1 << 14)) == 0;
            (sg, sh)
        }
    }

    fn init_j4_keys(&mut self, enabled: bool) {
        if enabled {
            unsafe {
                let gpioa = &*pac::GPIOA::ptr();
                // PA13: bits 27:26 = 00 (Input), PA14: bits 29:28 = 00 (Input)
                gpioa.moder.modify(|r, w| w.bits(r.bits() & !(0x0F << 26)));
                // PUPDR: PA13 bits 27:26 = 01 (Pull-up), PA14 bits 29:28 = 01 (Pull-up)
                gpioa.pupdr.modify(|r, w| {
                    let val = r.bits();
                    w.bits((val & !(0x0F << 26)) | (0x05 << 26))
                });
            }
        }
    }
}
