use core::marker::PhantomData;
use ferrox_core::hal::DisplayBus;
use stm32f4::stm32f407 as pac;

use crate::descriptor::{PocketHal, Qx7HalDescriptor};

pub struct St7567SpiBus<H: Qx7HalDescriptor = PocketHal> {
    _phantom: PhantomData<H>,
}

impl<H: Qx7HalDescriptor> St7567SpiBus<H> {
    pub const fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }

    #[inline(always)]
    fn delay_cycles(&self, count: u32) {
        cortex_m::asm::delay(count);
    }

    #[inline]
    fn write_byte(&self, byte: u8, is_data: bool) {
        let gpioa = unsafe { &*pac::GPIOA::ptr() };
        let gpioc = unsafe { &*pac::GPIOC::ptr() };
        let spi3 = unsafe { &*pac::SPI3::ptr() };

        unsafe {
            if is_data {
                // PC11 HIGH for Data
                gpioc.bsrr.write(|w| w.bits(1 << 11));
            } else {
                // PC11 LOW for Command
                gpioc.bsrr.write(|w| w.bits(1 << (11 + 16)));
            }

            // Assert NCS (PA15 LOW)
            gpioa.bsrr.write(|w| w.bits(1 << (15 + 16)));

            // Wait until TX buffer empty (TXE = bit 1 in SPI_SR)
            let mut timeout = 50_000u32;
            while (spi3.sr.read().bits() & (1 << 1)) == 0 && timeout > 0 {
                timeout -= 1;
            }
            if timeout > 0 {
                // Drain any stale RXNE before starting transfer (RM0090 28.3.3)
                let _ = spi3.dr.read().bits();
                spi3.dr.write(|w| w.bits(byte as u32));

                // Wait for RXNE (bit 0) — only set after all 8 SCK bits finish
                timeout = 50_000;
                while (spi3.sr.read().bits() & (1 << 0)) == 0 && timeout > 0 {
                    timeout -= 1;
                }
                let _ = spi3.dr.read().bits();
            }

            // Deassert NCS (PA15 HIGH)
            gpioa.bsrr.write(|w| w.bits(1 << 15));
        }
    }

    pub fn write_data_burst(&self, data: &[u8]) {
        let gpioa = unsafe { &*pac::GPIOA::ptr() };
        let gpioc = unsafe { &*pac::GPIOC::ptr() };
        let spi3 = unsafe { &*pac::SPI3::ptr() };

        unsafe {
            gpioc.bsrr.write(|w| w.bits(1 << 11)); // A0 = HIGH (Data)
            gpioa.bsrr.write(|w| w.bits(1 << (15 + 16))); // NCS LOW

            let _ = spi3.dr.read().bits(); // Clear any stale RXNE
            for &byte in data {
                let mut timeout = 50_000u32;
                while (spi3.sr.read().bits() & (1 << 1)) == 0 && timeout > 0 {
                    timeout -= 1;
                }
                if timeout == 0 {
                    break;
                }
                spi3.dr.write(|w| w.bits(byte as u32));

                timeout = 50_000;
                while (spi3.sr.read().bits() & (1 << 0)) == 0 && timeout > 0 {
                    timeout -= 1;
                }
                let _ = spi3.dr.read().bits();
            }

            let mut timeout = 50_000u32;
            while (spi3.sr.read().bits() & (1 << 7)) != 0 && timeout > 0 {
                timeout -= 1;
            }

            gpioa.bsrr.write(|w| w.bits(1 << 15)); // NCS HIGH
        }
    }

    #[inline]
    pub fn write_cmd(&self, cmd: u8) {
        self.write_byte(cmd, false);
    }
}

impl<H: Qx7HalDescriptor> Default for St7567SpiBus<H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H: Qx7HalDescriptor> DisplayBus for St7567SpiBus<H> {
    fn init(&mut self) {
        let rcc = unsafe { &*pac::RCC::ptr() };
        let gpioa = unsafe { &*pac::GPIOA::ptr() };
        let gpioc = unsafe { &*pac::GPIOC::ptr() };
        let gpiod = unsafe { &*pac::GPIOD::ptr() };
        let tim4 = unsafe { &*pac::TIM4::ptr() };
        let spi3 = unsafe { &*pac::SPI3::ptr() };

        unsafe {
            // 1. Enable Clocks: GPIOA, GPIOC, GPIOD (AHB1) + SPI3, TIM4 (APB1)
            rcc.ahb1enr
                .modify(|r, w| w.bits(r.bits() | (1 << 0) | (1 << 2) | (1 << 3)));
            let _ = rcc.ahb1enr.read().bits();
            rcc.apb1enr
                .modify(|r, w| w.bits(r.bits() | (1 << 15) | (1 << 2)));
            let _ = rcc.apb1enr.read().bits();

            // 2. PA15 (NCS): GPIO Output, High Speed
            gpioa
                .moder
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 30)) | (0x01 << 30)));
            gpioa.ospeedr.modify(|r, w| w.bits(r.bits() | (0x03 << 30)));
            gpioa.bsrr.write(|w| w.bits(1 << 15)); // Default HIGH (deselected)

            // 3. PC11 (A0/DC): GPIO Output, High Speed
            gpioc
                .moder
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 22)) | (0x01 << 22)));
            gpioc.ospeedr.modify(|r, w| w.bits(r.bits() | (0x03 << 22)));

            // 4. PD12 (RST): GPIO Output
            gpiod
                .moder
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 24)) | (0x01 << 24)));
            gpiod.bsrr.write(|w| w.bits(1 << 12)); // Default HIGH

            // 5. PC10 (SCK), PC12 (MOSI): AF6 (SPI3), Very High Speed
            gpioc.moder.modify(|r, w| {
                w.bits((r.bits() & !((0x03 << 20) | (0x03 << 24))) | (0x02 << 20) | (0x02 << 24))
            });
            gpioc
                .ospeedr
                .modify(|r, w| w.bits(r.bits() | (0x03 << 20) | (0x03 << 24)));
            gpioc.afrh.modify(|r, w| {
                let mask = (0x0F << 8) | (0x0F << 16);
                let af6 = (0x06 << 8) | (0x06 << 16);
                w.bits((r.bits() & !mask) | af6)
            });

            // 6. Reset SPI3 to clear any bootloader DMA/IRQ state, then configure Master 8-bit
            rcc.apb1rstr.modify(|r, w| w.bits(r.bits() | (1 << 15)));
            rcc.apb1rstr.modify(|r, w| w.bits(r.bits() & !(1 << 15)));
            spi3.cr1.write(|w| w.bits(0));
            spi3.cr2.write(|w| w.bits(0));
            spi3.cr1.write(|w| {
                w.bits(
                    (1 << 9)   // SSM
                    | (1 << 8) // SSI
                    | (1 << 2) // MSTR
                    | (1 << 1) // CPOL
                    | (1 << 0) // CPHA
                    | (0x02 << 3) // BR = /8 (42 MHz / 8 = 5.25 MHz, within 10 MHz ST7567 spec)
                    | (1 << 6), // SPE
                )
            });

            // 7. PD13 (Backlight): AF2 (TIM4_CH2)
            gpiod
                .moder
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 26)) | (0x02 << 26)));
            gpiod
                .afrh
                .modify(|r, w| w.bits((r.bits() & !(0x0F << 20)) | (0x02 << 20)));

            // TIM4 PWM Setup: 84 MHz / 84 = 1 MHz count, ARR = 999 (1 kHz PWM)
            tim4.psc.write(|w| w.bits(83));
            tim4.arr.write(|w| w.bits(999));
            tim4.ccr2().write(|w| w.bits(999)); // Initial 100% duty
            tim4.ccmr1_output()
                .modify(|r, w| w.bits((r.bits() & !(0x7F << 8)) | (0x06 << 12) | (1 << 11)));
            tim4.ccer.modify(|r, w| w.bits(r.bits() | (1 << 4)));
            tim4.egr.write(|w| w.bits(1));
            tim4.cr1.modify(|r, w| w.bits(r.bits() | 0x01)); // CEN

            // 8. Hardware Reset Pulse (ST7567 RC reset + booster settle)
            gpiod.bsrr.write(|w| w.bits(1 << (12 + 16))); // RST LOW
            self.delay_cycles(168_000 * 15); // ~15ms
            gpiod.bsrr.write(|w| w.bits(1 << 12)); // RST HIGH
            self.delay_cycles(168_000 * 150); // ~150ms
        }

        // 9. ST7567 Controller Initialization Sequence
        let seg_cmd = if H::LCD_HORIZ_INVERT { 0xA1 } else { 0xA0 };
        let com_cmd = if H::LCD_VERT_INVERT { 0xC8 } else { 0xC0 };
        let contrast_val = (0x25 as i16 + H::LCD_CONTRAST_OFFSET as i16).clamp(0, 0x3F) as u8;

        self.write_cmd(0xE2); // Soft reset
        self.delay_cycles(168_000 * 10);
        self.write_cmd(seg_cmd);
        self.write_cmd(com_cmd);
        self.write_cmd(0xF8); // Set booster ratio
        self.write_cmd(0x00); // 5x
        self.write_cmd(0xA3); // Bias = 1/6
        self.write_cmd(0x22); // Rb/Ra = 5.0
        self.write_cmd(0x2F); // Power circuits ON
        self.write_cmd(0x81); // Set contrast command
        self.write_cmd(contrast_val);
        self.write_cmd(0xA6); // Normal display (not inverse)
        self.write_cmd(0xAF); // Display ON
        self.delay_cycles(168_000 * 20); // 20ms for internal DC-DC converter startup
    }

    fn flush(&mut self, framebuffer: &[u8]) {
        let col_offset: u8 = if H::LCD_HORIZ_INVERT { 4 } else { 0 };
        let cols_per_page = framebuffer.len() / 8;

        for page in 0..8 {
            self.write_cmd(0x10 | (col_offset >> 4));
            self.write_cmd(0xB0 | page as u8);
            self.write_cmd(col_offset & 0x0F);

            let page_offset = page * cols_per_page;
            self.write_data_burst(&framebuffer[page_offset..page_offset + cols_per_page]);
        }
    }

    fn set_contrast(&mut self, contrast: u8) {
        let val = (contrast.min(63) as i16 + H::LCD_CONTRAST_OFFSET as i16).clamp(0, 63) as u8;
        self.write_cmd(0x81);
        self.write_cmd(val);
    }

    fn set_backlight_level(&mut self, pct: u8) {
        let duty = (pct.min(100) as u32 * 999) / 100;
        let tim4 = unsafe { &*pac::TIM4::ptr() };
        tim4.ccr2().write(|w| unsafe { w.bits(duty) });
    }
}
