use ferrox_core::hal::TrainerDriver;
use stm32f4::stm32f407 as pac;

pub struct Stm32f4TrainerDriver {
    out_enabled: bool,
    in_enabled: bool,
}

impl Stm32f4TrainerDriver {
    pub const fn new() -> Self {
        Self {
            out_enabled: false,
            in_enabled: false,
        }
    }
}

impl Default for Stm32f4TrainerDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl TrainerDriver for Stm32f4TrainerDriver {
    fn init(&mut self, ppm_out_en: bool, ppm_in_en: bool) {
        let rcc = unsafe { &*pac::RCC::ptr() };
        let gpioc = unsafe { &*pac::GPIOC::ptr() };

        unsafe {
            // Enable GPIOC (AHB1) and TIM3 (APB1)
            rcc.ahb1enr.modify(|r, w| w.bits(r.bits() | (1 << 2)));
            rcc.apb1enr.modify(|r, w| w.bits(r.bits() | (1 << 1)));

            // PC8 (PPM_IN): AF2 (TIM3_CH3)
            gpioc
                .moder
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 16)) | (0x02 << 16)));
            gpioc
                .afrh
                .modify(|r, w| w.bits((r.bits() & !(0x0F << 0)) | (0x02 << 0)));

            // PC9 (PPM_OUT): AF2 (TIM3_CH4)
            gpioc
                .moder
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 18)) | (0x02 << 18)));
            gpioc
                .afrh
                .modify(|r, w| w.bits((r.bits() & !(0x0F << 4)) | (0x02 << 4)));
        }

        self.set_ppm_out(ppm_out_en);
        self.set_ppm_in(ppm_in_en);
    }

    fn set_ppm_out(&mut self, enabled: bool) {
        self.out_enabled = enabled;
    }

    fn set_ppm_in(&mut self, enabled: bool) {
        self.in_enabled = enabled;
    }
}
