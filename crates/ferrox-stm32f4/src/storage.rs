use ferrox_core::hal::StorageBus;
use stm32f4::stm32f407 as pac;

const EEPROM_ADDR: u8 = 0x51; // 7-bit I2C address for M24256 on Pocket
const PAGE_SIZE: usize = 64;

pub struct PocketEepromI2cBus;

impl PocketEepromI2cBus {
    pub const fn new() -> Self {
        Self
    }

    fn wait_while_busy(&self) {
        let i2c1 = unsafe { &*pac::I2C1::ptr() };
        let mut timeout = 100_000u32;
        while (i2c1.sr2.read().bits() & (1 << 1)) != 0 && timeout > 0 {
            timeout -= 1;
        }
    }

    fn send_start(&self) -> Result<(), ()> {
        let i2c1 = unsafe { &*pac::I2C1::ptr() };
        i2c1.cr1.modify(|r, w| unsafe { w.bits(r.bits() | (1 << 8)) }); // START
        let mut timeout = 100_000u32;
        while (i2c1.sr1.read().bits() & (1 << 0)) == 0 {
            // SB bit
            timeout -= 1;
            if timeout == 0 {
                return Err(());
            }
        }
        Ok(())
    }

    fn send_addr(&self, addr_7bit: u8, is_read: bool) -> Result<(), ()> {
        let i2c1 = unsafe { &*pac::I2C1::ptr() };
        let val = (addr_7bit << 1) | (is_read as u8);
        i2c1.dr.write(|w| unsafe { w.bits(val as u32) });

        let mut timeout = 100_000u32;
        while (i2c1.sr1.read().bits() & (1 << 1)) == 0 {
            // ADDR bit
            timeout -= 1;
            if (i2c1.sr1.read().bits() & (1 << 10)) != 0 {
                // AF bit (NACK)
                i2c1.sr1.modify(|r, w| unsafe { w.bits(r.bits() & !(1 << 10)) });
                return Err(());
            }
            if timeout == 0 {
                return Err(());
            }
        }
        // Clear ADDR flag by reading SR1 and SR2
        let _ = i2c1.sr1.read();
        let _ = i2c1.sr2.read();
        Ok(())
    }

    fn write_byte(&self, byte: u8) -> Result<(), ()> {
        let i2c1 = unsafe { &*pac::I2C1::ptr() };
        let mut timeout = 100_000u32;
        while (i2c1.sr1.read().bits() & (1 << 7)) == 0 {
            // TXE bit
            timeout -= 1;
            if (i2c1.sr1.read().bits() & (1 << 10)) != 0 {
                // AF bit (NACK)
                i2c1.sr1.modify(|r, w| unsafe { w.bits(r.bits() & !(1 << 10)) });
                return Err(());
            }
            if timeout == 0 {
                return Err(());
            }
        }
        i2c1.dr.write(|w| unsafe { w.bits(byte as u32) });
        Ok(())
    }

    fn send_stop(&self) {
        let i2c1 = unsafe { &*pac::I2C1::ptr() };
        i2c1.cr1.modify(|r, w| unsafe { w.bits(r.bits() | (1 << 9)) }); // STOP
    }

    fn poll_ack(&self) -> bool {
        let mut attempts = 1000;
        while attempts > 0 {
            attempts -= 1;
            if self.send_start().is_ok() && self.send_addr(EEPROM_ADDR, false).is_ok() {
                self.send_stop();
                return true;
            }
            self.send_stop();
            cortex_m::asm::delay(1680); // ~10µs
        }
        false
    }
}

impl Default for PocketEepromI2cBus {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageBus for PocketEepromI2cBus {
    fn init(&mut self) -> bool {
        let rcc = unsafe { &*pac::RCC::ptr() };
        let gpiob = unsafe { &*pac::GPIOB::ptr() };
        let gpiod = unsafe { &*pac::GPIOD::ptr() };
        let i2c1 = unsafe { &*pac::I2C1::ptr() };

        unsafe {
            // Enable GPIOB, GPIOD (AHB1) and I2C1 (APB1)
            rcc.ahb1enr
                .modify(|r, w| w.bits(r.bits() | (1 << 1) | (1 << 3)));
            rcc.apb1enr.modify(|r, w| w.bits(r.bits() | (1 << 21)));

            // PD10 (EEPROM_WP): GPIO Output, driven LOW to enable writes
            gpiod
                .moder
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 20)) | (0x01 << 20)));
            gpiod.bsrr.write(|w| w.bits(1 << (10 + 16))); // PD10 LOW

            // PB8 (SCL), PB9 (SDA): AF4 Open-Drain with pull-up
            gpiob.moder.modify(|r, w| {
                w.bits((r.bits() & !((0x03 << 16) | (0x03 << 18))) | (0x02 << 16) | (0x02 << 18))
            });
            gpiob
                .otyper
                .modify(|r, w| w.bits(r.bits() | (1 << 8) | (1 << 9)));
            gpiob.pupdr.modify(|r, w| {
                w.bits((r.bits() & !((0x03 << 16) | (0x03 << 18))) | (0x01 << 16) | (0x01 << 18))
            });
            gpiob.afrh.modify(|r, w| {
                let mask = (0x0F << 0) | (0x0F << 4);
                let af4 = (0x04 << 0) | (0x04 << 4);
                w.bits((r.bits() & !mask) | af4)
            });

            // Software reset I2C1
            i2c1.cr1.modify(|r, w| w.bits(r.bits() | (1 << 15)));
            cortex_m::asm::delay(1680);
            i2c1.cr1.modify(|r, w| w.bits(r.bits() & !(1 << 15)));

            // Configure I2C1: APB1 = 42 MHz
            i2c1.cr2.write(|w| w.bits(42)); // 42 MHz
            // Fast Mode (400 kHz), Duty = 0 (2:1): CCR = 42MHz / (3 * 400kHz) = 35 = 0x23
            i2c1.ccr.write(|w| w.bits(0x8000 | 35));
            // TRISE = (300ns * 42MHz) + 1 = 13.6 -> 14
            i2c1.trise.write(|w| w.bits(14));
            // Enable peripheral
            i2c1.cr1.modify(|r, w| w.bits(r.bits() | 0x01));
        }

        self.poll_ack()
    }

    fn read(&mut self, addr: u16, buf: &mut [u8]) -> Result<(), ()> {
        let i2c1 = unsafe { &*pac::I2C1::ptr() };

        self.wait_while_busy();
        self.send_start()?;
        self.send_addr(EEPROM_ADDR, false)?;

        // Send 16-bit word address
        self.write_byte((addr >> 8) as u8)?;
        self.write_byte((addr & 0xFF) as u8)?;

        // Restart in Read mode
        self.send_start()?;
        self.send_addr(EEPROM_ADDR, true)?;

        let len = buf.len();
        if len == 1 {
            // Disable ACK before single byte read
            i2c1.cr1.modify(|r, w| unsafe { w.bits(r.bits() & !(1 << 10)) });
            self.send_stop();

            let mut timeout = 100_000u32;
            while (i2c1.sr1.read().bits() & (1 << 6)) == 0 {
                // RXNE
                timeout -= 1;
                if timeout == 0 {
                    return Err(());
                }
            }
            buf[0] = i2c1.dr.read().bits() as u8;
        } else {
            // Enable ACK
            i2c1.cr1.modify(|r, w| unsafe { w.bits(r.bits() | (1 << 10)) });

            for i in 0..len {
                if i == len - 1 {
                    // Disable ACK and send STOP on last byte
                    i2c1.cr1.modify(|r, w| unsafe { w.bits(r.bits() & !(1 << 10)) });
                    self.send_stop();
                }

                let mut timeout = 100_000u32;
                while (i2c1.sr1.read().bits() & (1 << 6)) == 0 {
                    timeout -= 1;
                    if timeout == 0 {
                        return Err(());
                    }
                }
                buf[i] = i2c1.dr.read().bits() as u8;
            }
        }

        Ok(())
    }

    fn write(&mut self, mut addr: u16, mut buf: &[u8]) -> Result<(), ()> {
        while !buf.is_empty() {
            let offset_in_page = (addr as usize) % PAGE_SIZE;
            let bytes_in_page = (PAGE_SIZE - offset_in_page).min(buf.len());

            self.wait_while_busy();
            self.send_start()?;
            self.send_addr(EEPROM_ADDR, false)?;

            // 16-bit address
            self.write_byte((addr >> 8) as u8)?;
            self.write_byte((addr & 0xFF) as u8)?;

            for &byte in &buf[..bytes_in_page] {
                self.write_byte(byte)?;
            }

            self.send_stop();

            // Wait for internal write cycle to complete (up to ~5 ms)
            if !self.poll_ack() {
                return Err(());
            }

            addr += bytes_in_page as u16;
            buf = &buf[bytes_in_page..];
        }

        Ok(())
    }
}
