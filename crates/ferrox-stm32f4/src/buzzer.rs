//! STM32F407 12-bit DAC1 (`PA4`) Tone Buzzer using `TIM5_TRGO` + `DMA1_Stream5`.
//!
//! On STM32F407 radios (TX12v2, RadioMaster Pocket, Zorro), `PA4` is connected to the
//! audio power amplifier input and lacks any hardware timer alternate function.
//! This driver outputs clean hardware-timed acoustic tones and melodies by triggering
//! circular DMA transfers of a 32-sample sine table from `DMA1_Stream5` to `DAC_DHR12R1`
//! using `TIM5` TRGO update events. It coordinates seamlessly with the streaming PCM
//! voice driver in [`crate::audio`], muting `PE12` (`AUDIO_MUTE`) when silent to prevent hiss.

use ferrox_core::hal::ToneOutput;
use stm32f4::stm32f407 as pac;

const SINE_SAMPLES: usize = 32;
const DAC_MIDPOINT: u16 = 2048;

/// 32-point 12-bit sinusoidal wave centered at 2048 with peak amplitude ~1500 (range 548..3548).
#[repr(align(4))]
struct SineLut([u16; SINE_SAMPLES]);

static SINE_LUT: SineLut = SineLut([
    2048, 2341, 2622, 2881, 3109, 3295, 3434, 3519, 3548, 3519, 3434, 3295, 3109, 2881, 2622, 2341,
    2048, 1755, 1474, 1215, 987, 801, 662, 577, 548, 577, 662, 801, 987, 1215, 1474, 1755,
]);

pub struct DacToneBuzzer {
    is_playing: bool,
}

impl DacToneBuzzer {
    pub const fn new() -> Self {
        Self { is_playing: false }
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
        let dac = unsafe { &*pac::DAC::ptr() };

        unsafe {
            // Enable GPIOA, GPIOE, DMA1 (AHB1) and TIM5, DAC (APB1)
            rcc.ahb1enr
                .modify(|r, w| w.bits(r.bits() | (1 << 0) | (1 << 4) | (1 << 21)));
            rcc.apb1enr
                .modify(|r, w| w.bits(r.bits() | (1 << 3) | (1 << 29)));
            let _ = rcc.ahb1enr.read().bits();
            let _ = rcc.apb1enr.read().bits();

            // PE12 (AUDIO_MUTE): GPIO Output, default HIGH (muted to avoid startup pop)
            gpioe
                .moder
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 24)) | (0x01 << 24)));
            gpioe.bsrr.write(|w| w.bits(1 << 12)); // PE12 HIGH = Muted

            // PA4 (AUDIO_OUT): Analog mode (0b11) for DAC_OUT1
            gpioa
                .moder
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 8)) | (0x03 << 8)));

            // Hold DAC1 at midpoint
            dac.dhr12r1.write(|w| w.bits(DAC_MIDPOINT as u32));
            dac.cr.write(|w| w.bits(1 << 0)); // EN1
        }
    }

    fn on(&mut self, freq_hz: u16) {
        if crate::audio::is_active() {
            self.is_playing = false;
            return;
        }
        if freq_hz < 100 {
            self.off();
            return;
        }

        let tim5 = unsafe { &*pac::TIM5::ptr() };
        let gpioe = unsafe { &*pac::GPIOE::ptr() };
        let dac = unsafe { &*pac::DAC::ptr() };
        let dma1 = unsafe { &*pac::DMA1::ptr() };

        // APB1 timer clock for TIM5 is 84 MHz (SYSCLK 168 MHz / 4 * 2)
        // With 32 samples per wave cycle, sample frequency is freq_hz * 32.
        // Timer tick ARR = (84_000_000 / (freq_hz * 32)) - 1.
        let target_rate = (freq_hz as u32).saturating_mul(SINE_SAMPLES as u32);
        let arr = (84_000_000u32 / target_rate).saturating_sub(1).max(1);

        unsafe {
            if self.is_playing {
                // Tone frequency change while playing: simply update ARR cleanly
                tim5.arr.write(|w| w.bits(arr));
                if tim5.cnt.read().bits() > arr {
                    tim5.cnt.write(|w| w.bits(0));
                }
                return;
            }

            // Stop TIM5 and disable DMA1 Stream 5 before configuration
            tim5.cr1.write(|w| w.bits(0));
            dma1.st[5].cr.modify(|r, w| w.bits(r.bits() & !1));
            while (dma1.st[5].cr.read().bits() & 1) != 0 {}
            dma1.hifcr.write(|w| w.bits(0x3D << 6)); // Clear stream 5 flags

            let lut_ptr = core::ptr::addr_of!(SINE_LUT.0) as u32;
            let dhr_ptr = core::ptr::addr_of!(dac.dhr12r1) as u32;

            dma1.st[5].par.write(|w| w.bits(dhr_ptr));
            dma1.st[5].m0ar.write(|w| w.bits(lut_ptr));
            dma1.st[5].ndtr.write(|w| w.bits(SINE_SAMPLES as u32));
            // Channel 7 (DAC1), Priority High (10), 16-bit Memory (01), 16-bit Peripheral (01),
            // Memory Increment (1), Circular mode (1), Memory-to-Peripheral (01), Enable (1)
            dma1.st[5].cr.write(|w| {
                w.bits(
                    (7 << 25)
                        | (2 << 16)
                        | (1 << 13)
                        | (1 << 11)
                        | (1 << 10)
                        | (1 << 8)
                        | (1 << 6)
                        | 1,
                )
            });

            // Configure DAC Channel 1:
            // EN1 (bit 0), TEN1 (bit 2), TSEL1 = 0b011 (TIM5_TRGO, bits 5:3), DMAEN1 (bit 12)
            dac.cr.write(|w| {
                w.bits((1 << 0) | (1 << 2) | (0b011 << 3) | (1 << 12))
            });

            // Configure TIM5: PSC = 0, ARR = arr, MMS = 0b010 (Update -> TRGO), CEN = 1
            tim5.psc.write(|w| w.bits(0));
            tim5.arr.write(|w| w.bits(arr));
            tim5.cr2.write(|w| w.bits(0b010 << 4)); // MMS = Update
            tim5.egr.write(|w| w.bits(1));
            tim5.cr1.write(|w| w.bits(1)); // CEN

            // Unmute audio amp (PE12 LOW)
            gpioe.bsrr.write(|w| w.bits(1 << (12 + 16)));
        }

        self.is_playing = true;
    }

    fn off(&mut self) {
        if crate::audio::is_active() {
            self.is_playing = false;
            return;
        }

        let tim5 = unsafe { &*pac::TIM5::ptr() };
        let gpioe = unsafe { &*pac::GPIOE::ptr() };
        let dac = unsafe { &*pac::DAC::ptr() };
        let dma1 = unsafe { &*pac::DMA1::ptr() };

        unsafe {
            // Mute audio amp first (PE12 HIGH) to prevent speaker pop
            gpioe.bsrr.write(|w| w.bits(1 << 12));

            // Stop timer and DMA
            tim5.cr1.write(|w| w.bits(0));
            dma1.st[5].cr.modify(|r, w| w.bits(r.bits() & !1));

            // Restore DAC to non-triggered midpoint
            dac.dhr12r1.write(|w| w.bits(DAC_MIDPOINT as u32));
            dac.cr.write(|w| w.bits(1 << 0));
        }

        self.is_playing = false;
    }
}
