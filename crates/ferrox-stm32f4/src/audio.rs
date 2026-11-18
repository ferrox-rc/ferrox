//! STM32F407 12-bit DAC1 (`PA4`) + `TIM6` + `DMA1_Stream5` (Channel 7) PCM audio driver.
//!
//! Uses a circular 2048-sample (`4 KiB`) ring buffer driven by `DMA1_Stream5` in circular
//! mode triggered by `TIM6_TRGO`. Consumed slots are cleared to DAC midpoint (`2048`)
//! during `sync_dma()` so underruns or end-of-clip transitions never replay stale audio.
//! Coordinates `PA4` pin muxing and `PE12` (`AUDIO_MUTE`) with [`crate::DacToneBuzzer`].

use core::sync::atomic::{AtomicBool, Ordering};
use ferrox_core::hal::AudioOutput;
use stm32f4::stm32f407 as pac;

const BUF_LEN: usize = 2048;
const DAC_MIDPOINT: u16 = 2048;

/// Shared flag so `DacToneBuzzer` does not steal `PA4` or mute `PE12` while PCM is streaming.
static AUDIO_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Returns `true` while the PCM DAC driver owns `PA4` and `PE12`.
pub fn is_active() -> bool {
    AUDIO_ACTIVE.load(Ordering::Relaxed)
}

#[repr(align(4))]
struct DmaRing([u16; BUF_LEN]);

static mut DMA_RING: DmaRing = DmaRing([DAC_MIDPOINT; BUF_LEN]);

pub struct Stm32f4AudioDriver {
    active: bool,
    sample_rate_hz: u32,
    write_idx: usize,
    last_dma_pos: usize,
    samples_queued: usize,
}

impl Stm32f4AudioDriver {
    pub const fn new() -> Self {
        Self {
            active: false,
            sample_rate_hz: 0,
            write_idx: 0,
            last_dma_pos: 0,
            samples_queued: 0,
        }
    }

    fn current_dma_pos() -> usize {
        let dma1 = unsafe { &*pac::DMA1::ptr() };
        let ndtr = dma1.st[5].ndtr.read().bits() as usize;
        if ndtr == 0 || ndtr > BUF_LEN {
            0
        } else {
            (BUF_LEN - ndtr) % BUF_LEN
        }
    }

    fn sync_dma(&mut self) {
        if !self.active {
            return;
        }
        let dma_pos = Self::current_dma_pos();
        let consumed = (dma_pos + BUF_LEN - self.last_dma_pos) % BUF_LEN;
        if consumed > 0 {
            let mut idx = self.last_dma_pos;
            for _ in 0..consumed {
                unsafe {
                    DMA_RING.0[idx] = DAC_MIDPOINT;
                }
                idx = (idx + 1) % BUF_LEN;
            }
            self.last_dma_pos = dma_pos;
            self.samples_queued = self.samples_queued.saturating_sub(consumed);
        }
    }
}

impl Default for Stm32f4AudioDriver {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared hardware initialization for `PA4` (`DAC_OUT1`), `PE12` (`AUDIO_MUTE`), `DMA1`, `TIM5`, `TIM6`, and `DAC1`.
pub(crate) unsafe fn init_dac1_hw() {
    let rcc = &*pac::RCC::ptr();
    let gpioa = &*pac::GPIOA::ptr();
    let gpioe = &*pac::GPIOE::ptr();
    let dac = &*pac::DAC::ptr();

    // Enable GPIOA, GPIOE, DMA1 clocks (AHB1) and TIM5, TIM6, DAC clocks (APB1)
    rcc.ahb1enr
        .modify(|r, w| w.bits(r.bits() | (1 << 0) | (1 << 4) | (1 << 21)));
    rcc.apb1enr
        .modify(|r, w| w.bits(r.bits() | (1 << 3) | (1 << 4) | (1 << 29)));
    let _ = rcc.ahb1enr.read().bits();
    let _ = rcc.apb1enr.read().bits();

    // Ensure PE12 (AUDIO_MUTE) is output HIGH (muted)
    gpioe
        .moder
        .modify(|r, w| w.bits((r.bits() & !(0x03 << 24)) | (0x01 << 24)));
    gpioe.bsrr.write(|w| w.bits(1 << 12));

    // PA4 (AUDIO_OUT): Analog mode (0b11) for DAC_OUT1
    gpioa
        .moder
        .modify(|r, w| w.bits((r.bits() & !(0x03 << 8)) | (0x03 << 8)));

    // Park DAC1 at midpoint
    dac.dhr12r1.write(|w| w.bits(DAC_MIDPOINT as u32));
    dac.cr.write(|w| w.bits(1 << 0));
}

/// Configure `DMA1_Stream5` (Channel 7) in circular 16-bit M2P mode feeding `DAC_DHR12R1`
/// with trigger selection `tsel` (`0b000` = `TIM6_TRGO`, `0b011` = `TIM5_TRGO`).
pub(crate) unsafe fn start_dac1_circular_dma(buf_ptr: u32, len: u32, tsel: u32) {
    let gpioa = &*pac::GPIOA::ptr();
    let dac = &*pac::DAC::ptr();
    let dma1 = &*pac::DMA1::ptr();

    gpioa
        .moder
        .modify(|r, w| w.bits((r.bits() & !(0x03 << 8)) | (0x03 << 8)));

    // Disable DMA1 Stream 5 with bounded timeout before reconfiguring (RM0090 §10.3.17)
    dma1.st[5].cr.modify(|r, w| w.bits(r.bits() & !1));
    let mut timeout = 10_000u32;
    while (dma1.st[5].cr.read().bits() & 1) != 0 && timeout > 0 {
        timeout -= 1;
    }
    // Clear Stream 5 interrupt flags (HIFCR bits 11:6)
    dma1.hifcr.write(|w| w.bits(0x3D << 6));

    let dhr_ptr = core::ptr::addr_of!(dac.dhr12r1) as u32;
    dma1.st[5].par.write(|w| w.bits(dhr_ptr));
    dma1.st[5].m0ar.write(|w| w.bits(buf_ptr));
    dma1.st[5].ndtr.write(|w| w.bits(len));
    // CHSEL=7 (bits 27:25), PL=10 High (17:16), MSIZE=01 16b (14:13),
    // PSIZE=01 16b (12:11), MINC=1 (10), CIRC=1 (8), DIR=01 M2P (6), EN=1 (0)
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

    // Configure DAC Channel 1: EN1 (0), TEN1 (2), TSEL1 (5:3), DMAEN1 (12)
    dac.dhr12r1.write(|w| w.bits(DAC_MIDPOINT as u32));
    dac.cr
        .write(|w| w.bits((1 << 0) | (1 << 2) | ((tsel & 0x07) << 3) | (1 << 12)));
}

/// Mute `PE12` (`AUDIO_MUTE` HIGH), stop `DMA1_Stream5`, and park `DAC1` at midpoint (`2048`).
pub(crate) unsafe fn stop_dac1_dma_and_mute() {
    let gpioe = &*pac::GPIOE::ptr();
    let dac = &*pac::DAC::ptr();
    let dma1 = &*pac::DMA1::ptr();

    // Mute audio amp first (PE12 HIGH)
    gpioe.bsrr.write(|w| w.bits(1 << 12));

    dma1.st[5].cr.modify(|r, w| w.bits(r.bits() & !1));
    dac.dhr12r1.write(|w| w.bits(DAC_MIDPOINT as u32));
    dac.cr.write(|w| w.bits(1 << 0)); // EN1 only (holding midpoint)
}

/// Unmute `PE12` (`AUDIO_MUTE` LOW).
#[inline]
pub(crate) unsafe fn unmute_amp() {
    let gpioe = &*pac::GPIOE::ptr();
    gpioe.bsrr.write(|w| w.bits(1 << (12 + 16)));
}

impl AudioOutput for Stm32f4AudioDriver {
    fn init(&mut self) {
        unsafe {
            init_dac1_hw();
        }
    }

    fn start(&mut self, sample_rate_hz: u32) -> bool {
        let sr = sample_rate_hz.clamp(4000, 48000);
        if self.active && self.sample_rate_hz == sr {
            return true;
        }
        if self.active {
            self.stop();
        }

        let tim5 = unsafe { &*pac::TIM5::ptr() };
        let tim6 = unsafe { &*pac::TIM6::ptr() };

        unsafe {
            // Stop TIM5 tone timer if running
            tim5.cr1.modify(|r, w| w.bits(r.bits() & !0x01));

            // Fill ring with midpoint silence before starting DMA
            DMA_RING.0.fill(DAC_MIDPOINT);
            self.write_idx = 0;
            self.last_dma_pos = 0;
            self.samples_queued = 0;
            self.sample_rate_hz = sr;

            let ring_ptr = core::ptr::addr_of!(DMA_RING.0) as u32;
            start_dac1_circular_dma(ring_ptr, BUF_LEN as u32, 0b000); // TSEL1 = 000 (TIM6_TRGO)

            // Configure TIM6 (APB1 timer clock = 84 MHz): MMS=010 (Update -> TRGO)
            tim6.cr1.write(|w| w.bits(0));
            tim6.psc.write(|w| w.bits(0));
            let arr = (84_000_000u32 / sr).saturating_sub(1).max(1);
            tim6.arr.write(|w| w.bits(arr));
            tim6.cr2.write(|w| w.bits(0b010 << 4)); // MMS = Update
            tim6.egr.write(|w| w.bits(1));
            tim6.cr1.write(|w| w.bits(1)); // CEN

            unmute_amp();
        }

        self.active = true;
        AUDIO_ACTIVE.store(true, Ordering::Relaxed);
        true
    }

    fn stop(&mut self) {
        if !self.active {
            return;
        }
        let tim6 = unsafe { &*pac::TIM6::ptr() };

        unsafe {
            tim6.cr1.write(|w| w.bits(0));
            stop_dac1_dma_and_mute();
        }

        self.active = false;
        self.samples_queued = 0;
        AUDIO_ACTIVE.store(false, Ordering::Relaxed);
    }

    fn write_samples(&mut self, samples: &[u16]) -> usize {
        if !self.active || samples.is_empty() {
            return 0;
        }
        self.sync_dma();
        let free = (BUF_LEN - 1).saturating_sub(self.samples_queued);
        let take = free.min(samples.len());
        for &s in &samples[..take] {
            unsafe {
                DMA_RING.0[self.write_idx] = s.min(4095);
            }
            self.write_idx = (self.write_idx + 1) % BUF_LEN;
        }
        self.samples_queued += take;
        take
    }

    fn free_space(&self) -> usize {
        if !self.active {
            return 0;
        }
        // Read live DMA position without mutating so caller gets accurate headroom
        let dma_pos = Self::current_dma_pos();
        let consumed = (dma_pos + BUF_LEN - self.last_dma_pos) % BUF_LEN;
        let queued = self.samples_queued.saturating_sub(consumed);
        (BUF_LEN - 1).saturating_sub(queued)
    }

    fn is_playing(&self) -> bool {
        if !self.active {
            return false;
        }
        let dma_pos = Self::current_dma_pos();
        let consumed = (dma_pos + BUF_LEN - self.last_dma_pos) % BUF_LEN;
        self.samples_queued > consumed
    }
}
