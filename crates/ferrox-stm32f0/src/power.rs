//! Electronic power management subsystem for FlySky FS-i6S on STM32F072.
//!
//! Pin mapping:
//! - PB15: Hardware power latch (Push-Pull, High = Latch Power ON)
//! - PB14: Dual power button sense line (Input with Pull-Up, Active LOW)
//! - PD10, PD11: Power button blue illumination LEDs (Push-Pull, High = ON)

use ferrox_core::hal::PowerDriver;
use stm32f0xx_hal::pac;

pub struct Stm32f0PowerDriver;

impl Stm32f0PowerDriver {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for Stm32f0PowerDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl PowerDriver for Stm32f0PowerDriver {
    fn init(&mut self) {
        unsafe {
            let rcc = &*pac::RCC::ptr();
            let gpiob = &*pac::GPIOB::ptr();
            let gpiod = &*pac::GPIOD::ptr();

            // Enable GPIOB (bit 18) and GPIOD (bit 20) in RCC_AHBENR
            rcc.ahbenr
                .modify(|r, w| w.bits(r.bits() | (1 << 18) | (1 << 20)));

            // Configure PB15 as push-pull output (MODER bits 31:30 = 01)
            gpiob.moder.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !(3 << 30)) | (1 << 30))
            });

            // Latch power ON immediately by driving PB15 HIGH
            gpiob.bsrr.write(|w| w.bits(1 << 15));

            // Configure PB14 as input (MODER bits 29:28 = 00) with pull-up (PUPDR bits 29:28 = 01)
            gpiob.moder.modify(|r, w| w.bits(r.bits() & !(3 << 28)));
            gpiob.pupdr.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !(3 << 28)) | (1 << 28))
            });

            // Configure PD10 and PD11 as push-pull outputs
            gpiod.moder.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !(0x0F << 20)) | (0x05 << 20))
            });

            // Turn on power button blue LEDs
            gpiod.bsrr.write(|w| w.bits((1 << 10) | (1 << 11)));
        }
    }

    fn latch_on(&mut self) {
        unsafe {
            let rcc = &*pac::RCC::ptr();
            let gpiob = &*pac::GPIOB::ptr();

            rcc.ahbenr.modify(|r, w| w.bits(r.bits() | (1 << 18)));
            gpiob.moder.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !(3 << 30)) | (1 << 30))
            });
            gpiob.bsrr.write(|w| w.bits(1 << 15));
        }
    }

    fn latch_off(&mut self) {
        unsafe {
            let gpiob = &*pac::GPIOB::ptr();
            gpiob.bsrr.write(|w| w.bits(1 << 31));
            loop {
                cortex_m::asm::wfi();
            }
        }
    }

    fn is_power_key_pressed(&mut self) -> bool {
        unsafe {
            let gpiob = &*pac::GPIOB::ptr();
            (gpiob.idr.read().bits() & (1 << 14)) == 0
        }
    }

    fn set_leds(&mut self, on: bool) {
        unsafe {
            let gpiod = &*pac::GPIOD::ptr();
            if on {
                gpiod.bsrr.write(|w| w.bits((1 << 10) | (1 << 11)));
            } else {
                gpiod.bsrr.write(|w| w.bits(((1 << 10) | (1 << 11)) << 16));
            }
        }
    }
}
