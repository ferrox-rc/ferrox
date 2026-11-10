use ferrox_core::hal::AdcDriver;
use stm32f4::stm32f407 as pac;

const SCAN_CHANNELS: usize = 6;
static mut ADC_DMA_BUFFER: [u16; SCAN_CHANNELS] = [2048; SCAN_CHANNELS];

pub struct Stm32f4AdcDriver;

impl Stm32f4AdcDriver {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for Stm32f4AdcDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl AdcDriver for Stm32f4AdcDriver {
    fn init(&mut self, _ext_adc: bool) {
        let rcc = unsafe { &*pac::RCC::ptr() };
        let gpioa = unsafe { &*pac::GPIOA::ptr() };
        let gpiob = unsafe { &*pac::GPIOB::ptr() };
        let gpioc = unsafe { &*pac::GPIOC::ptr() };
        let gpiod = unsafe { &*pac::GPIOD::ptr() };
        let gpioe = unsafe { &*pac::GPIOE::ptr() };
        let adc1 = unsafe { &*pac::ADC1::ptr() };
        let dma2 = unsafe { &*pac::DMA2::ptr() };

        unsafe {
            // 1. Enable Clocks: GPIOA, GPIOB, GPIOC, GPIOD, GPIOE, DMA2 (AHB1) + ADC1 (APB2)
            rcc.ahb1enr.modify(|r, w| {
                w.bits(
                    r.bits()
                        | (1 << 0)
                        | (1 << 1)
                        | (1 << 2)
                        | (1 << 3)
                        | (1 << 4)
                        | (1 << 22),
                )
            });
            rcc.apb2enr.modify(|r, w| w.bits(r.bits() | (1 << 8)));

            // 2. Configure Analog Pins: PA0, PA1, PA2, PA3, PB0, PC0 (MODER = 0b11)
            gpioa.moder.modify(|r, w| w.bits(r.bits() | 0xFF)); // PA0..PA3 Analog
            gpiob.moder.modify(|r, w| w.bits(r.bits() | 0x03)); // PB0 Analog
            gpioc.moder.modify(|r, w| w.bits(r.bits() | 0x03)); // PC0 Analog

            // 3. Configure Digital Switch Pins for virtual ADC conversion (Input with Pull-Up)
            // SA: PC13
            gpioc
                .moder
                .modify(|r, w| w.bits(r.bits() & !(0x03 << 26)));
            gpioc
                .pupdr
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 26)) | (0x01 << 26)));

            // SB: PA5 (High), PE15 (Low)
            gpioa
                .moder
                .modify(|r, w| w.bits(r.bits() & !(0x03 << 10)));
            gpioa
                .pupdr
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 10)) | (0x01 << 10)));
            gpioe
                .moder
                .modify(|r, w| w.bits(r.bits() & !(0x03 << 30)));
            gpioe
                .pupdr
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 30)) | (0x01 << 30)));

            // SC: PD11 (High), PE0 (Low)
            gpiod
                .moder
                .modify(|r, w| w.bits(r.bits() & !(0x03 << 22)));
            gpiod
                .pupdr
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 22)) | (0x01 << 22)));
            gpioe.moder.modify(|r, w| w.bits(r.bits() & !0x03));
            gpioe
                .pupdr
                .modify(|r, w| w.bits((r.bits() & !0x03) | 0x01));

            // SD: PE8
            gpioe
                .moder
                .modify(|r, w| w.bits(r.bits() & !(0x03 << 16)));
            gpioe
                .pupdr
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 16)) | (0x01 << 16)));

            // 4. Configure DMA2 Stream 4 (Channel 0): Circular, 16-bit, Memory increment
            let stream4 = &dma2.st[4];
            stream4.cr.write(|w| w.bits(0)); // Disable stream
            while stream4.cr.read().bits() & 0x01 != 0 {} // Wait disabled

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
            // 6 conversions: SQR1 L = 5 (6 conversions: L=N-1)
            adc1.sqr1.write(|w| w.bits(5 << 20));
            // SQR3: SQ1=IN0, SQ2=IN1, SQ3=IN2, SQ4=IN3, SQ5=IN8, SQ6=IN10
            adc1.sqr3.write(|w| {
                w.bits(
                    0 | (1 << 5) | (2 << 10) | (3 << 15) | (8 << 20) | (10 << 25),
                )
            });

            // Sample time: 56 cycles (SMPR = 0b011)
            adc1.smpr2
                .write(|w| w.bits(0o33333333));
            adc1.smpr1
                .write(|w| w.bits(0o33333333));

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
        let (raw_dma, sa_down, sb_up, sb_down, sc_up, sc_down, sd_down) = unsafe {
            let gpioa = &*pac::GPIOA::ptr();
            let gpioc = &*pac::GPIOC::ptr();
            let gpiod = &*pac::GPIOD::ptr();
            let gpioe = &*pac::GPIOE::ptr();

            let dma = core::ptr::read_volatile(core::ptr::addr_of!(ADC_DMA_BUFFER));
            let sa_down = (gpioc.idr.read().bits() & (1 << 13)) == 0;
            let sb_up = (gpioa.idr.read().bits() & (1 << 5)) == 0;
            let sb_down = (gpioe.idr.read().bits() & (1 << 15)) == 0;
            let sc_up = (gpiod.idr.read().bits() & (1 << 11)) == 0;
            let sc_down = (gpioe.idr.read().bits() & (1 << 0)) == 0;
            let sd_down = (gpioe.idr.read().bits() & (1 << 8)) == 0;

            (dma, sa_down, sb_up, sb_down, sc_up, sc_down, sd_down)
        };

        if dest.len() >= 11 {
            // Gimbals per EdgeTX pocket.json:
            // RH (IN1, inverted) -> dest[0]
            dest[0] = 4095 - raw_dma[1].min(4095);
            // RV (IN0, normal) -> dest[1]
            dest[1] = raw_dma[0].min(4095);
            // LV (IN2, normal) -> dest[2]
            dest[2] = raw_dma[2].min(4095);
            // LH (IN3, inverted) -> dest[3]
            dest[3] = 4095 - raw_dma[3].min(4095);

            // Switches synthesized as virtual ADC voltages:
            // SA (2-pos): Down = 4000, Up = 100
            dest[4] = if sa_down { 4000 } else { 100 };
            // SB (3-pos): Up = 100, Down = 4000, Mid = 2048
            dest[5] = if sb_up {
                100
            } else if sb_down {
                4000
            } else {
                2048
            };

            // S1 Pot (IN8, inverted per pocket.json) -> dest[6]
            dest[6] = 4095 - raw_dma[4].min(4095);
            // VRB (not present on Pocket) -> dest[7] neutral
            dest[7] = 2048;

            // SC (3-pos): Up = 100, Down = 4000, Mid = 2048
            dest[8] = if sc_up {
                100
            } else if sc_down {
                4000
            } else {
                2048
            };
            // SD (2-pos): Down = 4000, Up = 100
            dest[9] = if sd_down { 4000 } else { 100 };

            // VBAT (IN10)
            dest[10] = raw_dma[5].min(4095);
        }
    }

    fn wait_first_conversion(&mut self) {
        cortex_m::asm::delay(168_000 * 5); // ~5ms
    }
}
