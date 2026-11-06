//! Hardware SPI1 and RF frontend control for Amiccom A7105 on FlySky FS-i6X and FS-i6S.

use ferrox_core::hal::RfBus;
use stm32f0xx_hal::pac::{self, interrupt};

pub const RF_MODE_RX_EN: u8 = 0b0000_0001;
pub const RF_MODE_TX_EN: u8 = 0b0000_0010;
pub const RF_MODE_OFF: u8 = 0b0000_0011;

static mut ANTENNA_STATE: u8 = 0;

pub struct Stm32f0RfBus;

impl Stm32f0RfBus {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for Stm32f0RfBus {
    fn default() -> Self {
        Self::new()
    }
}

impl RfBus for Stm32f0RfBus {
    fn init(&mut self) {
        unsafe {
            let rcc = &*pac::RCC::ptr();
            let gpiob = &*pac::GPIOB::ptr();
            let gpioe = &*pac::GPIOE::ptr();
            let syscfg = &*pac::SYSCFG::ptr();
            let exti = &*pac::EXTI::ptr();
            let spi1 = &*pac::SPI1::ptr();

            // 1. Enable peripheral clocks
            rcc.ahbenr
                .modify(|_, w| w.iopben().set_bit().iopeen().set_bit());

            rcc.apb2enr.modify(|_, w| {
                w.syscfgen()
                    .set_bit()
                    .spi1en()
                    .set_bit()
                    .tim16en()
                    .set_bit()
            });

            // 2. Configure GPIOE pins:
            // PE13 (SCK), PE14 (MISO), PE15 (MOSI) -> Alternate Function (0b10)
            // PE12 (CSN) -> Output (0b01)
            // PE8 (RX_EN), PE9 (TX_EN) -> Output (0b01)
            // PE10 (RF0), PE11 (RF1) -> Output (0b01)
            let moder = gpioe.moder.read().bits();
            let moder_cleared = moder
                & !(
                    (0b11 << 16) | // PE8
                    (0b11 << 18) | // PE9
                    (0b11 << 20) | // PE10
                    (0b11 << 22) | // PE11
                    (0b11 << 24) | // PE12
                    (0b11 << 26) | // PE13
                    (0b11 << 28) | // PE14
                    (0b11 << 30)   // PE15
                );
            let moder_set = (0b01 << 16)   // PE8 Output
                | (0b01 << 18)             // PE9 Output
                | (0b01 << 20)             // PE10 Output
                | (0b01 << 22)             // PE11 Output
                | (0b01 << 24)             // PE12 Output (CSN)
                | (0b10 << 26)             // PE13 AF (SCK)
                | (0b10 << 28)             // PE14 AF (MISO)
                | (0b10 << 30);            // PE15 AF (MOSI)
            gpioe.moder.write(|w| w.bits(moder_cleared | moder_set));

            // High speed for SPI pins PE13, PE14, PE15
            gpioe
                .ospeedr
                .modify(|r, w| w.bits(r.bits() | (0b11 << 26) | (0b11 << 28) | (0b11 << 30)));

            // Set AF1 (SPI1) for PE13, PE14, PE15 in AFRH
            let afrh = gpioe.afrh.read().bits();
            let afrh_cleared = afrh & !(0x0FFF << 20);
            let afrh_set = (1 << 20) | (1 << 24) | (1 << 28);
            gpioe.afrh.write(|w| w.bits(afrh_cleared | afrh_set));

            // CSN starts HIGH (idle, deselected)
            gpioe.bsrr.write(|w| w.bs12().set_bit());

            // Turn front-end OFF initially
            self.set_tx_rx_mode(RF_MODE_OFF);

            // Antenna 0 selected by default (RF0=1, RF1=0)
            gpioe.bsrr.write(|w| w.bits((1 << 10) | (1 << (11 + 16))));

            // 3. Configure PB2 (GIO2 / WTR) as input with pull-up
            gpiob.moder.modify(|r, w| w.bits(r.bits() & !(0b11 << 4)));
            gpiob
                .pupdr
                .modify(|r, w| w.bits((r.bits() & !(0b11 << 4)) | (0b01 << 4)));

            // Route EXTI2 to PB2 in SYSCFG_EXTICR1
            syscfg
                .exticr1
                .modify(|r, w| w.bits((r.bits() & !(0x0F << 8)) | (0x01 << 8)));

            // Falling edge trigger for EXTI2
            exti.ftsr.modify(|_, w| w.tr2().set_bit());

            // Clear any pending EXTI2 flag
            exti.pr.write(|w| w.bits(1 << 2));

            // 4. Configure SPI1:
            spi1.cr1.write(|w| w.bits(0));

            let cr1 = (1 << 2) | (1 << 4) | (1 << 8) | (1 << 9);
            spi1.cr1.write(|w| w.bits(cr1));

            // CR2: DS = 0b0111 (8-bit data), FRXTH = 1 (1/4 threshold)
            let cr2 = (0b0111 << 8) | (1 << 12);
            spi1.cr2.write(|w| w.bits(cr2));

            // Enable SPI1 (SPE = bit 6)
            spi1.cr1.write(|w| w.bits(cr1 | (1 << 6)));

            // Drain any stale data in RX FIFO (max 16 bytes)
            let mut drain = 16;
            while spi1.sr.read().rxne().bit_is_set() && drain > 0 {
                drain -= 1;
                let _ =
                    core::ptr::read_volatile(core::ptr::addr_of!((*pac::SPI1::ptr()).dr) as *const u8);
            }
        }
    }

    fn init_timer(&mut self, period_us: u16) {
        unsafe {
            let tim16 = &*pac::TIM16::ptr();

            // Stop timer
            tim16.cr1.write(|w| w.bits(0));

            // PSC = 47 -> 48 MHz / (47 + 1) = 1 MHz = 1.0 µs per tick
            tim16.psc.write(|w| w.psc().bits(47));
            // ARR = period_us - 1
            tim16
                .arr
                .write(|w| w.arr().bits(period_us.saturating_sub(1)));

            // Clear update interrupt flag
            tim16.sr.write(|w| w.bits(0));

            // Enable Update Interrupt (UIE = bit 0)
            tim16.dier.write(|w| w.uie().set_bit());

            // Set priority for EXTI2_3 (IRQ 6) and TIM16 (IRQ 21) in NVIC
            let mut cp = cortex_m::peripheral::Peripherals::steal();
            cp.NVIC.set_priority(pac::Interrupt::EXTI2_3, 0x80);
            cp.NVIC.set_priority(pac::Interrupt::TIM16, 0x80);

            // Enable IRQ 6 (EXTI2_3) and IRQ 21 (TIM16)
            cortex_m::peripheral::NVIC::unmask(pac::Interrupt::EXTI2_3);
            cortex_m::peripheral::NVIC::unmask(pac::Interrupt::TIM16);

            // Start timer (CEN = bit 0)
            tim16.cr1.write(|w| w.cen().set_bit());
        }
    }

    fn clear_timer_flag(&mut self) {
        unsafe {
            let tim16 = &*pac::TIM16::ptr();
            tim16.sr.write(|w| w.bits(0));
        }
    }

    fn csn_low(&mut self) {
        unsafe {
            let spi1 = &*pac::SPI1::ptr();
            let gpioe = &*pac::GPIOE::ptr();
            let mut timeout = 10_000u32;
            while spi1.sr.read().bsy().bit_is_set() && timeout > 0 {
                timeout -= 1;
            }
            gpioe.bsrr.write(|w| w.br12().set_bit());
            cortex_m::asm::nop();
            cortex_m::asm::nop();
        }
    }

    fn csn_high(&mut self) {
        unsafe {
            let spi1 = &*pac::SPI1::ptr();
            let gpioe = &*pac::GPIOE::ptr();
            let mut timeout = 10_000u32;
            while spi1.sr.read().bsy().bit_is_set() && timeout > 0 {
                timeout -= 1;
            }
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            gpioe.bsrr.write(|w| w.bs12().set_bit());
            cortex_m::asm::nop();
            cortex_m::asm::nop();
        }
    }

    fn transfer_byte(&mut self, data: u8) -> u8 {
        unsafe {
            let spi1 = &*pac::SPI1::ptr();
            let mut timeout = 10_000u32;
            while spi1.sr.read().bsy().bit_is_set() && timeout > 0 {
                timeout -= 1;
            }
            timeout = 10_000;
            while spi1.sr.read().txe().bit_is_clear() && timeout > 0 {
                timeout -= 1;
            }
            core::ptr::write_volatile(core::ptr::addr_of!((*pac::SPI1::ptr()).dr) as *mut u8, data);
            timeout = 10_000;
            while spi1.sr.read().rxne().bit_is_clear() && timeout > 0 {
                timeout -= 1;
            }
            core::ptr::read_volatile(core::ptr::addr_of!((*pac::SPI1::ptr()).dr) as *const u8)
        }
    }

    fn set_tx_rx_mode(&mut self, mode: u8) {
        let tmp = ((mode << 1) & 0x02) | ((mode >> 1) & 0x01);
        unsafe {
            let val = (0x0300u32 << 16) | ((tmp as u32) << 8);
            let gpioe = &*pac::GPIOE::ptr();
            gpioe.bsrr.write(|w| w.bits(val));
        }
    }

    fn switch_antenna(&mut self) {
        unsafe {
            let gpioe = &*pac::GPIOE::ptr();
            if ANTENNA_STATE == 0 {
                gpioe.bsrr.write(|w| w.bits((1 << 10) | (1 << (11 + 16))));
                ANTENNA_STATE = 1;
            } else {
                gpioe.bsrr.write(|w| w.bits((1 << (10 + 16)) | (1 << 11)));
                ANTENNA_STATE = 0;
            }
        }
    }

    fn enable_gio2_irq(&mut self) {
        unsafe {
            let exti = &*pac::EXTI::ptr();
            exti.pr.write(|w| w.bits(1 << 2));
            exti.imr.modify(|_, w| w.mr2().set_bit());
        }
    }

    fn disable_gio2_irq(&mut self) {
        unsafe {
            let exti = &*pac::EXTI::ptr();
            exti.imr.modify(|_, w| w.mr2().clear_bit());
        }
    }
}

#[interrupt]
fn TIM16() {
    ferrox_core::rf::on_timer_tick();
}

#[interrupt]
fn EXTI2_3() {
    unsafe {
        let exti = &*pac::EXTI::ptr();
        if (exti.pr.read().bits() & (1 << 2)) != 0 {
            exti.pr.write(|w| w.bits(1 << 2));
            ferrox_core::rf::on_gio2_event();
        }
    }
}
