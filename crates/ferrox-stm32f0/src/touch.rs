//! Hardware FT6236 capacitive touchscreen driver for STM32F072 (FlySky FS-i6S).
//!
//! Pin Assignments:
//! - PB8:  I2C1_SCL (AF1, Open-Drain, High-Speed, Pull-up) @ 400 kHz Fast Mode
//! - PB9:  I2C1_SDA (AF1, Open-Drain, High-Speed, Pull-up) @ 400 kHz Fast Mode
//! - PA15: TOUCH_RST (Active LOW Hardware Reset, pulsed LOW for 20 ms at init)
//! - PC12: TOUCH_INT (Active LOW Interrupt / Touch Detected)

use core::sync::atomic::{AtomicU8, Ordering};
use ferrox_core::hal::TouchDriver;
use ferrox_core::touch::ft6236::FT6236_I2C_ADDR;
use stm32f0xx_hal::pac;

static I2C_FAIL_COUNT: AtomicU8 = AtomicU8::new(0);

pub struct Stm32f0TouchDriver;

impl Stm32f0TouchDriver {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for Stm32f0TouchDriver {
    fn default() -> Self {
        Self::new()
    }
}

/// Clear latched I2C1 status flags using the STM32F0 ICR register (RM0091 §26.7.6).
#[inline(always)]
unsafe fn clear_i2c1_flags(i2c1: &pac::i2c1::RegisterBlock) {
    // Bit 3: ADDRCF, Bit 4: NACKCF, Bit 5: STOPCF, Bit 8: BERRCF, Bit 9: ARLOCF,
    // Bit 10: OVRCF, Bit 11: PECCF, Bit 12: TIMOUTCF, Bit 13: ALERTCF (0x3F38)
    i2c1.icr.write(|w| w.bits(0x3F38));
}

/// Abort a failed transfer and return I2C1 peripheral to a clean enabled state (RM0091 §26.4.3).
fn recover_i2c1() {
    unsafe {
        let rcc = &*pac::RCC::ptr();
        let i2c1 = &*pac::I2C1::ptr();

        // Request STOP on any active transfer, then wait briefly for STOPF
        i2c1.cr2.modify(|r, w| w.bits(r.bits() | (1 << 14)));
        let mut timeout = 2_000u32;
        while !i2c1.isr.read().stopf().bit() && timeout > 0 {
            timeout -= 1;
        }

        // Disable peripheral (RM0091 §26.4.3: PE=0 must be held >= 3 APB cycles) and clear flags
        i2c1.cr1.modify(|_, w| w.pe().clear_bit());
        cortex_m::asm::nop();
        cortex_m::asm::nop();
        cortex_m::asm::nop();
        cortex_m::asm::nop();
        clear_i2c1_flags(i2c1);

        // Reset I2C1 block via APB1 peripheral reset (bit 21)
        rcc.apb1rstr.modify(|r, w| w.bits(r.bits() | (1 << 21)));
        rcc.apb1rstr.modify(|r, w| w.bits(r.bits() & !(1 << 21)));
        let _ = rcc.apb1rstr.read().bits();

        // Restore timing and re-enable peripheral
        i2c1.cr1.modify(|_, w| w.pe().clear_bit());
        i2c1.timingr.write(|w| w.bits(0x00B0_1A4B));
        clear_i2c1_flags(i2c1);
        i2c1.cr1.modify(|_, w| w.pe().set_bit());
    }
}

/// Write a single 8-bit value to an FT6236 register at address `0x38`.
fn write_reg(reg: u8, val: u8) -> bool {
    unsafe {
        let i2c1 = &*pac::I2C1::ptr();

        // SADD = 0x38 << 1, NBYTES = 2, RD_WRN = 0 (write), START = 1, AUTOEND = 1
        let cr2_val = ((FT6236_I2C_ADDR as u32) << 1) | (2 << 16) | (1 << 13) | (1 << 25);
        i2c1.cr2.write(|w| w.bits(cr2_val));

        // Write register address
        let mut timeout = 10_000u32;
        while !i2c1.isr.read().txis().bit() && timeout > 0 {
            let isr = i2c1.isr.read();
            if isr.nackf().bit() || isr.berr().bit() || isr.arlo().bit() || isr.ovr().bit() {
                recover_i2c1();
                return false;
            }
            timeout -= 1;
        }
        if timeout == 0 {
            recover_i2c1();
            return false;
        }
        i2c1.txdr.write(|w| w.txdata().bits(reg));

        // Write register value
        timeout = 10_000;
        while !i2c1.isr.read().txis().bit() && timeout > 0 {
            let isr = i2c1.isr.read();
            if isr.nackf().bit() || isr.berr().bit() || isr.arlo().bit() || isr.ovr().bit() {
                recover_i2c1();
                return false;
            }
            timeout -= 1;
        }
        if timeout == 0 {
            recover_i2c1();
            return false;
        }
        i2c1.txdr.write(|w| w.txdata().bits(val));

        // Wait for STOPF
        timeout = 10_000;
        while !i2c1.isr.read().stopf().bit() && timeout > 0 {
            let isr = i2c1.isr.read();
            if isr.berr().bit() || isr.arlo().bit() || isr.ovr().bit() {
                recover_i2c1();
                return false;
            }
            timeout -= 1;
        }
        if timeout == 0 {
            recover_i2c1();
            return false;
        }
        i2c1.icr.write(|w| w.stopcf().set_bit());
        true
    }
}

/// Hard reset FT6236 controller by toggling PA15 LOW for 20 ms, re-initializing I2C1 and registers.
fn reset_touch_controller() {
    unsafe {
        let gpioa = &*pac::GPIOA::ptr();

        // 1. Recover/reset I2C1 peripheral first
        recover_i2c1();

        // 2. Pulse PA15 LOW for ~20 ms @ 48 MHz to hard-reset the FT6236 controller
        gpioa.bsrr.write(|w| w.bits(1 << (15 + 16)));
        cortex_m::asm::delay(48_000 * 20);
        gpioa.bsrr.write(|w| w.bits(1 << 15));
        cortex_m::asm::delay(48_000 * 50);

        // 3. Configure FT6236 registers
        cortex_m::asm::delay(48_000 * 10);
        let _ = write_reg(0xA4, 0x00);
        let _ = write_reg(0x80, 22);
        let _ = write_reg(0x88, 14);
    }
}

fn record_i2c_failure() {
    recover_i2c1();
    let fails = I2C_FAIL_COUNT.load(Ordering::Relaxed).saturating_add(1);
    if fails >= 5 {
        I2C_FAIL_COUNT.store(0, Ordering::Relaxed);
        reset_touch_controller();
    } else {
        I2C_FAIL_COUNT.store(fails, Ordering::Relaxed);
    }
}

#[inline(always)]
fn record_i2c_success() {
    I2C_FAIL_COUNT.store(0, Ordering::Relaxed);
}

impl TouchDriver for Stm32f0TouchDriver {
    fn init(&mut self) {
        unsafe {
            let rcc = &*pac::RCC::ptr();
            let gpioa = &*pac::GPIOA::ptr();
            let gpiob = &*pac::GPIOB::ptr();
            let gpioc = &*pac::GPIOC::ptr();
            let i2c1 = &*pac::I2C1::ptr();

            // 1. Enable clocks for GPIOA, GPIOB, GPIOC, and I2C1 (ES0260 dummy read-back)
            rcc.ahbenr
                .modify(|r, w| w.bits(r.bits() | (1 << 17) | (1 << 18) | (1 << 19)));
            let _ = rcc.ahbenr.read().bits();
            rcc.apb1enr.modify(|r, w| w.bits(r.bits() | (1 << 21)));
            let _ = rcc.apb1enr.read().bits();

            // 2. Configure I2C1 clock source in RCC_CFGR3: Select SYSCLK (48 MHz, I2C1SW = bit 4)
            rcc.cfgr3.modify(|r, w| w.bits(r.bits() | (1 << 4)));

            // 3. Configure PA15 (TOUCH_RST): General Purpose Output (MODER = 01)
            gpioa.moder.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !(3 << 30)) | (1 << 30))
            });

            // 4. Configure PC12 (TOUCH_INT): Input (MODER = 00) with pull-up (PUPDR = 01)
            gpioc.moder.modify(|r, w| w.bits(r.bits() & !(3 << 24)));
            gpioc.pupdr.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !(3 << 24)) | (1 << 24))
            });

            // 5. Configure PB8 (SCL) and PB9 (SDA): AF1, Open-Drain, High-Speed, Pull-up
            gpiob.moder.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !(0x0F << 16)) | (0x0A << 16))
            });
            gpiob
                .otyper
                .modify(|r, w| w.bits(r.bits() | (1 << 8) | (1 << 9)));
            gpiob.ospeedr.modify(|r, w| w.bits(r.bits() | (0x0F << 16)));
            gpiob.pupdr.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !(0x0F << 16)) | (0x05 << 16))
            });
            gpiob.afrh.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !0xFF) | 0x11)
            });

            // 6. Pulse PA15 LOW for 20 ms @ 48 MHz to reset FT6236 controller
            gpioa.bsrr.write(|w| w.bits(1 << (15 + 16)));
            cortex_m::asm::delay(48_000 * 20);
            gpioa.bsrr.write(|w| w.bits(1 << 15));
            cortex_m::asm::delay(48_000 * 50);

            // 7. Reset I2C1 peripheral through RCC and configure 400 kHz Fast Mode @ 48 MHz SYSCLK
            i2c1.cr1.modify(|_, w| w.pe().clear_bit());
            rcc.apb1rstr.modify(|r, w| w.bits(r.bits() | (1 << 21)));
            rcc.apb1rstr.modify(|r, w| w.bits(r.bits() & !(1 << 21)));
            let _ = rcc.apb1rstr.read().bits();
            i2c1.cr1.modify(|_, w| w.pe().clear_bit());
            i2c1.timingr.write(|w| w.bits(0x00B0_1A4B));
            clear_i2c1_flags(i2c1);
            i2c1.cr1.modify(|_, w| w.pe().set_bit());

            // 8. Configure FT6236 registers after 10 ms internal state machine ready delay:
            // - 0xA4 = 0x00: Polling Mode (PC12 stays active LOW for entire touch/gesture duration)
            // - 0x80 = 22:   Touch detection threshold
            // - 0x88 = 14:   Active scan report rate (~70 Hz)
            cortex_m::asm::delay(48_000 * 10);
            let _ = write_reg(0xA4, 0x00);
            let _ = write_reg(0x80, 22);
            let _ = write_reg(0x88, 14);
        }
    }

    fn is_touch_int_asserted(&mut self) -> bool {
        unsafe {
            let gpioc = &*pac::GPIOC::ptr();
            (gpioc.idr.read().bits() & (1 << 12)) == 0
        }
    }

    fn read_touch(&mut self) -> Option<[u8; 7]> {
        unsafe {
            let i2c1 = &*pac::I2C1::ptr();

            // 1. Write register start address (0x00)
            // SADD = 0x38 << 1, NBYTES = 1, RD_WRN = 0 (write), START = 1, AUTOEND = 0
            let cr2_val = ((FT6236_I2C_ADDR as u32) << 1) | (1 << 16) | (1 << 13);
            i2c1.cr2.write(|w| w.bits(cr2_val));

            let mut timeout = 10_000u32;
            while !i2c1.isr.read().txis().bit() && timeout > 0 {
                let isr = i2c1.isr.read();
                if isr.nackf().bit() || isr.berr().bit() || isr.arlo().bit() || isr.ovr().bit() {
                    record_i2c_failure();
                    return None;
                }
                timeout -= 1;
            }
            if timeout == 0 {
                record_i2c_failure();
                return None;
            }

            i2c1.txdr.write(|w| w.txdata().bits(0x00));

            // Wait for TC (Transfer Complete)
            timeout = 10_000;
            while !i2c1.isr.read().tc().bit() && timeout > 0 {
                let isr = i2c1.isr.read();
                if isr.nackf().bit() || isr.berr().bit() || isr.arlo().bit() || isr.ovr().bit() {
                    record_i2c_failure();
                    return None;
                }
                timeout -= 1;
            }
            if timeout == 0 {
                record_i2c_failure();
                return None;
            }

            // 2. Repeated START to read 7 bytes (0x00..0x06)
            // SADD = 0x38 << 1, NBYTES = 7, RD_WRN = 1 (read), START = 1, AUTOEND = 1
            let cr2_read =
                ((FT6236_I2C_ADDR as u32) << 1) | (7 << 16) | (1 << 10) | (1 << 13) | (1 << 25);
            i2c1.cr2.write(|w| w.bits(cr2_read));

            let mut buf = [0u8; 7];
            for byte in buf.iter_mut() {
                timeout = 10_000;
                while !i2c1.isr.read().rxne().bit() && timeout > 0 {
                    let isr = i2c1.isr.read();
                    if isr.nackf().bit() || isr.berr().bit() || isr.arlo().bit() || isr.ovr().bit()
                    {
                        record_i2c_failure();
                        return None;
                    }
                    timeout -= 1;
                }
                if timeout == 0 {
                    record_i2c_failure();
                    return None;
                }
                *byte = i2c1.rxdr.read().rxdata().bits();
            }

            // Wait for STOPF and clear flag
            timeout = 10_000;
            while !i2c1.isr.read().stopf().bit() && timeout > 0 {
                let isr = i2c1.isr.read();
                if isr.berr().bit() || isr.arlo().bit() || isr.ovr().bit() {
                    record_i2c_failure();
                    return None;
                }
                timeout -= 1;
            }
            if timeout == 0 {
                record_i2c_failure();
                return None;
            }
            i2c1.icr.write(|w| w.stopcf().set_bit());

            record_i2c_success();
            Some(buf)
        }
    }
}
