//! I2C EEPROM storage driver (FT24C128A 16 KB) for STM32F072.
//!
//! Provides hardware-level implementation of `StorageBus`.
//! Supports adaptive bus & address auto-detection:
//! - Primary: I2C2 on PB10 (SCL) and PB11 (SDA) across addresses 0x50..0x57
//! - Fallback: I2C1 on PB8 (SCL) and PB9 (SDA) across addresses 0x50..0x57

use ferrox_core::hal::StorageBus;
use ferrox_core::storage::{EEPROM_STATUS, PAGE_SIZE};
use stm32f0xx_hal::pac;

pub struct EepromI2cBus;

impl EepromI2cBus {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for EepromI2cBus {
    fn default() -> Self {
        Self::new()
    }
}

#[inline(always)]
fn clear_i2c_flags(i2c: &pac::i2c1::RegisterBlock) {
    unsafe {
        i2c.icr.write(|w| w.bits(0x3F38));
    }
}

fn recover_i2c(bus_num: u8) {
    let rcc = unsafe { &*pac::RCC::ptr() };
    unsafe {
        if bus_num == 1 {
            let i2c1 = &*pac::I2C1::ptr();
            i2c1.cr2.modify(|r, w| w.bits(r.bits() | (1 << 14)));
            let mut timeout = 2_000u32;
            while !i2c1.isr.read().stopf().bit() && timeout > 0 {
                timeout -= 1;
            }
            i2c1.cr1.modify(|_, w| w.pe().clear_bit());
            clear_i2c_flags(&*pac::I2C1::ptr());
            rcc.apb1rstr.modify(|r, w| w.bits(r.bits() | (1 << 21)));
            rcc.apb1rstr.modify(|r, w| w.bits(r.bits() & !(1 << 21)));
            i2c1.cr1.modify(|_, w| w.pe().clear_bit());
            i2c1.timingr.write(|w| w.bits(0x00B0_1A4B));
            clear_i2c_flags(&*pac::I2C1::ptr());
            i2c1.cr1.modify(|_, w| w.pe().set_bit());
        } else {
            let i2c2 = &*pac::I2C2::ptr();
            let i2c_block = &*pac::I2C2::ptr();
            i2c2.cr2.modify(|r, w| w.bits(r.bits() | (1 << 14)));
            let mut timeout = 2_000u32;
            while !i2c2.isr.read().stopf().bit() && timeout > 0 {
                timeout -= 1;
            }
            i2c2.cr1.modify(|_, w| w.pe().disabled());
            clear_i2c_flags(i2c_block);
            rcc.apb1rstr.modify(|r, w| w.bits(r.bits() | (1 << 22)));
            rcc.apb1rstr.modify(|r, w| w.bits(r.bits() & !(1 << 22)));
            i2c2.cr1.modify(|_, w| w.pe().disabled());
            i2c2.timingr.write(|w| {
                w.presc()
                    .bits(0)
                    .scldel()
                    .bits(3)
                    .sdadel()
                    .bits(0)
                    .sclh()
                    .bits(9)
                    .scll()
                    .bits(25)
            });
            clear_i2c_flags(i2c_block);
            i2c2.cr1.modify(|_, w| w.pe().enabled());
        }
    }
}

unsafe fn probe_device(i2c: &pac::i2c1::RegisterBlock, bus_num: u8, addr_7bit: u8) -> bool {
    clear_i2c_flags(i2c);

    // Standard I2C probe transaction: 0-byte write with AUTOEND
    i2c.cr2.write(|w| {
        w.sadd()
            .bits((addr_7bit as u16) << 1)
            .nbytes()
            .bits(0)
            .rd_wrn()
            .write()
            .autoend()
            .automatic()
            .start()
            .start()
    });

    let mut timeout = 20_000u32;
    let mut got_nack = false;
    while i2c.isr.read().stopf().bit_is_clear() && timeout > 0 {
        let isr = i2c.isr.read();
        if isr.berr().bit_is_set() || isr.arlo().bit_is_set() || isr.ovr().bit_is_set() {
            clear_i2c_flags(i2c);
            recover_i2c(bus_num);
            return false;
        }
        if isr.nackf().bit_is_set() {
            got_nack = true;
            i2c.icr.write(|w| w.nackcf().clear());
        }
        timeout -= 1;
    }

    if timeout == 0 {
        clear_i2c_flags(i2c);
        recover_i2c(bus_num);
        return false;
    }

    i2c.icr.write(|w| w.stopcf().clear());
    !got_nack
}

impl StorageBus for EepromI2cBus {
    fn init(&mut self) -> bool {
        let rcc = unsafe { &*pac::RCC::ptr() };
        let gpiob = unsafe { &*pac::GPIOB::ptr() };
        let i2c2 = unsafe { &*pac::I2C2::ptr() };
        let syscfg = unsafe { &*pac::SYSCFG::ptr() };

        rcc.ahbenr.modify(|_, w| w.iopben().enabled());
        rcc.apb2enr.modify(|_, w| w.syscfgen().enabled());
        rcc.apb1enr.modify(|_, w| w.i2c2en().enabled());

        // Hardware Bus Recovery: Clock 9 SCL pulses on PB10 with PB11 open-drain pull-up
        gpiob
            .moder
            .modify(|_, w| w.moder10().output().moder11().input());
        gpiob
            .otyper
            .modify(|_, w| w.ot10().open_drain().ot11().open_drain());
        gpiob
            .pupdr
            .modify(|_, w| w.pupdr10().pull_up().pupdr11().pull_up());
        gpiob
            .ospeedr
            .modify(|_, w| w.ospeedr10().high_speed().ospeedr11().high_speed());

        for _ in 0..9 {
            gpiob.bsrr.write(|w| w.br10().reset());
            for _ in 0..120 {
                cortex_m::asm::nop();
            }
            gpiob.bsrr.write(|w| w.bs10().set());
            for _ in 0..120 {
                cortex_m::asm::nop();
            }
        }

        syscfg.cfgr1.modify(|_, w| w.i2c2_fmp().fmp());

        gpiob.afrh.modify(|_, w| w.afrh10().af1().afrh11().af1());
        gpiob
            .moder
            .modify(|_, w| w.moder10().alternate().moder11().alternate());
        gpiob
            .otyper
            .modify(|_, w| w.ot10().open_drain().ot11().open_drain());
        gpiob
            .ospeedr
            .modify(|_, w| w.ospeedr10().high_speed().ospeedr11().high_speed());
        gpiob
            .pupdr
            .modify(|_, w| w.pupdr10().pull_up().pupdr11().pull_up());

        i2c2.cr1.modify(|_, w| w.pe().disabled());
        rcc.apb1rstr
            .modify(|r, w| unsafe { w.bits(r.bits() | (1 << 22)) });
        rcc.apb1rstr
            .modify(|r, w| unsafe { w.bits(r.bits() & !(1 << 22)) });
        i2c2.cr1.modify(|_, w| w.pe().disabled());

        i2c2.timingr.write(|w| {
            w.presc()
                .bits(0)
                .scldel()
                .bits(3)
                .sdadel()
                .bits(0)
                .sclh()
                .bits(9)
                .scll()
                .bits(25)
        });

        clear_i2c_flags(unsafe { &*pac::I2C2::ptr() });
        i2c2.cr1.modify(|_, w| w.pe().enabled());

        unsafe {
            EEPROM_STATUS.bus = 0;
            EEPROM_STATUS.detected = false;

            for addr in 0x50..=0x57 {
                if probe_device(&*pac::I2C2::ptr(), 2, addr) {
                    EEPROM_STATUS.bus = 2;
                    EEPROM_STATUS.addr = addr;
                    EEPROM_STATUS.detected = true;
                    break;
                }
            }

            if !EEPROM_STATUS.detected && rcc.apb1enr.read().i2c1en().bit_is_set() {
                for addr in 0x50..=0x57 {
                    if probe_device(&*pac::I2C1::ptr(), 1, addr) {
                        EEPROM_STATUS.bus = 1;
                        EEPROM_STATUS.addr = addr;
                        EEPROM_STATUS.detected = true;
                        break;
                    }
                }
            }

            EEPROM_STATUS.detected
        }
    }

    fn read(&mut self, addr: u16, data: &mut [u8]) -> Result<(), ()> {
        if data.is_empty() {
            return Ok(());
        }

        let status = unsafe { EEPROM_STATUS };
        let bus_num = if status.bus == 1 { 1 } else { 2 };
        let dev_addr = if status.detected { status.addr } else { 0x50 };
        let i2c = unsafe {
            if bus_num == 1 {
                &*pac::I2C1::ptr()
            } else {
                &*pac::I2C2::ptr()
            }
        };

        let mut read_offset = 0;
        while read_offset < data.len() {
            let chunk_len = (data.len() - read_offset).min(64);
            let Some(chunk_addr) = addr.checked_add(read_offset as u16) else {
                return Err(());
            };
            let addr_bytes = chunk_addr.to_be_bytes();

            i2c.cr2.write(|w| {
                w.sadd()
                    .bits((dev_addr as u16) << 1)
                    .nbytes()
                    .bits(2)
                    .rd_wrn()
                    .write()
                    .autoend()
                    .software()
                    .start()
                    .start()
            });

            for &b in &addr_bytes {
                let mut timeout = 10_000u32;
                while !i2c.isr.read().txis().bit() && timeout > 0 {
                    let isr = i2c.isr.read();
                    if isr.nackf().bit() || isr.berr().bit() || isr.arlo().bit() || isr.ovr().bit()
                    {
                        clear_i2c_flags(i2c);
                        recover_i2c(bus_num);
                        return Err(());
                    }
                    timeout -= 1;
                }
                if timeout == 0 {
                    recover_i2c(bus_num);
                    return Err(());
                }
                i2c.txdr.write(|w| w.txdata().bits(b));
            }

            let mut tc_timeout = 10_000u32;
            while !i2c.isr.read().tc().bit() && tc_timeout > 0 {
                let isr = i2c.isr.read();
                if isr.nackf().bit() || isr.berr().bit() || isr.arlo().bit() || isr.ovr().bit() {
                    clear_i2c_flags(i2c);
                    recover_i2c(bus_num);
                    return Err(());
                }
                tc_timeout -= 1;
            }
            if tc_timeout == 0 {
                recover_i2c(bus_num);
                return Err(());
            }

            i2c.cr2.write(|w| {
                w.sadd()
                    .bits((dev_addr as u16) << 1)
                    .nbytes()
                    .bits(chunk_len as u8)
                    .rd_wrn()
                    .read()
                    .autoend()
                    .automatic()
                    .start()
                    .start()
            });

            let dst = &mut data[read_offset..read_offset + chunk_len];
            for b in dst.iter_mut() {
                let mut rx_timeout = 10_000u32;
                while !i2c.isr.read().rxne().bit() && rx_timeout > 0 {
                    let isr = i2c.isr.read();
                    if isr.nackf().bit() || isr.berr().bit() || isr.arlo().bit() || isr.ovr().bit()
                    {
                        clear_i2c_flags(i2c);
                        recover_i2c(bus_num);
                        return Err(());
                    }
                    rx_timeout -= 1;
                }
                if rx_timeout == 0 {
                    recover_i2c(bus_num);
                    return Err(());
                }
                *b = i2c.rxdr.read().rxdata().bits();
            }

            let mut stop_timeout = 10_000u32;
            while !i2c.isr.read().stopf().bit() && stop_timeout > 0 {
                let isr = i2c.isr.read();
                if isr.berr().bit() || isr.arlo().bit() || isr.ovr().bit() {
                    clear_i2c_flags(i2c);
                    recover_i2c(bus_num);
                    return Err(());
                }
                stop_timeout -= 1;
            }
            if stop_timeout == 0 {
                recover_i2c(bus_num);
                return Err(());
            }
            i2c.icr.write(|w| w.stopcf().clear());

            read_offset += chunk_len;
        }

        Ok(())
    }

    fn write(&mut self, mut addr: u16, mut data: &[u8]) -> Result<(), ()> {
        while !data.is_empty() {
            let page_offset = (addr as usize) % PAGE_SIZE;
            let space_in_page = PAGE_SIZE - page_offset;
            let chunk_len = data.len().min(space_in_page).min(PAGE_SIZE);

            let chunk = &data[..chunk_len];

            let status = unsafe { EEPROM_STATUS };
            let bus_num = if status.bus == 1 { 1 } else { 2 };
            let dev_addr = if status.detected { status.addr } else { 0x50 };
            let i2c = unsafe {
                if bus_num == 1 {
                    &*pac::I2C1::ptr()
                } else {
                    &*pac::I2C2::ptr()
                }
            };

            let total_bytes = (2 + chunk.len()) as u8;
            let addr_bytes = addr.to_be_bytes();

            i2c.cr2.write(|w| {
                w.sadd()
                    .bits((dev_addr as u16) << 1)
                    .nbytes()
                    .bits(total_bytes)
                    .rd_wrn()
                    .write()
                    .autoend()
                    .automatic()
                    .start()
                    .start()
            });

            for &b in &addr_bytes {
                let mut timeout = 10_000u32;
                while !i2c.isr.read().txis().bit() && timeout > 0 {
                    let isr = i2c.isr.read();
                    if isr.nackf().bit() || isr.berr().bit() || isr.arlo().bit() || isr.ovr().bit()
                    {
                        clear_i2c_flags(i2c);
                        recover_i2c(bus_num);
                        return Err(());
                    }
                    timeout -= 1;
                }
                if timeout == 0 {
                    recover_i2c(bus_num);
                    return Err(());
                }
                i2c.txdr.write(|w| w.txdata().bits(b));
            }

            for &b in chunk {
                let mut timeout = 10_000u32;
                while !i2c.isr.read().txis().bit() && timeout > 0 {
                    let isr = i2c.isr.read();
                    if isr.nackf().bit() || isr.berr().bit() || isr.arlo().bit() || isr.ovr().bit()
                    {
                        clear_i2c_flags(i2c);
                        recover_i2c(bus_num);
                        return Err(());
                    }
                    timeout -= 1;
                }
                if timeout == 0 {
                    recover_i2c(bus_num);
                    return Err(());
                }
                i2c.txdr.write(|w| w.txdata().bits(b));
            }

            let mut stop_timeout = 10_000u32;
            while !i2c.isr.read().stopf().bit() && stop_timeout > 0 {
                let isr = i2c.isr.read();
                if isr.nackf().bit() || isr.berr().bit() || isr.arlo().bit() || isr.ovr().bit() {
                    clear_i2c_flags(i2c);
                    recover_i2c(bus_num);
                    return Err(());
                }
                stop_timeout -= 1;
            }
            if stop_timeout == 0 {
                recover_i2c(bus_num);
                return Err(());
            }
            i2c.icr.write(|w| w.stopcf().clear());

            // Acknowledge polling: wait for internal write cycle (max 5ms)
            let mut poll_ack = false;
            let mut poll_attempts = 150u32;
            while poll_attempts > 0 {
                cortex_m::asm::delay(2_400); // ~50 us
                clear_i2c_flags(i2c);
                i2c.cr2.write(|w| {
                    w.sadd()
                        .bits((dev_addr as u16) << 1)
                        .nbytes()
                        .bits(0)
                        .rd_wrn()
                        .write()
                        .autoend()
                        .automatic()
                        .start()
                        .start()
                });

                let mut wait = 2_000u32;
                while wait > 0 {
                    let isr = i2c.isr.read();
                    if isr.nackf().bit_is_set() {
                        clear_i2c_flags(i2c);
                        break;
                    }
                    if isr.stopf().bit_is_set() {
                        clear_i2c_flags(i2c);
                        poll_ack = true;
                        break;
                    }
                    wait -= 1;
                }
                if poll_ack {
                    break;
                }
                poll_attempts -= 1;
            }

            if !poll_ack {
                recover_i2c(bus_num);
                return Err(());
            }

            let Some(next_addr) = addr.checked_add(chunk_len as u16) else {
                if data.len() > chunk_len {
                    return Err(());
                }
                break;
            };
            addr = next_addr;
            data = &data[chunk_len..];
        }

        Ok(())
    }
}
