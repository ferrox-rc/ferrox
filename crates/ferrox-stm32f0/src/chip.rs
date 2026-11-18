use ferrox_core::hal::SystemDriver;
use stm32f0xx_hal::pac;

pub struct Stm32f0SystemDriver;

impl Stm32f0SystemDriver {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for Stm32f0SystemDriver {
    fn default() -> Self {
        Self::new()
    }
}

/// Memory addresses for supported STM32F0 / APM32F0 chips.
#[cfg(feature = "apm32")]
const UID_BASE_ADDR: usize = 0x1FFF_F7E8;
#[cfg(not(feature = "apm32"))]
const UID_BASE_ADDR: usize = 0x1FFF_F7AC;

#[cfg(feature = "apm32")]
const DFU_BOOTLOADER_ADDR: usize = 0x1FFF_F000;
#[cfg(not(feature = "apm32"))]
const DFU_BOOTLOADER_ADDR: usize = 0x1FFF_C800;

impl SystemDriver for Stm32f0SystemDriver {
    fn pre_init(&mut self) {}

    fn check_and_clear_reset_flags(&mut self) -> bool {
        check_and_clear_reset_flags()
    }

    fn init_system_clock(&mut self) {
        init_system_clock();
    }

    fn mcu_name(&self) -> &'static str {
        #[cfg(feature = "apm32")]
        {
            "APM32F072VB"
        }
        #[cfg(not(feature = "apm32"))]
        {
            "STM32F072VB"
        }
    }

    fn read_uid(&mut self) -> [u8; 12] {
        unsafe { ferrox_core::chip::read_uid_from_addr(UID_BASE_ADDR) }
    }

    fn init_time(&mut self) {
        crate::time::init();
    }

    fn enter_dfu_bootloader(&mut self) -> ! {
        enter_dfu_bootloader()
    }
}

/// Initialize system clock to 48 MHz using external 8 MHz crystal (HSE) + PLL.
pub fn init_system_clock() {
    let rcc = unsafe { &*pac::RCC::ptr() };
    let flash = unsafe { &*pac::FLASH::ptr() };

    unsafe {
        // 1. Enable HSE (8 MHz external crystal)
        rcc.cr.modify(|_, w| w.hseon().set_bit());

        // 2. Wait for HSERDY with timeout
        let mut timeout = 100_000u32;
        while !rcc.cr.read().hserdy().bit() && timeout > 0 {
            timeout -= 1;
        }

        if timeout > 0 {
            // 3. Set Flash latency to 1 wait state (required for 48 MHz) & enable prefetch buffer
            flash.acr.write(|w| w.latency().ws1().prftbe().enabled());

            // 4. Configure PLL: HSE / 1 * 6 = 48 MHz
            let cfgr = rcc.cfgr.read().bits();
            let cfgr_clean = cfgr & !(0x003F_0000 | 0x0000_0003);
            rcc.cfgr
                .write(|w| w.bits(cfgr_clean | (1 << 16) | 0x0010_0000));

            // 5. Enable PLL (PLLON = bit 24)
            rcc.cr.modify(|_, w| w.pllon().set_bit());

            // 6. Wait for PLLRDY (bit 25)
            timeout = 100_000;
            while !rcc.cr.read().pllrdy().bit() && timeout > 0 {
                timeout -= 1;
            }

            if timeout > 0 {
                // 7. Select PLL as system clock source (SW = 0b10)
                let cfgr = rcc.cfgr.read().bits();
                rcc.cfgr.write(|w| w.bits((cfgr & !0x03) | 0x02));

                // 8. Wait for PLL to be used as system clock source (SWS = 0b1000)
                timeout = 100_000;
                while (rcc.cfgr.read().bits() & 0x0C) != 0x08 && timeout > 0 {
                    timeout -= 1;
                }

                // 9. Select PLL (48 MHz) as USB clock source (USBSW = 1 in RCC_CFGR3 bit 7)
                rcc.cfgr3.modify(|_, w| w.usbsw().pllclk());
            }
        }

        // 10. Enable LSI (40 kHz internal oscillator) and wait for stability
        rcc.csr.modify(|_, w| w.lsion().set_bit());
        let mut lsi_timeout = 100_000u32;
        while !rcc.csr.read().lsirdy().bit() && lsi_timeout > 0 {
            lsi_timeout -= 1;
        }
    }
}

/// Check if the MCU was rebooted warm by the hardware watchdog (IWDG or WWDG)
/// or a software reset, and clear reset flags in RCC_CSR.
pub fn check_and_clear_reset_flags() -> bool {
    let rcc = unsafe { &*pac::RCC::ptr() };
    let csr = rcc.csr.read();
    let was_warm_reset =
        csr.iwdgrstf().bit_is_set() || csr.wwdgrstf().bit_is_set() || csr.sftrstf().bit_is_set();

    // Clear all reset source flags by setting RMVF
    rcc.csr.modify(|_, w| w.rmvf().set_bit());

    was_warm_reset
}

/// Reset RCC to default power-on state (HSI 8MHz, no PLL, peripheral clocks disabled).
unsafe fn rcc_deinit() {
    let rcc = &*pac::RCC::ptr();
    let gpioc = &*pac::GPIOC::ptr();
    let gpiof = &*pac::GPIOF::ptr();

    // Switch PC9 (LCD backlight) from TIM3_CH4 AF to steady GPIO output HIGH
    // and ensure PF3 (stock backlight) stays HIGH before TIM3 is reset on APB1.
    rcc.ahbenr
        .modify(|r, w| w.bits(r.bits() | (1 << 19) | (1 << 22)));
    gpioc
        .moder
        .modify(|r, w| w.bits((r.bits() & !(3 << 18)) | (1 << 18)));
    gpioc.bsrr.write(|w| w.bits(1 << 9));
    gpiof.bsrr.write(|w| w.bits(1 << 3));

    // Turn on HSI
    rcc.cr.modify(|_, w| w.hsion().set_bit());
    let mut timeout = 100_000u32;
    while rcc.cr.read().hsirdy().bit_is_clear() && timeout > 0 {
        timeout -= 1;
    }

    // Reset CFGR
    rcc.cfgr.write(|w| w.bits(0x0000_0000));

    // Reset HSEON, CSSON, PLLON
    rcc.cr.modify(|_, w| {
        w.hseon()
            .clear_bit()
            .csson()
            .clear_bit()
            .pllon()
            .clear_bit()
    });

    // Reset CIR interrupts
    rcc.cir.write(|w| w.bits(0x0000_0000));

    // Reset APB peripherals
    rcc.apb2rstr.write(|w| w.bits(0xFFFF_FFFF));
    rcc.apb2rstr.write(|w| w.bits(0x0000_0000));
    rcc.apb1rstr.write(|w| w.bits(0xFFFF_FFFF));
    rcc.apb1rstr.write(|w| w.bits(0x0000_0000));

    // Reset peripheral clocks except SYSCFG, SRAM, and GPIOB..GPIOF (preserving PB15 power latch,
    // PB4 LCD RST, and PC9/PF3 LCD backlight while leaving GPIOA in reset state for USB DFU).
    rcc.ahbenr
        .write(|w| w.bits(0x0000_0014 | (1 << 18) | (1 << 19) | (1 << 20) | (1 << 21) | (1 << 22)));
    rcc.apb2enr.write(|w| w.bits(0x0000_0001)); // SYSCFGEN
    rcc.apb1enr.write(|w| w.bits(0x0000_0000));
}

/// Perform a clean software jump to the MCU's built-in factory DFU bootloader.
pub fn enter_dfu_bootloader() -> ! {
    unsafe {
        rcc_deinit();
        ferrox_core::chip::jump_to_system_bootloader(DFU_BOOTLOADER_ADDR, 1, || {
            let syscfg = &*pac::SYSCFG::ptr();
            syscfg
                .cfgr1
                .modify(|r, w| w.bits((r.bits() & !0x03) | 0x01));
        })
    }
}
