use core::marker::PhantomData;
use ferrox_core::hal::PowerDriver;

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
        // Latch power ON immediately (PD0 on Q X7/Pocket/Zorro, PD1 on GX12)
        H::PWR_ON_PIN.enable_clock_and_output(true);

        // Configure power switch input with pull-up (PD1 on Q X7/Pocket/Zorro, PD0 on GX12)
        H::PWR_SWITCH_PIN.enable_clock_and_input_pullup();

        // Configure Status LED and turn it on to indicate power-on
        if let Some(led) = H::LED_STATUS {
            led.enable_clock_and_output(true);
        }
    }

    fn latch_on(&mut self) {
        H::PWR_ON_PIN.enable_clock_and_output(true);
    }

    fn latch_off(&mut self) {
        // Turn off status LED
        if let Some(led) = H::LED_STATUS {
            led.set_low();
        }

        // Release power latch
        H::PWR_ON_PIN.set_low();
        loop {
            cortex_m::asm::wfi();
        }
    }

    fn is_power_key_pressed(&mut self) -> bool {
        // Power switch is active LOW
        H::PWR_SWITCH_PIN.is_low()
    }

    fn set_leds(&mut self, on: bool) {
        if let Some(led) = H::LED_STATUS {
            if on {
                led.set_high();
            } else {
                led.set_low();
            }
        }
    }
}
