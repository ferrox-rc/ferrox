use core::marker::PhantomData;
use ferrox_core::hal::PowerDriver;
use stm32f4::stm32f407 as pac;

use crate::descriptor::{PocketHal, Qx7HalDescriptor};

pub struct Stm32f4PowerDriver<H: Qx7HalDescriptor = PocketHal> {
    _phantom: PhantomData<H>,
}

impl<H: Qx7HalDescriptor> Stm32f4PowerDriver<H> {
    pub const fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<H: Qx7HalDescriptor> Default for Stm32f4PowerDriver<H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H: Qx7HalDescriptor> PowerDriver for Stm32f4PowerDriver<H> {
    fn init(&mut self) {
        unsafe {
            let rcc = &*pac::RCC::ptr();
            let gpiod = &*pac::GPIOD::ptr();

            // Enable GPIOD (bit 3) in RCC_AHB1ENR
            rcc.ahb1enr.modify(|r, w| w.bits(r.bits() | (1 << 3)));

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

            // Configure Status LED and turn it on to indicate power-on
            H::LED_STATUS.enable_clock_and_output(true);
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

            // Turn off status LED
            H::LED_STATUS.set_low();

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
        if on {
            H::LED_STATUS.set_high();
        } else {
            H::LED_STATUS.set_low();
        }
    }
}
