use core::ptr;
use ferrox_core::hal::SystemDriver;
use stm32f4::stm32f407 as pac;

pub struct Stm32f4SystemDriver;

impl Stm32f4SystemDriver {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for Stm32f4SystemDriver {
    fn default() -> Self {
        Self::new()
    }
}

pub const DFU_MAGIC_FLAG: u32 = 0xDEAD_BEEF;

use core::mem::MaybeUninit;

#[link_section = ".uninit"]
static mut DFU_RETAINED_FLAG: MaybeUninit<u32> = MaybeUninit::uninit();

#[cfg(feature = "edgetx-bootloader")]
const EDGETX_FLAVOUR: &str = if cfg!(feature = "board-zorro") {
    "zorro"
} else if cfg!(feature = "board-boxer") {
    "boxer"
} else if cfg!(feature = "board-tx12") {
    "tx12mk2"
} else if cfg!(feature = "board-qx7") {
    "x7"
} else if cfg!(feature = "board-tprov2") {
    "tprov2"
} else if cfg!(feature = "board-t12") {
    "t12"
} else if cfg!(feature = "board-x9d") {
    "x9d"
} else if cfg!(feature = "board-x9e") {
    "x9e"
} else if cfg!(feature = "board-gx12") {
    "gx12"
} else if cfg!(feature = "board-at9spro") {
    "at9spro"
} else {
    "pocket"
};

#[cfg(feature = "edgetx-bootloader")]
const fn make_fw_version_tag(flavour: &str, version: &str) -> [u8; 48] {
    let mut buf = [0u8; 48];
    let prefix = b"edgetx-";
    let mut i = 0;
    while i < prefix.len() && i < 47 {
        buf[i] = prefix[i];
        i += 1;
    }
    let fb = flavour.as_bytes();
    let mut j = 0;
    while j < fb.len() && i < 47 {
        buf[i] = fb[j];
        i += 1;
        j += 1;
    }
    if i < 47 {
        buf[i] = b'-';
        i += 1;
    }
    let vb = version.as_bytes();
    let mut k = 0;
    while k < vb.len() && i < 47 {
        buf[i] = vb[k];
        i += 1;
        k += 1;
    }
    buf
}

#[cfg(feature = "edgetx-bootloader")]
#[used]
#[link_section = ".fwversiondata"]
static EDGETX_FW_VERSION: [u8; 48] = make_fw_version_tag(
    EDGETX_FLAVOUR,
    concat!("ferrox-", env!("CARGO_PKG_VERSION")),
);

const UID_BASE_ADDR: usize = 0x1FFF_7A10;
const DFU_BOOTLOADER_ADDR: usize = 0x1FFF_0000;

impl SystemDriver for Stm32f4SystemDriver {
    fn pre_init(&mut self) {
        #[cfg(feature = "edgetx-bootloader")]
        unsafe {
            // 1. Keep interrupts disabled while tearing down bootloader state
            cortex_m::interrupt::disable();

            // 2. Disable SysTick left running by the bootloader
            let syst = &*cortex_m::peripheral::SYST::PTR;
            syst.csr.write(0);
            syst.rvr.write(0);
            syst.cvr.write(0);

            // 3. Disable and clear all pending NVIC interrupts from the bootloader
            let nvic = &*cortex_m::peripheral::NVIC::PTR;
            for i in 0..8 {
                nvic.icer[i].write(0xFFFF_FFFF);
                nvic.icpr[i].write(0xFFFF_FFFF);
            }

            // 4. Relocate vector table to 0x0800_8000 with memory barriers
            let scb = &*cortex_m::peripheral::SCB::PTR;
            scb.vtor.write(0x0800_8000);
            cortex_m::asm::dsb();
            cortex_m::asm::isb();

            // 5. Re-enable interrupts now that vector table and NVIC are clean
            cortex_m::interrupt::enable();
        }
    }

    fn check_and_clear_reset_flags(&mut self) -> bool {
        check_and_clear_reset_flags()
    }

    fn init_system_clock(&mut self) {
        init_system_clock();
    }

    fn mcu_name(&self) -> &'static str {
        "STM32F407VET6"
    }

    fn read_uid(&mut self) -> [u8; 12] {
        let mut uid = [0u8; 12];
        unsafe {
            let src = UID_BASE_ADDR as *const u8;
            for (i, byte) in uid.iter_mut().enumerate() {
                *byte = core::ptr::read_volatile(src.add(i));
            }
        }
        uid
    }

    fn init_time(&mut self) {
        crate::time::init();
    }

    fn delay_cycles(&mut self, n: u32) {
        cortex_m::asm::delay(n);
    }

    fn has_retained_dfu_flag(&mut self) -> bool {
        unsafe {
            let flag =
                core::ptr::read_volatile(core::ptr::addr_of!(DFU_RETAINED_FLAG).cast::<u32>());
            flag == DFU_MAGIC_FLAG
        }
    }

    fn set_retained_dfu_flag(&mut self) {
        unsafe {
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!(DFU_RETAINED_FLAG).cast::<u32>(),
                DFU_MAGIC_FLAG,
            );
        }
    }

    fn clear_retained_dfu_flag(&mut self) {
        unsafe {
            core::ptr::write_volatile(core::ptr::addr_of_mut!(DFU_RETAINED_FLAG).cast::<u32>(), 0);
        }
    }

    fn reboot_to_dfu(&mut self) -> ! {
        self.set_retained_dfu_flag();
        cortex_m::peripheral::SCB::sys_reset()
    }

    fn enter_dfu_bootloader(&mut self) -> ! {
        enter_dfu_bootloader()
    }
}

/// Initialize system clocks: 12 MHz HSE -> 168 MHz SYSCLK, 42 MHz APB1, 84 MHz APB2, 48 MHz USB.
pub fn init_system_clock() {
    let rcc = unsafe { &*pac::RCC::ptr() };
    let pwr = unsafe { &*pac::PWR::ptr() };
    let flash = unsafe { &*pac::FLASH::ptr() };

    unsafe {
        // 0. If a bootloader left PLL active as SYSCLK, switch back to HSI and disable PLL
        //    so RCC_PLLCFGR can be written cleanly.
        rcc.cr.modify(|r, w| w.bits(r.bits() | (1 << 0))); // HSION
        let mut hsi_timeout = 100_000u32;
        while (rcc.cr.read().bits() & (1 << 1)) == 0 && hsi_timeout > 0 {
            hsi_timeout -= 1;
        }
        rcc.cfgr.modify(|r, w| w.bits(r.bits() & !0x03)); // SW = 0b00 (HSI)
        let mut sw_timeout = 100_000u32;
        while (rcc.cfgr.read().bits() & 0x0C) != 0 && sw_timeout > 0 {
            sw_timeout -= 1;
        }
        rcc.cr.modify(|r, w| w.bits(r.bits() & !(1 << 24))); // PLLON = 0
        let mut pll_off_timeout = 100_000u32;
        while (rcc.cr.read().bits() & (1 << 25)) != 0 && pll_off_timeout > 0 {
            pll_off_timeout -= 1;
        }

        // 1. Enable PWR clock in RCC_APB1ENR (bit 28)
        rcc.apb1enr.modify(|r, w| w.bits(r.bits() | (1 << 28)));

        // 2. Set Voltage Scale 1 (VOS = 1, bit 14 in PWR_CR) for 168 MHz
        pwr.cr.modify(|r, w| w.bits(r.bits() | (1 << 14)));

        // 3. Enable HSE (12 MHz external crystal on RadioMaster Pocket)
        rcc.cr.modify(|r, w| w.bits(r.bits() | (1 << 16))); // HSEON

        let mut timeout = 100_000u32;
        while (rcc.cr.read().bits() & (1 << 17)) == 0 && timeout > 0 {
            timeout -= 1;
        }

        if timeout > 0 {
            // 4. Configure Flash latency: 5 Wait States (LATENCY = 5), enable ICEN, DCEN, PRFTEN
            flash
                .acr
                .write(|w| w.bits(0x05 | (1 << 8) | (1 << 9) | (1 << 10)));

            // 5. Configure PLL: HSE (12 MHz) / 12 * 336 / 2 = 168 MHz SYSCLK; / 7 = 48 MHz USB
            // PLLM = 12 (bits 0..5), PLLN = 336 (bits 6..14), PLLP = 0 (/2, bits 16..17),
            // PLLSRC = 1 (HSE, bit 22), PLLQ = 7 (bits 24..27)
            rcc.pllcfgr
                .write(|w| w.bits(12 | (336 << 6) | (0 << 16) | (1 << 22) | (7 << 24)));

            // 6. Enable PLL (PLLON = bit 24)
            rcc.cr.modify(|r, w| w.bits(r.bits() | (1 << 24)));

            timeout = 100_000;
            while (rcc.cr.read().bits() & (1 << 25)) == 0 && timeout > 0 {
                timeout -= 1;
            }

            if timeout > 0 {
                // 7. Configure Prescalers: HPRE = /1 (168MHz), PPRE1 = /4 (42MHz), PPRE2 = /2 (84MHz)
                // RCC_CFGR: bits 10..12 = 0b101 (/4), bits 13..15 = 0b100 (/2)
                let cfgr = rcc.cfgr.read().bits();
                let clean_cfgr = cfgr & !(0xF0 | (0x07 << 10) | (0x07 << 13));
                rcc.cfgr
                    .write(|w| w.bits(clean_cfgr | (0x05 << 10) | (0x04 << 13)));

                // 8. Select PLL as system clock source (SW = 0b10)
                rcc.cfgr.modify(|r, w| w.bits((r.bits() & !0x03) | 0x02));

                timeout = 100_000;
                while (rcc.cfgr.read().bits() & 0x0C) != 0x08 && timeout > 0 {
                    timeout -= 1;
                }
            }
        }

        // 9. Enable LSI (32 kHz internal oscillator) for hardware watchdog
        rcc.csr.modify(|r, w| w.bits(r.bits() | (1 << 0))); // LSION
        let mut lsi_timeout = 100_000u32;
        while (rcc.csr.read().bits() & (1 << 1)) == 0 && lsi_timeout > 0 {
            lsi_timeout -= 1;
        }
    }
}

pub fn check_and_clear_reset_flags() -> bool {
    let rcc = unsafe { &*pac::RCC::ptr() };
    let csr = rcc.csr.read().bits();
    // IWDGRSTF = bit 29, WWDGRSTF = bit 30, SFTRSTF = bit 28
    let was_warm_reset = (csr & ((1 << 28) | (1 << 29) | (1 << 30))) != 0;

    // Clear reset flags by setting RMVF (bit 24)
    rcc.csr
        .modify(|r, w| unsafe { w.bits(r.bits() | (1 << 24)) });

    was_warm_reset
}

unsafe fn rcc_deinit() {
    let rcc = &*pac::RCC::ptr();

    // Turn on HSI (16 MHz)
    rcc.cr.modify(|r, w| w.bits(r.bits() | (1 << 0)));
    let mut timeout = 100_000u32;
    while (rcc.cr.read().bits() & (1 << 1)) == 0 && timeout > 0 {
        timeout -= 1;
    }

    // Reset CFGR
    rcc.cfgr.write(|w| w.bits(0x0000_0000));

    // Disable HSE, CSS, PLL
    rcc.cr
        .modify(|r, w| w.bits(r.bits() & !((1 << 16) | (1 << 19) | (1 << 24))));

    // Reset CIR
    rcc.cir.write(|w| w.bits(0x0000_0000));

    // Reset AHB1/AHB2/AHB3 & APB1/APB2 peripherals
    rcc.apb2rstr.write(|w| w.bits(0xFFFF_FFFF));
    rcc.apb2rstr.write(|w| w.bits(0x0000_0000));
    rcc.apb1rstr.write(|w| w.bits(0xFFFF_FFFF));
    rcc.apb1rstr.write(|w| w.bits(0x0000_0000));

    // Keep GPIOD/GPIOE clocks enabled so PD0 power latch & backlight don't glitch
    rcc.ahb1enr.write(|w| w.bits((1 << 3) | (1 << 4))); // GPIODEN | GPIOEEN
    rcc.apb2enr.write(|w| w.bits(1 << 14)); // SYSCFGEN
    rcc.apb1enr.write(|w| w.bits(0x0000_0000));
}

pub fn enter_dfu_bootloader() -> ! {
    unsafe {
        rcc_deinit();

        let mut cp = cortex_m::peripheral::Peripherals::steal();
        cp.SYST.disable_counter();
        cp.SYST.disable_interrupt();
        cp.SYST.clear_current();
        cp.SCB.icsr.write(1 << 25); // PENDSTCLR

        for i in 0..8 {
            cp.NVIC.icer[i].write(0xFFFF_FFFF);
            cp.NVIC.icpr[i].write(0xFFFF_FFFF);
        }

        cortex_m::interrupt::disable();
        cortex_m::asm::dsb();

        // Remap System Memory to 0x0000_0000 via SYSCFG_MEMRMP (MEM_MODE = 0b01)
        let syscfg = &*pac::SYSCFG::ptr();
        syscfg
            .memrm
            .modify(|r, w| w.bits((r.bits() & !0x03) | 0x01));

        cortex_m::asm::dsb();
        cortex_m::asm::isb();

        let bootloader_addr = DFU_BOOTLOADER_ADDR;
        let initial_msp = ptr::read_volatile(bootloader_addr as *const u32);
        let reset_handler_addr = ptr::read_volatile((bootloader_addr + 4) as *const u32);

        cortex_m::interrupt::enable();
        cortex_m::asm::bootstrap(initial_msp as *const u32, reset_handler_addr as *const u32);
    }
}
