use core::marker::PhantomData;
use ferrox_core::hal::AdcDriver;
use stm32f4::stm32f407 as pac;

use crate::descriptor::{PocketHal, Qx7HalDescriptor};

const SCAN_CHANNELS: usize = 7;
static mut ADC_DMA_BUFFER: [u16; SCAN_CHANNELS] = [2048; SCAN_CHANNELS];

pub struct Stm32f4AdcDriver<H: Qx7HalDescriptor = PocketHal> {
    _phantom: PhantomData<H>,
}

impl<H: Qx7HalDescriptor> Stm32f4AdcDriver<H> {
    pub const fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<H: Qx7HalDescriptor> Default for Stm32f4AdcDriver<H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H: Qx7HalDescriptor> AdcDriver for Stm32f4AdcDriver<H> {
    fn init(&mut self, _ext_adc: bool) {
        let rcc = unsafe { &*pac::RCC::ptr() };
        let gpioa = unsafe { &*pac::GPIOA::ptr() };
        let gpiob = unsafe { &*pac::GPIOB::ptr() };
        let gpioc = unsafe { &*pac::GPIOC::ptr() };
        let adc1 = unsafe { &*pac::ADC1::ptr() };
        let dma2 = unsafe { &*pac::DMA2::ptr() };

        unsafe {
            // 1. Enable Clocks: GPIOA, GPIOB, GPIOC, GPIOD, GPIOE, DMA2 (AHB1) + ADC1 (APB2)
            rcc.ahb1enr.modify(|r, w| {
                w.bits(r.bits() | (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3) | (1 << 4) | (1 << 22))
            });
            rcc.apb2enr.modify(|r, w| w.bits(r.bits() | (1 << 8)));

            // 2. Configure Analog Pins: PA0, PA1, PA2, PA3, PA6, PB0, PC0 (MODER = 0b11)
            // PA0..PA3 + PA6 Analog
            gpioa
                .moder
                .modify(|r, w| w.bits(r.bits() | 0xFF | (0x03 << 12)));
            // PB0 Analog
            gpiob.moder.modify(|r, w| w.bits(r.bits() | 0x03));
            // PC0 Analog
            gpioc.moder.modify(|r, w| w.bits(r.bits() | 0x03));

            // 3. Configure Digital Switch Pins for virtual ADC conversion (Input with Pull-Up)
            H::SW_SA.init();
            H::SW_SB.init();
            H::SW_SC.init();
            H::SW_SD.init();

            // 4. Configure DMA2 Stream 4 (Channel 0): Circular, 16-bit, Memory increment
            let stream4 = &dma2.st[4];
            stream4.cr.write(|w| w.bits(0)); // Disable stream
            let mut dma_timeout = 50_000u32;
            while (stream4.cr.read().bits() & 0x01) != 0 && dma_timeout > 0 {
                dma_timeout -= 1;
            }

            dma2.hifcr.write(|w| w.bits(0x3D << 0)); // Clear Stream 4 flags

            stream4
                .par
                .write(|w| w.bits(core::ptr::addr_of!(adc1.dr) as u32));
            stream4
                .m0ar
                .write(|w| w.bits(core::ptr::addr_of_mut!(ADC_DMA_BUFFER) as u32));
            stream4.ndtr.write(|w| w.bits(SCAN_CHANNELS as u32));

            // CHSEL = 0, MSIZE = 01 (16-bit), PSIZE = 01 (16-bit), MINC = 1, CIRC = 1, PL = 10 (High)
            stream4.cr.write(|w| {
                w.bits(
                    (0 << 25)      // Channel 0
                    | (0x02 << 16) // Priority High
                    | (0x01 << 13) // MSIZE 16-bit
                    | (0x01 << 11) // PSIZE 16-bit
                    | (1 << 10)    // MINC
                    | (1 << 8)     // CIRC
                    | (1 << 0), // EN
                )
            });

            // 5. Configure ADC1 Common: Prescaler /4 (84 MHz / 4 = 21 MHz)
            let ccr = &*pac::ADC_COMMON::ptr();
            ccr.ccr.write(|w| w.bits(0x01 << 16)); // ADCPRE /4

            // 6. Configure ADC1
            // 7 conversions: SQR1 L = 6 (L = N - 1)
            adc1.sqr1.write(|w| w.bits(6 << 20));
            // SQR3: SQ1=IN0 (RV), SQ2=IN1 (RH), SQ3=IN2 (LV), SQ4=IN3 (LH), SQ5=IN8 (PB0), SQ6=IN6 (PA6)
            adc1.sqr3
                .write(|w| w.bits(0 | (1 << 5) | (2 << 10) | (3 << 15) | (8 << 20) | (6 << 25)));
            // SQR2: SQ7=IN10 (PC0 VBAT)
            adc1.sqr2.write(|w| w.bits(10 << 0));

            // Sample time: 56 cycles (SMPR = 0b011)
            adc1.smpr2.write(|w| w.bits(0o33333333));
            adc1.smpr1.write(|w| w.bits(0o33333333));

            // CR1: SCAN mode
            adc1.cr1.write(|w| w.bits(1 << 8));
            // CR2: ADON, CONT, DMA, DDS
            adc1.cr2
                .write(|w| w.bits((1 << 0) | (1 << 1) | (1 << 8) | (1 << 9)));

            // Start conversion (SWSTART = bit 30 in CR2)
            adc1.cr2.modify(|r, w| w.bits(r.bits() | (1 << 30)));
        }
    }

    fn read_channels(&mut self, dest: &mut [u16]) {
        let raw_dma = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(ADC_DMA_BUFFER)) };

        if dest.len() >= 11 {
            let raw_rv = raw_dma[0].min(4095);
            let raw_rh = raw_dma[1].min(4095);
            let raw_lv = raw_dma[2].min(4095);
            let raw_lh = raw_dma[3].min(4095);
            let raw_pb0 = raw_dma[4].min(4095);
            let raw_pa6 = raw_dma[5].min(4095);
            let raw_vbat = raw_dma[6].min(4095);

            let (raw_s1, raw_s2) = if H::SWAP_S1_S2_ADC {
                (raw_pa6, raw_pb0)
            } else {
                (raw_pb0, raw_pa6)
            };

            // Gimbals per descriptor stick inversion:
            // dest[0]: RH
            dest[0] = if H::STICK_INVERT[1] {
                4095 - raw_rh
            } else {
                raw_rh
            };
            // dest[1]: RV
            dest[1] = if H::STICK_INVERT[0] {
                4095 - raw_rv
            } else {
                raw_rv
            };
            // dest[2]: LV
            dest[2] = if H::STICK_INVERT[2] {
                4095 - raw_lv
            } else {
                raw_lv
            };
            // dest[3]: LH
            dest[3] = if H::STICK_INVERT[3] {
                4095 - raw_lh
            } else {
                raw_lh
            };

            // Switches synthesized as virtual ADC voltages:
            dest[4] = H::SW_SA.read_virtual_adc();
            dest[5] = H::SW_SB.read_virtual_adc();

            // S1 Pot (dest[6])
            dest[6] = if H::S1_INVERT { 4095 - raw_s1 } else { raw_s1 };

            // S2 Pot (dest[7])
            dest[7] = if H::HAS_S2 {
                if H::S2_INVERT {
                    4095 - raw_s2
                } else {
                    raw_s2
                }
            } else {
                2048
            };

            // SC and SD switches
            dest[8] = H::SW_SC.read_virtual_adc();
            dest[9] = H::SW_SD.read_virtual_adc();

            // VBAT (dest[10])
            dest[10] = raw_vbat;
        }
    }

    fn wait_first_conversion(&mut self) {
        cortex_m::asm::delay(168_000 * 5); // ~5ms
    }
}
