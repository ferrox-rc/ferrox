//! ST7567 128×64 Monochrome LCD parallel bus and backlight driver for STM32F072.
//!
//! Pin mapping:
//! - Data bus: PE0..PE7 (8-bit parallel 6800/8080 mode)
//! - Controls: PB3 (RS), PB4 (RST), PB5 (RW), PD2 (CS), PD7 (RD / Strobe)
//! - Backlight: PF3 (Stock active HIGH) / PC9 (TIM3_CH4 PWM dimming mod)

use ferrox_core::display::st7567::WIDTH;
use ferrox_core::hal::DisplayBus;
use stm32f0xx_hal::pac;

pub struct St7567ParallelBus;

impl St7567ParallelBus {
    pub const fn new() -> Self {
        Self
    }

    #[inline(always)]
    fn delay_cycles(&self, count: u32) {
        cortex_m::asm::delay(count);
    }

    #[inline]
    fn write_byte(&self, byte: u8, is_data: bool) {
        let gpiob = unsafe { &*pac::GPIOB::ptr() };
        let gpiod = unsafe { &*pac::GPIOD::ptr() };
        let gpioe = unsafe { &*pac::GPIOE::ptr() };

        unsafe {
            if is_data {
                gpiob.bsrr.write(|w| w.bits(1 << 3));
            } else {
                gpiob.bsrr.write(|w| w.bits(1 << (3 + 16)));
            }

            gpioe
                .odr
                .modify(|r, w| w.bits((r.bits() & !0xFF) | (byte as u32)));

            gpiod.bsrr.write(|w| w.bits(1 << 7));
            cortex_m::asm::nop();
            gpiod.bsrr.write(|w| w.bits(1 << (7 + 16)));
        }
    }

    #[inline]
    pub fn write_cmd(&self, cmd: u8) {
        self.write_byte(cmd, false);
    }
}

impl Default for St7567ParallelBus {
    fn default() -> Self {
        Self::new()
    }
}

impl DisplayBus for St7567ParallelBus {
    fn init(&mut self) {
        let rcc = unsafe { &*pac::RCC::ptr() };
        let gpiob = unsafe { &*pac::GPIOB::ptr() };
        let gpioc = unsafe { &*pac::GPIOC::ptr() };
        let gpiod = unsafe { &*pac::GPIOD::ptr() };
        let gpioe = unsafe { &*pac::GPIOE::ptr() };
        let gpiof = unsafe { &*pac::GPIOF::ptr() };
        let tim3 = unsafe { &*pac::TIM3::ptr() };

        unsafe {
            // Enable GPIOB, GPIOC, GPIOD, GPIOE, GPIOF in RCC_AHBENR
            rcc.ahbenr.modify(|r, w| {
                w.bits(r.bits() | (1 << 18) | (1 << 19) | (1 << 20) | (1 << 21) | (1 << 22))
            });

            // Configure PE0..PE7 as outputs
            gpioe
                .moder
                .modify(|r, w| w.bits((r.bits() & !0x0000_FFFF) | 0x0000_5555));

            // PB3 (RS), PB4 (RST), PB5 (RW) as outputs
            let b_mask = (3 << 6) | (3 << 8) | (3 << 10);
            let b_val = (1 << 6) | (1 << 8) | (1 << 10);
            gpiob
                .moder
                .modify(|r, w| w.bits((r.bits() & !b_mask) | b_val));

            // PD2 (CS), PD7 (RD / Strobe) as outputs
            let d_mask = (3 << 4) | (3 << 14);
            let d_val = (1 << 4) | (1 << 14);
            gpiod
                .moder
                .modify(|r, w| w.bits((r.bits() & !d_mask) | d_val));

            // Standard Backlight on PF3
            gpiof
                .moder
                .modify(|r, w| w.bits((r.bits() & !(3 << 6)) | (1 << 6)));

            // Default states
            gpiod.bsrr.write(|w| w.bits((1 << (2 + 16)) | (1 << 7)));
            gpiob
                .bsrr
                .write(|w| w.bits((1 << (5 + 16)) | (1 << 4) | (1 << 3)));

            // Backlight PWM on PC9 (TIM3_CH4)
            rcc.apb1enr.modify(|r, w| w.bits(r.bits() | (1 << 1)));
            gpioc
                .moder
                .modify(|r, w| w.bits((r.bits() & !(3 << 18)) | (2 << 18)));
            gpioc.afrh.modify(|r, w| w.bits(r.bits() & !(0xF << 4)));

            tim3.psc.write(|w| w.bits(47));
            tim3.arr.write(|w| w.bits(999));
            tim3.ccr4.write(|w| w.bits(999));
            tim3.ccmr2_output()
                .modify(|r, w| w.bits((r.bits() & !(0x7F << 8)) | (6 << 12) | (1 << 11)));
            tim3.ccer.modify(|r, w| w.bits(r.bits() | (1 << 12)));
            tim3.egr.write(|w| w.bits(1));
            tim3.cr1
                .modify(|r, w| w.bits(r.bits() | (1 << 7) | (1 << 0)));

            // Reset controller
            gpiob.bsrr.write(|w| w.bits(1 << (4 + 16)));
            self.delay_cycles(500);
            gpiob.bsrr.write(|w| w.bits(1 << 4));
            self.delay_cycles(500);
        }

        self.write_cmd(0xE2); // LCD_CMD_RESET
        self.delay_cycles(10_000);
        self.write_cmd(0xAE); // DISPLAY_OFF
        self.write_cmd(0xA4); // MODE_RAM
        self.write_cmd(0xA3); // BIAS_1_7
        self.write_cmd(0xC0); // COM_NORMAL
        self.write_cmd(0xA1); // SEG_INVERSE
        self.write_cmd(0x2F); // POWERCTRL_ALL_ON
        self.write_cmd(0x23); // REG_RATIO_011
        self.write_cmd(0x81); // EV
        self.write_cmd(0x25); // Default contrast
        self.write_cmd(0x40); // STARTLINE 0
        self.write_cmd(0xB0); // PAGESTART 0
        self.write_cmd(0x04); // COL_LO 4
        self.write_cmd(0x10); // COL_HI 0
        self.write_cmd(0xAF); // DISPLAY_ON
        self.delay_cycles(10_000);
    }

    fn flush(&mut self, framebuffer: &[u8; 1024]) {
        let gpiob = unsafe { &*pac::GPIOB::ptr() };
        let gpiod = unsafe { &*pac::GPIOD::ptr() };

        unsafe {
            let data_odr_u8 = core::ptr::addr_of!((*pac::GPIOE::ptr()).odr) as *mut u8;

            for page in 0..8 {
                // Command mode: PB3 Low
                gpiob.bsrr.write(|w| w.bits(1 << (3 + 16)));

                // Page selection (0xB0..0xB7)
                core::ptr::write_volatile(data_odr_u8, 0xB0 | page as u8);
                gpiod.bsrr.write(|w| w.bits(1 << 7));
                gpiod.bsrr.write(|w| w.bits(1 << (7 + 16)));

                // Column low nibble = 4
                core::ptr::write_volatile(data_odr_u8, 0x04);
                gpiod.bsrr.write(|w| w.bits(1 << 7));
                gpiod.bsrr.write(|w| w.bits(1 << (7 + 16)));

                // Column high nibble = 0
                core::ptr::write_volatile(data_odr_u8, 0x10);
                gpiod.bsrr.write(|w| w.bits(1 << 7));
                gpiod.bsrr.write(|w| w.bits(1 << (7 + 16)));

                // Data mode: PB3 High
                gpiob.bsrr.write(|w| w.bits(1 << 3));

                let start = page * WIDTH;
                let end = start + WIDTH;
                for &byte in &framebuffer[start..end] {
                    core::ptr::write_volatile(data_odr_u8, byte);
                    gpiod.bsrr.write(|w| w.bits(1 << 7));
                    gpiod.bsrr.write(|w| w.bits(1 << (7 + 16)));
                }
            }
        }
    }

    fn set_contrast(&mut self, contrast: u8) {
        let val = contrast.min(63);
        self.write_cmd(0x81);
        self.write_cmd(val);
    }

    fn set_backlight_level(&mut self, pct: u8) {
        let gpiof = unsafe { &*pac::GPIOF::ptr() };
        let tim3 = unsafe { &*pac::TIM3::ptr() };

        unsafe {
            if pct == 0 {
                gpiof.bsrr.write(|w| w.bits(1 << (3 + 16)));
                tim3.ccr4.write(|w| w.bits(0));
            } else {
                gpiof.bsrr.write(|w| w.bits(1 << 3));
                let duty = (pct.min(100) as u32 * 999) / 100;
                tim3.ccr4.write(|w| w.bits(duty));
            }
        }
    }
}
