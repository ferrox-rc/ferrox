use ferrox_core::hal::KeyDriver;
use ferrox_core::input::SwitchPos;
use stm32f4::stm32f407 as pac;

pub struct PocketKeyDriver {
    prev_enc_state: u8,
}

impl PocketKeyDriver {
    pub const fn new() -> Self {
        Self { prev_enc_state: 0 }
    }
}

impl Default for PocketKeyDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyDriver for PocketKeyDriver {
    fn init(&mut self) {
        let rcc = unsafe { &*pac::RCC::ptr() };
        let gpioa = unsafe { &*pac::GPIOA::ptr() };
        let gpiob = unsafe { &*pac::GPIOB::ptr() };
        let gpioc = unsafe { &*pac::GPIOC::ptr() };
        let gpiod = unsafe { &*pac::GPIOD::ptr() };
        let gpioe = unsafe { &*pac::GPIOE::ptr() };

        unsafe {
            // Enable GPIOA..GPIOE
            rcc.ahb1enr
                .modify(|r, w| w.bits(r.bits() | 0x1F));

            // Configure Navigation Keys as Input with Pull-Up:
            // PA13 (ENTER): Set MODER 0b00, PUPDR 0b01
            gpioa
                .moder
                .modify(|r, w| w.bits(r.bits() & !(0x03 << 26)));
            gpioa
                .pupdr
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 26)) | (0x01 << 26)));

            // PB4 (SYS): Set MODER 0b00, PUPDR 0b01
            gpiob
                .moder
                .modify(|r, w| w.bits(r.bits() & !(0x03 << 8)));
            gpiob
                .pupdr
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 8)) | (0x01 << 8)));

            // PC5 (RTN/EXIT), PC1 (T1 Inc), PC2 (T3 Inc), PC3 (T3 Dec)
            gpioc.moder.modify(|r, w| {
                w.bits(
                    r.bits()
                        & !(
                            (0x03 << 10) // PC5
                            | (0x03 << 2) // PC1
                            | (0x03 << 4) // PC2
                            | (0x03 << 6)
                            // PC3
                        ),
                )
            });
            gpioc.pupdr.modify(|r, w| {
                w.bits(
                    (r.bits()
                        & !(
                            (0x03 << 10)
                                | (0x03 << 2)
                                | (0x03 << 4)
                                | (0x03 << 6)
                        ))
                        | (0x01 << 10)
                        | (0x01 << 2)
                        | (0x01 << 4)
                        | (0x01 << 6),
                )
            });

            // PD2 (TELE), PD3 (PAGE<), PD7 (PAGE>), PD15 (T1 Dec)
            gpiod.moder.modify(|r, w| {
                w.bits(
                    r.bits()
                        & !(
                            (0x03 << 4) // PD2
                            | (0x03 << 6) // PD3
                            | (0x03 << 14) // PD7
                            | (0x03 << 30)
                            // PD15
                        ),
                )
            });
            gpiod.pupdr.modify(|r, w| {
                w.bits(
                    (r.bits()
                        & !(
                            (0x03 << 4)
                                | (0x03 << 6)
                                | (0x03 << 14)
                                | (0x03 << 30)
                        ))
                        | (0x01 << 4)
                        | (0x01 << 6)
                        | (0x01 << 14)
                        | (0x01 << 30),
                )
            });

            // PE11 (MDL), PE3 (T4 Dec), PE4 (T4 Inc), PE5 (T2 Inc), PE6 (T2 Dec), PE7 (SE), PE9 (ENC_A), PE10 (ENC_B)
            gpioe.moder.modify(|r, w| {
                w.bits(
                    r.bits()
                        & !(
                            (0x03 << 22) // PE11
                            | (0x03 << 6) // PE3
                            | (0x03 << 8) // PE4
                            | (0x03 << 10) // PE5
                            | (0x03 << 12) // PE6
                            | (0x03 << 14) // PE7
                            | (0x03 << 18) // PE9
                            | (0x03 << 20)
                            // PE10
                        ),
                )
            });
            gpioe.pupdr.modify(|r, w| {
                w.bits(
                    (r.bits()
                        & !(
                            (0x03 << 22)
                                | (0x03 << 6)
                                | (0x03 << 8)
                                | (0x03 << 10)
                                | (0x03 << 12)
                                | (0x03 << 14)
                                | (0x03 << 18)
                                | (0x03 << 20)
                        ))
                        | (0x01 << 22)
                        | (0x01 << 6)
                        | (0x01 << 8)
                        | (0x01 << 10)
                        | (0x01 << 12)
                        | (0x01 << 14)
                        | (0x01 << 18)
                        | (0x01 << 20),
                )
            });

            // Read initial encoder state
            self.prev_enc_state = ((gpioe.idr.read().bits() >> 9) & 0x03) as u8;
        }
    }

    fn scan_keys(&mut self) -> u16 {
        let (a_idr, b_idr, c_idr, d_idr, e_idr) = unsafe {
            let gpioa = &*pac::GPIOA::ptr();
            let gpiob = &*pac::GPIOB::ptr();
            let gpioc = &*pac::GPIOC::ptr();
            let gpiod = &*pac::GPIOD::ptr();
            let gpioe = &*pac::GPIOE::ptr();
            (
                gpioa.idr.read().bits(),
                gpiob.idr.read().bits(),
                gpioc.idr.read().bits(),
                gpiod.idr.read().bits(),
                gpioe.idr.read().bits(),
            )
        };

        let mut keys = 0u16;

        // Trims (active LOW):
        // T4: Dec = PE3 (bit 0), Inc = PE4 (bit 1)
        if (e_idr & (1 << 3)) == 0 {
            keys |= 1 << 0;
        }
        if (e_idr & (1 << 4)) == 0 {
            keys |= 1 << 1;
        }
        // T3: Dec = PC3 (bit 2), Inc = PC2 (bit 3)
        if (c_idr & (1 << 3)) == 0 {
            keys |= 1 << 2;
        }
        if (c_idr & (1 << 2)) == 0 {
            keys |= 1 << 3;
        }
        // T2: Dec = PE6 (bit 4), Inc = PE5 (bit 5)
        if (e_idr & (1 << 6)) == 0 {
            keys |= 1 << 4;
        }
        if (e_idr & (1 << 5)) == 0 {
            keys |= 1 << 5;
        }
        // T1: Dec = PD15 (bit 6), Inc = PC1 (bit 7)
        if (d_idr & (1 << 15)) == 0 {
            keys |= 1 << 6;
        }
        if (c_idr & (1 << 1)) == 0 {
            keys |= 1 << 7;
        }

        // Rotary Encoder Navigation (PE9/PE10):
        let enc_cur = ((e_idr >> 9) & 0x03) as u8;
        if enc_cur != self.prev_enc_state {
            let transition = (self.prev_enc_state << 2) | enc_cur;
            self.prev_enc_state = enc_cur;
            // Pocket encoder is inverted per EdgeTX pocket.json:
            // 0b0001 or 0b0111 or 0b1110 or 0b1000 = Down (bit 8)
            // 0b0010 or 0b1011 or 0b1101 or 0b0100 = Up (bit 9)
            match transition {
                0b0001 | 0b0111 | 0b1110 | 0b1000 => keys |= 1 << 8, // Down
                0b0010 | 0b1011 | 0b1101 | 0b0100 => keys |= 1 << 9, // Up
                _ => {}
            }
        }

        // Buttons:
        // Down (bit 8): PD7 (PAGE>)
        if (d_idr & (1 << 7)) == 0 {
            keys |= 1 << 8;
        }
        // Up (bit 9): PD3 (PAGE<)
        if (d_idr & (1 << 3)) == 0 {
            keys |= 1 << 9;
        }
        // OK (bit 10): PA13 (ENTER) or PE11 (MDL)
        if (a_idr & (1 << 13)) == 0 || (e_idr & (1 << 11)) == 0 {
            keys |= 1 << 10;
        }
        // Cancel (bit 11): PC5 (RTN/EXIT)
        if (c_idr & (1 << 5)) == 0 {
            keys |= 1 << 11;
        }
        // Bind / System (bit 12): PB4 (SYS) or PD2 (TELE)
        if (b_idr & (1 << 4)) == 0 || (d_idr & (1 << 2)) == 0 {
            keys |= 1 << 12;
        }

        keys
    }

    fn read_ext_switches(&mut self) -> (SwitchPos, SwitchPos) {
        let e_idr = unsafe { (*pac::GPIOE::ptr()).idr.read().bits() };
        // SE: PE7 (active LOW momentary switch)
        let se = if (e_idr & (1 << 7)) == 0 {
            SwitchPos::Down
        } else {
            SwitchPos::Up
        };
        (se, SwitchPos::Up)
    }
}
