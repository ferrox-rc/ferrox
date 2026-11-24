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
        unsafe {
            crate::audio::init_dac1_hw();
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

        // APB1 timer clock for TIM5 is 84 MHz (SYSCLK 168 MHz / 4 * 2)
        // With 32 samples per wave cycle, sample frequency is freq_hz * 32.
        let target_rate = (freq_hz as u32).saturating_mul(SINE_SAMPLES as u32);
        let arr = (84_000_000u32 / target_rate).saturating_sub(1).max(1);

        unsafe {
            if self.is_playing {
                tim5.arr.write(|w| w.bits(arr));
                if tim5.cnt.read().bits() > arr {
                    tim5.cnt.write(|w| w.bits(0));
                }
                return;
            }

            tim5.cr1.write(|w| w.bits(0));
            let lut_ptr = core::ptr::addr_of!(SINE_LUT.0) as u32;
            crate::audio::start_dac1_circular_dma(lut_ptr, SINE_SAMPLES as u32, 0b011); // TSEL1 = 011 (TIM5_TRGO)

            // Configure TIM5: PSC = 0, ARR = arr, MMS = 0b010 (Update -> TRGO), CEN = 1
            tim5.psc.write(|w| w.bits(0));
            tim5.arr.write(|w| w.bits(arr));
            tim5.cr2.write(|w| w.bits(0b010 << 4)); // MMS = Update
            tim5.egr.write(|w| w.bits(1));
            tim5.cr1.write(|w| w.bits(1)); // CEN

            crate::audio::unmute_amp();
        }

        self.is_playing = true;
    }

    fn off(&mut self) {
        if crate::audio::is_active() {
            self.is_playing = false;
            return;
        }

        let tim5 = unsafe { &*pac::TIM5::ptr() };

        unsafe {
            tim5.cr1.write(|w| w.bits(0));
            crate::audio::stop_dac1_dma_keep_amp_warm();
        }

        self.is_playing = false;
    }

    fn tick(&mut self, elapsed_ms: u16) {
        unsafe {
            crate::audio::tick_amp_power(elapsed_ms, self.is_playing);
        }
    }

    fn is_warming_up(&self) -> bool {
        crate::audio::is_amp_warming_up()
    }
}
