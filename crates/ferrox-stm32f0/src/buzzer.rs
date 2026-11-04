//! Hardware Piezo Buzzer driver using TIM1 Channel 1 on PA8 for STM32F072.
//!
//! Pin PA8 is configured as Alternate Function 2 (TIM1_CH1).
//! TIM1 is clocked at 48 MHz from APB2 with PSC=47 (1 MHz tick).
//! Outputs clean hardware PWM tones with programmable frequency and duration.

use ferrox_core::hal::ToneOutput;
use stm32f0xx_hal::pac;

pub struct Tim1Buzzer;

impl Tim1Buzzer {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for Tim1Buzzer {
    fn default() -> Self {
        Self::new()
    }
}

impl ToneOutput for Tim1Buzzer {
    /// Initialize GPIOA PA8 as AF2 (TIM1_CH1) and configure TIM1 PWM output.
    fn init(&mut self) {
        let rcc = unsafe { &*pac::RCC::ptr() };
        let gpioa = unsafe { &*pac::GPIOA::ptr() };
        let tim1 = unsafe { &*pac::TIM1::ptr() };

        unsafe {
            // 1. Enable GPIOA (bit 17) and TIM1 (bit 11) clocks
            rcc.ahbenr.modify(|r, w| w.bits(r.bits() | (1 << 17)));
            rcc.apb2enr.modify(|r, w| w.bits(r.bits() | (1 << 11)));

            // 2. Configure PA8 as AF mode (MODER[17:16] = 10)
            gpioa.moder.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !(3 << 16)) | (2 << 16))
            });

            // 3. Set PA8 Alternate Function to AF2 (TIM1_CH1)
            gpioa.afrh.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !0x0F) | 0x02)
            });

            // 4. Configure TIM1 for PWM generation:
            // PSC = 47 -> 48 MHz / 48 = 1 MHz (1 µs resolution)
            tim1.psc.write(|w| w.bits(47));

            // Output Compare 1 Mode: PWM mode 1 (active while CNT < CCR1)
            tim1.ccmr1_output().modify(|r, w| {
                let val = r.bits();
                w.bits((val & !0x7F) | (6 << 4) | (1 << 3))
            });

            // Enable Channel 1 output with active low polarity in CCER (CC1E | CC1P)
            tim1.ccer.modify(|r, w| {
                let val = r.bits();
                w.bits(val | (1 << 1) | (1 << 0))
            });

            // Main Output Enable (MOE) in BDTR (bit 15: MOE)
            tim1.bdtr.modify(|r, w| w.bits(r.bits() | (1 << 15)));
        }
    }

    /// Turn on the hardware PWM generator at the specified frequency (50% duty cycle).
    fn on(&mut self, freq_hz: u16) {
        if freq_hz >= 100 {
            let tim1 = unsafe { &*pac::TIM1::ptr() };

            unsafe {
                let period = (1_000_000u32 / (freq_hz as u32)).clamp(10, 65535);
                let arr = period.saturating_sub(1);
                let ccr = period / 2; // 50% duty cycle

                tim1.arr.write(|w| w.bits(arr));
                tim1.ccr1.write(|w| w.bits(ccr));
                if tim1.cnt.read().bits() > arr {
                    tim1.cnt.write(|w| w.bits(0));
                }

                // Start counter: CEN (bit 0) | ARPE (bit 7)
                tim1.cr1
                    .modify(|r, w| w.bits(r.bits() | (1 << 7) | (1 << 0)));
            }
        }
    }

    /// Turn off the hardware PWM generator.
    fn off(&mut self) {
        let tim1 = unsafe { &*pac::TIM1::ptr() };

        unsafe {
            tim1.cr1.write(|w| w.bits(0));
            tim1.cnt.write(|w| w.bits(0));
            tim1.sr.write(|w| w.bits(0));
        }
    }
}
