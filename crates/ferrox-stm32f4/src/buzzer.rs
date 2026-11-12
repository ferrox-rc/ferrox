use ferrox_core::hal::ToneOutput;
use stm32f4::stm32f407 as pac;

pub struct DacToneBuzzer;

impl DacToneBuzzer {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for DacToneBuzzer {
    fn default() -> Self {
        Self::new()
    }
}

impl ToneOutput for DacToneBuzzer {
    fn init(&mut self) {
        let rcc = unsafe { &*pac::RCC::ptr() };
        let gpioa = unsafe { &*pac::GPIOA::ptr() };
        let gpioe = unsafe { &*pac::GPIOE::ptr() };
        let tim5 = unsafe { &*pac::TIM5::ptr() };

        unsafe {
            // Enable GPIOA, GPIOE (AHB1) and TIM5 (APB1)
            rcc.ahb1enr
                .modify(|r, w| w.bits(r.bits() | (1 << 0) | (1 << 4)));
            rcc.apb1enr.modify(|r, w| w.bits(r.bits() | (1 << 3))); // TIM5

            // PE12 (AUDIO_MUTE): GPIO Output, default HIGH (muted to avoid startup pop)
            gpioe
                .moder
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 24)) | (0x01 << 24)));
            gpioe.bsrr.write(|w| w.bits(1 << 12)); // PE12 HIGH = Muted

            // PA4 (AUDIO_OUT): AF2 (TIM5_CH1)
            gpioa
                .moder
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 8)) | (0x02 << 8)));
            gpioa
                .afrl
                .modify(|r, w| w.bits((r.bits() & !(0x0F << 16)) | (0x02 << 16)));

            // TIM5 on APB1 (84 MHz clock): PSC = 83 (1 MHz tick)
            tim5.psc.write(|w| w.bits(83));
            tim5.ccmr1_output().modify(|r, w| {
                w.bits((r.bits() & !(0x7F << 0)) | (0x06 << 4) | (1 << 3)) // PWM mode 1, preload
            });
            tim5.ccer.modify(|r, w| w.bits(r.bits() | (1 << 0))); // CC1E
        }
    }

    fn on(&mut self, freq_hz: u16) {
        if crate::audio::is_active() {
            return;
        }
        if freq_hz == 0 {
            self.off();
            return;
        }

        let tim5 = unsafe { &*pac::TIM5::ptr() };
        let gpioe = unsafe { &*pac::GPIOE::ptr() };

        let period_us = (1_000_000u32 / freq_hz as u32).max(2);
        let arr = period_us - 1;
        let ccr = period_us / 2; // 50% duty square wave

        unsafe {
            tim5.arr.write(|w| w.bits(arr));
            tim5.ccr1().write(|w| w.bits(ccr));
            tim5.egr.write(|w| w.bits(1));
            tim5.cr1.modify(|r, w| w.bits(r.bits() | 0x01)); // CEN

            // Unmute audio amp (PE12 LOW)
            gpioe.bsrr.write(|w| w.bits(1 << (12 + 16)));
        }
    }

    fn off(&mut self) {
        if crate::audio::is_active() {
            return;
        }
        let tim5 = unsafe { &*pac::TIM5::ptr() };
        let gpioe = unsafe { &*pac::GPIOE::ptr() };

        unsafe {
            tim5.cr1.modify(|r, w| w.bits(r.bits() & !0x01)); // Stop CEN
            tim5.ccr1().write(|w| w.bits(0));

            // Mute audio amp (PE12 HIGH) to prevent static hissing
            gpioe.bsrr.write(|w| w.bits(1 << 12));
        }
    }
}
