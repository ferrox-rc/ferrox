//! STM32F072 hardware TIM15 and GPIO PF9/PF10 driver for Trainer / PPM Subsystem.

use ferrox_core::hal::TrainerDriver;
use stm32f0xx_hal::pac::interrupt;

// Hardware register base addresses for STM32F072
const RCC_AHBENR: *mut u32 = 0x4002_1014 as *mut u32;
const RCC_APB2ENR: *mut u32 = 0x4002_1018 as *mut u32;
const GPIOF_MODER: *mut u32 = 0x4800_1400 as *mut u32;
const GPIOF_PUPDR: *mut u32 = 0x4800_140C as *mut u32;
const GPIOF_AFRH: *mut u32 = 0x4800_1424 as *mut u32;
const GPIOF_BSRR: *mut u32 = 0x4800_1418 as *mut u32;

// TIM15 base: 0x4001_4000
const TIM15_CR1: *mut u32 = 0x4001_4000 as *mut u32;
const TIM15_DIER: *mut u32 = 0x4001_400C as *mut u32;
const TIM15_SR: *mut u32 = 0x4001_4010 as *mut u32;
const TIM15_EGR: *mut u32 = 0x4001_4014 as *mut u32;
const TIM15_CCMR1: *mut u32 = 0x4001_4018 as *mut u32;
const TIM15_CCER: *mut u32 = 0x4001_4020 as *mut u32;
const TIM15_PSC: *mut u32 = 0x4001_4028 as *mut u32;
const TIM15_ARR: *mut u32 = 0x4001_402C as *mut u32;
const TIM15_CCR1: *mut u32 = 0x4001_4034 as *mut u32;
const TIM15_CCR2: *mut u32 = 0x4001_4038 as *mut u32;

// NVIC registers
const NVIC_ISER: *mut u32 = 0xE000_E100 as *mut u32;
const NVIC_ICER: *mut u32 = 0xE000_E180 as *mut u32;
const NVIC_IPR5: *mut u32 = 0xE000_E414 as *mut u32;

static mut LAST_CAPT: u16 = 0;

pub struct Stm32f0TrainerDriver {
    ppm_out_enabled: bool,
    ppm_in_enabled: bool,
}

impl Stm32f0TrainerDriver {
    pub const fn new() -> Self {
        Self {
            ppm_out_enabled: false,
            ppm_in_enabled: false,
        }
    }

    unsafe fn update_timer_state(&self) {
        let mut dier = 0u32;
        if self.ppm_out_enabled {
            dier |= 1 << 2; // CC2IE
        }
        if self.ppm_in_enabled {
            dier |= 1 << 1; // CC1IE
        }

        core::ptr::write_volatile(TIM15_DIER, dier);

        if self.ppm_out_enabled || self.ppm_in_enabled {
            core::ptr::write_volatile(NVIC_ISER, 1 << 20); // Enable IRQ 20
            core::ptr::write_volatile(TIM15_CR1, 1); // Start CEN
        } else {
            core::ptr::write_volatile(TIM15_CR1, 0); // Stop CEN
            core::ptr::write_volatile(NVIC_ICER, 1 << 20); // Mask IRQ 20
            core::ptr::write_volatile(GPIOF_BSRR, 1 << 10); // Hold PF10 High in idle
        }
    }
}

impl Default for Stm32f0TrainerDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl TrainerDriver for Stm32f0TrainerDriver {
    fn init(&mut self, ppm_out_en: bool, ppm_in_en: bool) {
        self.ppm_out_enabled = ppm_out_en;
        self.ppm_in_enabled = ppm_in_en;

        unsafe {
            // 1. Enable GPIOF clock (bit 22 in RCC_AHBENR)
            let ahb = core::ptr::read_volatile(RCC_AHBENR);
            core::ptr::write_volatile(RCC_AHBENR, ahb | (1 << 22));

            // 2. Configure PF10 (PPM Out) as General Purpose Output (MODER bits 21:20 = 01)
            let f_moder = core::ptr::read_volatile(GPIOF_MODER);
            core::ptr::write_volatile(
                GPIOF_MODER,
                (f_moder & !(3 << 20) & !(3 << 18)) | (1 << 20) | (2 << 18),
            );
            // Default PF10 HIGH (idle PPM line)
            core::ptr::write_volatile(GPIOF_BSRR, 1 << 10);

            // 3. Configure PF9 (PPM In) as Alternate Function AF0 (TIM15_CH1) with pull-up
            let f_pupdr = core::ptr::read_volatile(GPIOF_PUPDR);
            core::ptr::write_volatile(GPIOF_PUPDR, (f_pupdr & !(3 << 18)) | (1 << 18));

            // AF0 for PF9 (AFRH bits 7:4 = 0000)
            let f_afrh = core::ptr::read_volatile(GPIOF_AFRH);
            core::ptr::write_volatile(GPIOF_AFRH, f_afrh & !(0xF << 4));

            // 4. Enable TIM15 clock in RCC_APB2ENR (bit 16)
            let apb2 = core::ptr::read_volatile(RCC_APB2ENR);
            core::ptr::write_volatile(RCC_APB2ENR, apb2 | (1 << 16));

            // 5. Configure TIM15: 48 MHz / 48 = 1 MHz count rate (1 tick = 1 µs)
            core::ptr::write_volatile(TIM15_PSC, 47);
            core::ptr::write_volatile(TIM15_ARR, 0xFFFF);

            // CCMR1:
            // CH1: CC1S = 01 (Input on TI1), IC1F = 0011 (filter 8 samples) -> 0x31
            // CH2: CC2S = 00 (Output Compare)
            core::ptr::write_volatile(TIM15_CCMR1, 0x0031);

            // CCER:
            // CH1: CC1E = 1 (bit 0), CC1P = 1 (bit 1, falling edge)
            // CH2: CC2E = 1 (bit 4)
            core::ptr::write_volatile(TIM15_CCER, (1 << 0) | (1 << 1) | (1 << 4));

            // Schedule first compare match for PPM Out
            core::ptr::write_volatile(TIM15_CCR2, 400);

            // Re-initialize counter & prescaler
            core::ptr::write_volatile(TIM15_EGR, 1);
            core::ptr::write_volatile(TIM15_SR, 0);

            // Configure NVIC for TIM15 (IRQ 20) with priority 0x90
            let ipr5 = core::ptr::read_volatile(NVIC_IPR5);
            core::ptr::write_volatile(NVIC_IPR5, (ipr5 & !0xFF) | 0x90);

            self.update_timer_state();
        }
    }

    fn set_ppm_out(&mut self, enabled: bool) {
        self.ppm_out_enabled = enabled;
        unsafe {
            self.update_timer_state();
        }
    }

    fn set_ppm_in(&mut self, enabled: bool) {
        self.ppm_in_enabled = enabled;
        unsafe {
            self.update_timer_state();
        }
    }
}

#[interrupt]
fn TIM15() {
    unsafe {
        let sr = core::ptr::read_volatile(TIM15_SR);

        // 1. Channel 2: PPM Output Compare Match (PF10)
        if (sr & (1 << 2)) != 0 {
            core::ptr::write_volatile(TIM15_SR, !(1u32 << 2));

            let (duration_us, pin_high) = ferrox_core::trainer::on_ppm_out_event();
            if duration_us > 0 {
                if pin_high {
                    core::ptr::write_volatile(GPIOF_BSRR, 1 << 10);
                } else {
                    core::ptr::write_volatile(GPIOF_BSRR, 1 << (10 + 16));
                }
                let ccr2 = core::ptr::read_volatile(TIM15_CCR2) as u16;
                core::ptr::write_volatile(TIM15_CCR2, ccr2.wrapping_add(duration_us) as u32);
            } else {
                core::ptr::write_volatile(GPIOF_BSRR, 1 << 10);
            }
        }

        // 2. Channel 1: PPM Input Capture (PF9)
        if (sr & (1 << 1)) != 0 {
            core::ptr::write_volatile(TIM15_SR, !(1u32 << 1));

            let capt = core::ptr::read_volatile(TIM15_CCR1) as u16;
            let delta = capt.wrapping_sub(LAST_CAPT);
            LAST_CAPT = capt;

            ferrox_core::trainer::on_ppm_in_pulse(delta);
        }
    }
}
