//! Continuous ADC1 scanning via autonomous DMA1 Channel 1 for STM32F072 targets.

use core::cell::UnsafeCell;
use core::ptr;
use ferrox_core::hal::{AdcDriver, MAX_ADC_CHANNELS};
use stm32f0xx_hal::pac;

pub const NUM_STD_CHANNELS: usize = 11;
pub const NUM_CHANNELS: usize = MAX_ADC_CHANNELS;

const DMA_CCR_VAL: u32 = (1 << 7) | (1 << 8) | (1 << 10) | (2 << 12) | (1 << 0);

struct AdcDmaBuffer(UnsafeCell<[u16; NUM_CHANNELS]>);
unsafe impl Sync for AdcDmaBuffer {}

struct AdcChannelCount(UnsafeCell<usize>);
unsafe impl Sync for AdcChannelCount {}

static SCAN_CHANNELS: AdcChannelCount = AdcChannelCount(UnsafeCell::new(NUM_STD_CHANNELS));
static ADC_RAW_BUFFER: AdcDmaBuffer = AdcDmaBuffer(UnsafeCell::new([0; NUM_CHANNELS]));
static LATEST_ADC_SNAPSHOT: AdcDmaBuffer = AdcDmaBuffer(UnsafeCell::new([2048; NUM_CHANNELS]));

pub struct Stm32f0AdcDriver {
    is_i6s: bool,
}

impl Stm32f0AdcDriver {
    pub const fn new(is_i6s: bool) -> Self {
        Self { is_i6s }
    }
}

impl AdcDriver for Stm32f0AdcDriver {
    fn init(&mut self, ext_adc: bool) {
        let ch_count = if ext_adc {
            if self.is_i6s {
                16
            } else {
                15
            }
        } else {
            NUM_STD_CHANNELS
        };

        unsafe {
            *SCAN_CHANNELS.0.get() = ch_count;

            let chselr = if ext_adc {
                if self.is_i6s {
                    0xFFFF
                } else {
                    0xF7FF
                }
            } else {
                0x07FF
            };

            let rcc = &*pac::RCC::ptr();
            let gpioa = &*pac::GPIOA::ptr();
            let gpiob = &*pac::GPIOB::ptr();
            let gpioc = &*pac::GPIOC::ptr();
            let adc = &*pac::ADC::ptr();
            let dma = &*pac::DMA1::ptr();

            // 1. Enable GPIOA, GPIOB, GPIOC, DMA1 in RCC_AHBENR
            rcc.ahbenr.modify(|_, w| {
                w.dmaen()
                    .set_bit()
                    .iopaen()
                    .set_bit()
                    .iopben()
                    .set_bit()
                    .iopcen()
                    .set_bit()
            });

            // 2. Enable ADC1 clock in RCC_APB2ENR
            rcc.apb2enr.modify(|_, w| w.adcen().set_bit());

            // 3. Configure Pins as Analog Mode:
            gpioa
                .moder
                .modify(|r, w| w.bits((r.bits() & !0x0000_FFFF) | 0x0000_FFFF));
            gpioa.pupdr.modify(|r, w| w.bits(r.bits() & !0x0000_FFFF));

            gpiob
                .moder
                .modify(|r, w| w.bits((r.bits() & !0x0000_000F) | 0x0000_000F));
            gpiob.pupdr.modify(|r, w| w.bits(r.bits() & !0x0000_000F));

            if self.is_i6s {
                gpioc
                    .moder
                    .modify(|r, w| w.bits((r.bits() & !0x0000_0FFF) | 0x0000_0FFF));
                gpioc.pupdr.modify(|r, w| w.bits(r.bits() & !0x0000_0FFF));
            } else {
                gpioc
                    .moder
                    .modify(|r, w| w.bits((r.bits() & !0x0000_0FF3) | 0x0000_0FF3));
                gpioc.pupdr.modify(|r, w| w.bits(r.bits() & !0x0000_0FF3));
            }

            // 4. ADC Clock Selection: Synchronous PCLK/4 (CKMODE = 10 in CFGR2)
            adc.cfgr2.write(|w| w.ckmode().bits(0b10));

            // 5. Enable ADC1 with timeout protection
            adc.isr.write(|w| w.adrdy().set_bit());
            adc.cr.modify(|_, w| w.aden().set_bit());
            let mut timeout = 100_000u32;
            while adc.isr.read().adrdy().bit_is_clear() && timeout > 0 {
                timeout -= 1;
            }

            // 6. Configure Channels
            adc.chselr.write(|w| w.bits(chselr));

            // 7. Sampling Time: 239.5 ADC cycles (SMPR = 0b111)
            adc.smpr.write(|w| w.smp().bits(0b111));

            // 8. Configure ADC CFGR1
            adc.cfgr1.write(|w| w.dmaen().set_bit());

            // 9. Configure DMA1 Channel 1:
            let ch1 = &dma.ch1;
            ch1.cr.write(|w| w.bits(0));
            ch1.par
                .write(|w| w.bits(core::ptr::addr_of!((*pac::ADC::ptr()).dr) as u32));
            ch1.mar.write(|w| w.bits(ADC_RAW_BUFFER.0.get() as u32));
            ch1.ndtr.write(|w| w.ndt().bits(ch_count as u16));
            ch1.cr.write(|w| w.bits(DMA_CCR_VAL));

            // 10. Start first conversion sequence
            adc.cr.modify(|_, w| w.adstart().set_bit());
        }
    }

    fn read_channels(&mut self, dest: &mut [u16]) {
        let snapshot = self.read_raw();
        let copy_len = dest.len().min(snapshot.len());
        dest[..copy_len].copy_from_slice(&snapshot[..copy_len]);
    }

    fn wait_first_conversion(&mut self) {
        unsafe {
            let ch_count = *SCAN_CHANNELS.0.get();
            let dma = &*pac::DMA1::ptr();

            let mut timeout = 200_000u32;
            while dma.isr.read().tcif1().bit_is_clear() && timeout > 0 {
                timeout -= 1;
            }

            let _ = Self::capture_and_rearm(ch_count);
        }
    }
}

impl Stm32f0AdcDriver {
    unsafe fn capture_and_rearm(ch_count: usize) -> [u16; NUM_CHANNELS] {
        let dma = &*pac::DMA1::ptr();
        let adc = &*pac::ADC::ptr();
        let raw_ptr = ADC_RAW_BUFFER.0.get();
        let snap_ptr = LATEST_ADC_SNAPSHOT.0.get();

        for i in 0..ch_count {
            (*snap_ptr)[i] = ptr::read_volatile(&(*raw_ptr)[i]);
        }
        if ch_count < NUM_CHANNELS {
            for i in ch_count..NUM_CHANNELS {
                (*snap_ptr)[i] = 2048;
            }
        }

        dma.ifcr.write(|w| w.bits(0x0F));
        adc.isr.write(|w| w.bits((1 << 4) | (1 << 3)));

        dma.ch1.cr.write(|w| w.bits(0));
        dma.ch1.ndtr.write(|w| w.ndt().bits(ch_count as u16));
        dma.ch1.cr.write(|w| w.bits(DMA_CCR_VAL));

        adc.cr.modify(|_, w| w.adstart().set_bit());
        *snap_ptr
    }

    fn read_raw(&mut self) -> [u16; NUM_CHANNELS] {
        unsafe {
            let ch_count = *SCAN_CHANNELS.0.get();
            let dma = &*pac::DMA1::ptr();
            if dma.isr.read().tcif1().bit_is_set() {
                Self::capture_and_rearm(ch_count)
            } else {
                *LATEST_ADC_SNAPSHOT.0.get()
            }
        }
    }
}
