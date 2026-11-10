use ferrox_core::hal::PowerDriver;
use stm32f4::stm32f407 as pac;

pub struct Stm32f4PowerDriver;

impl Stm32f4PowerDriver {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for Stm32f4PowerDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl PowerDriver for Stm32f4PowerDriver {
    fn init(&mut self) {
        unsafe {
            let rcc = &*pac::RCC::ptr();
            let gpioa = &*pac::GPIOA::ptr();
            let gpiod = &*pac::GPIOD::ptr();
            let gpioe = &*pac::GPIOE::ptr();

            // Enable GPIOA (bit 0), GPIOD (bit 3), GPIOE (bit 4) in RCC_AHB1ENR
            rcc.ahb1enr
                .modify(|r, w| w.bits(r.bits() | (1 << 0) | (1 << 3) | (1 << 4)));

            // Configure PD0 (PWR_ON) as push-pull output (MODER bits 1:0 = 0b01)
            gpiod.moder.modify(|r, w| w.bits((r.bits() & !0x03) | 0x01));
            // Latch power ON immediately
            gpiod.bsrr.write(|w| w.bits(1 << 0));

            // Configure PD1 (PWR_SWITCH) as input (MODER bits 3:2 = 0b00) with pull-up (PUPDR bits 3:2 = 0b01)
            gpiod
                .moder
                .modify(|r, w| w.bits(r.bits() & !(0x03 << 2)));
            gpiod
                .pupdr
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 2)) | (0x01 << 2)));

            // Configure Status LEDs: PE2 (Green), PE13 (Red), PA7 (Blue) as push-pull outputs
            gpioe
                .moder
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 4)) | (0x01 << 4))); // PE2
            gpioe
                .moder
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 26)) | (0x01 << 26))); // PE13
            gpioa
                .moder
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 14)) | (0x01 << 14))); // PA7

            // Turn on Green LED (PE2) to indicate system power-on
            gpioe.bsrr.write(|w| w.bits(1 << 2));
        }
    }

    fn latch_on(&mut self) {
        unsafe {
            let rcc = &*pac::RCC::ptr();
            let gpiod = &*pac::GPIOD::ptr();

            rcc.ahb1enr.modify(|r, w| w.bits(r.bits() | (1 << 3)));
            gpiod.moder.modify(|r, w| w.bits((r.bits() & !0x03) | 0x01));
            gpiod.bsrr.write(|w| w.bits(1 << 0));
        }
    }

    fn latch_off(&mut self) {
        unsafe {
            let gpiod = &*pac::GPIOD::ptr();
            let gpioe = &*pac::GPIOE::ptr();

            // Turn off green/red LEDs
            gpioe.bsrr.write(|w| w.bits((1 << (2 + 16)) | (1 << (13 + 16))));

            // Release PD0 power latch
            gpiod.bsrr.write(|w| w.bits(1 << (0 + 16)));
            loop {
                cortex_m::asm::wfi();
            }
        }
    }

    fn is_power_key_pressed(&mut self) -> bool {
        unsafe {
            let gpiod = &*pac::GPIOD::ptr();
            // PD1 is active LOW
            (gpiod.idr.read().bits() & (1 << 1)) == 0
        }
    }

    fn set_leds(&mut self, on: bool) {
        unsafe {
            let gpioe = &*pac::GPIOE::ptr();
            if on {
                gpioe.bsrr.write(|w| w.bits(1 << 2));
            } else {
                gpioe.bsrr.write(|w| w.bits(1 << (2 + 16)));
            }
        }
    }
}
