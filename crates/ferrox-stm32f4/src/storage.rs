use core::marker::PhantomData;
use ferrox_core::hal::StorageBus;
use stm32f4::stm32f407 as pac;

use crate::descriptor::{PinDef, PocketHal, Qx7HalDescriptor};
use crate::sdcard::{
    SpiSdCard, FERROX_STORAGE_SECTOR_BASE, FERROX_TELEM_SECTOR_BASE, SD_SECTOR_SIZE,
};

const EEPROM_ADDR: u8 = 0x51; // 7-bit I2C address for M24256 on Q X7 family
const PAGE_SIZE: usize = 64;

pub struct PocketEepromI2cBus<H: Qx7HalDescriptor = PocketHal> {
    using_sd: bool,
    sd_card: SpiSdCard<H>,
    active_telem_sector: u32,
    _phantom: PhantomData<H>,
}

impl<H: Qx7HalDescriptor> PocketEepromI2cBus<H> {
    pub const fn new() -> Self {
        Self {
            using_sd: false,
            sd_card: SpiSdCard::new(),
            active_telem_sector: FERROX_TELEM_SECTOR_BASE,
            _phantom: PhantomData,
        }
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
            if timeout == 0 {
                return Err(());
            }
        }
        i2c1.dr.write(|w| unsafe { w.bits(byte as u32) });

        timeout = 100_000;
        while (i2c1.sr1.read().bits() & (1 << 2)) == 0 {
            // BTF bit
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
        Ok(())
    }

    fn send_stop(&self) {
        let i2c1 = unsafe { &*pac::I2C1::ptr() };
        i2c1.cr1.modify(|r, w| unsafe { w.bits(r.bits() | (1 << 9)) }); // STOP
    }

    fn poll_ack(&self) -> bool {
        for _ in 0..100 {
            if self.send_start().is_err() {
                continue;
            }
            if self.send_addr(EEPROM_ADDR, false).is_ok() {
                self.send_stop();
                return true;
            }
            self.send_stop();
            cortex_m::asm::delay(168_000); // 1ms delay between retries
        }
        false
    }

    fn init_i2c(&mut self) -> bool {
        let rcc = unsafe { &*pac::RCC::ptr() };
        let i2c1 = unsafe { &*pac::I2C1::ptr() };

        unsafe {
            // Enable I2C1 (bit 21) in APB1ENR
            rcc.apb1enr.modify(|r, w| w.bits(r.bits() | (1 << 21)));

            // Configure Write Protect pin: Push-Pull Output, driven LOW (write enabled)
            H::EEPROM_WP.enable_clock_and_output(false);

            // Configure SCL and SDA pins: AF4 Open-Drain with pull-up
            let configure_i2c_pin = |pin_def: PinDef| {
                rcc.ahb1enr
                    .modify(|r, w| w.bits(r.bits() | pin_def.rcc_ahb1_bit()));
                let gpio = &*pin_def.gpio_ptr();
                let pin = pin_def.pin;
                let shift = pin * 2;
                gpio.moder
                    .modify(|r, w| w.bits((r.bits() & !(0x03 << shift)) | (0x02 << shift)));
                gpio.otyper.modify(|r, w| w.bits(r.bits() | (1 << pin)));
                gpio.pupdr
                    .modify(|r, w| w.bits((r.bits() & !(0x03 << shift)) | (0x01 << shift)));
                if pin < 8 {
                    let af_shift = pin * 4;
                    gpio.afrl
                        .modify(|r, w| w.bits((r.bits() & !(0x0F << af_shift)) | (0x04 << af_shift)));
                } else {
                    let af_shift = (pin - 8) * 4;
                    gpio.afrh
                        .modify(|r, w| w.bits((r.bits() & !(0x0F << af_shift)) | (0x04 << af_shift)));
                }
            };

            configure_i2c_pin(H::EEPROM_SCL);
            configure_i2c_pin(H::EEPROM_SDA);

            // Software reset I2C1
            i2c1.cr1.modify(|r, w| w.bits(r.bits() | (1 << 15)));
            cortex_m::asm::delay(1680);
            i2c1.cr1.modify(|r, w| w.bits(r.bits() & !(1 << 15)));

            // Configure I2C1: APB1 = 42 MHz
            i2c1.cr2.write(|w| w.bits(42)); // 42 MHz
            // Fast Mode (400 kHz), Duty = 0 (2:1): CCR = 42MHz / (3 * 400kHz) = 35
            i2c1.ccr.write(|w| w.bits(0x8000 | 35));
            // TRISE = (300ns * 42MHz) + 1 = 14
            i2c1.trise.write(|w| w.bits(14));
            // Enable peripheral
            i2c1.cr1.modify(|r, w| w.bits(r.bits() | 0x01));
        }

        self.poll_ack()
    }

    fn read_i2c(&mut self, addr: u16, buf: &mut [u8]) -> Result<(), ()> {
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
                timeout -= 1;
                if timeout == 0 {
                    return Err(());
                }
            }
            buf[0] = i2c1.dr.read().bits() as u8;
            i2c1.cr1.modify(|r, w| unsafe { w.bits(r.bits() | (1 << 10)) });
            return Ok(());
        }

        // Multiple bytes read
        i2c1.cr1.modify(|r, w| unsafe { w.bits(r.bits() | (1 << 10)) }); // ACK enabled
        for (i, byte) in buf.iter_mut().enumerate() {
            if i == len - 1 {
                // Disable ACK and send STOP on last byte
                i2c1.cr1.modify(|r, w| unsafe { w.bits(r.bits() & !(1 << 10)) });
                self.send_stop();
            }

            let mut timeout = 100_000u32;
            while (i2c1.sr1.read().bits() & (1 << 6)) == 0 {
                // RXNE bit
                timeout -= 1;
                if timeout == 0 {
                    return Err(());
                }
            }
            *byte = i2c1.dr.read().bits() as u8;
        }

        i2c1.cr1.modify(|r, w| unsafe { w.bits(r.bits() | (1 << 10)) }); // Restore ACK
        Ok(())
    }

    fn write_i2c(&mut self, addr: u16, buf: &[u8]) -> Result<(), ()> {
        let mut cur_addr = addr;
        let mut offset = 0;
        let total = buf.len();

        while offset < total {
            let page_offset = (cur_addr as usize) % PAGE_SIZE;
            let bytes_in_page = PAGE_SIZE - page_offset;
            let chunk_size = bytes_in_page.min(total - offset);

            self.wait_while_busy();
            self.send_start()?;
            self.send_addr(EEPROM_ADDR, false)?;

            // Send word address
            self.write_byte((cur_addr >> 8) as u8)?;
            self.write_byte((cur_addr & 0xFF) as u8)?;

            // Stream page chunk
            for &byte in &buf[offset..offset + chunk_size] {
                self.write_byte(byte)?;
            }
            self.send_stop();

            // Wait for internal write cycle to complete (~5ms)
            if !self.poll_ack() {
                return Err(());
            }

            cur_addr += chunk_size as u16;
            offset += chunk_size;
        }

        Ok(())
    }

    fn read_sd(&mut self, addr: u16, buf: &mut [u8]) -> Result<(), ()> {
        let mut cur_addr = addr as usize;
        let mut offset = 0;
        let total = buf.len();
        let mut sector_buf = [0u8; SD_SECTOR_SIZE];

        while offset < total {
            let sector_idx = FERROX_STORAGE_SECTOR_BASE + (cur_addr / SD_SECTOR_SIZE) as u32;
            let sector_offset = cur_addr % SD_SECTOR_SIZE;
            let bytes_in_sector = SD_SECTOR_SIZE - sector_offset;
            let chunk_size = bytes_in_sector.min(total - offset);

            self.sd_card.read_sector(sector_idx, &mut sector_buf)?;
            buf[offset..offset + chunk_size]
                .copy_from_slice(&sector_buf[sector_offset..sector_offset + chunk_size]);

            cur_addr += chunk_size;
            offset += chunk_size;
        }
        Ok(())
    }

    fn write_sd(&mut self, addr: u16, buf: &[u8]) -> Result<(), ()> {
        let mut cur_addr = addr as usize;
        let mut offset = 0;
        let total = buf.len();
        let mut sector_buf = [0u8; SD_SECTOR_SIZE];

        while offset < total {
            let sector_idx = FERROX_STORAGE_SECTOR_BASE + (cur_addr / SD_SECTOR_SIZE) as u32;
            let sector_offset = cur_addr % SD_SECTOR_SIZE;
            let bytes_in_sector = SD_SECTOR_SIZE - sector_offset;
            let chunk_size = bytes_in_sector.min(total - offset);

            // If updating partial sector, read previous sector contents first
            if chunk_size < SD_SECTOR_SIZE {
                let _ = self.sd_card.read_sector(sector_idx, &mut sector_buf);
            }

            sector_buf[sector_offset..sector_offset + chunk_size]
                .copy_from_slice(&buf[offset..offset + chunk_size]);
            self.sd_card.write_sector(sector_idx, &sector_buf)?;

            cur_addr += chunk_size;
            offset += chunk_size;
        }
        Ok(())
    }
}

impl<H: Qx7HalDescriptor> Default for PocketEepromI2cBus<H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H: Qx7HalDescriptor> StorageBus for PocketEepromI2cBus<H> {
    fn init(&mut self) -> bool {
        // 1. Probe I2C1 EEPROM first (FrSky QX7 / X9D+ with onboard M24256)
        if self.init_i2c() {
            self.using_sd = false;
            return true;
        }

        // 2. Fall back to SPI2 SD card (RadioMaster Pocket, Boxer, Zorro, TX12 MKII)
        if self.sd_card.init() {
            self.using_sd = true;
            self.active_telem_sector = FERROX_TELEM_SECTOR_BASE;
            return true;
        }

        // 3. No hardware storage found (bench testing with SD card removed)
        self.using_sd = false;
        false
    }

    fn read(&mut self, addr: u16, buf: &mut [u8]) -> Result<(), ()> {
        if self.using_sd {
            self.read_sd(addr, buf)
        } else {
            self.read_i2c(addr, buf)
        }
    }

    fn write(&mut self, addr: u16, buf: &[u8]) -> Result<(), ()> {
        if self.using_sd {
            self.write_sd(addr, buf)
        } else {
            self.write_i2c(addr, buf)
        }
    }

    fn append_telemetry_sector(&mut self, sector: &[u8; 512]) -> Result<(), ()> {
        if self.using_sd {
            self.sd_card.write_sector(self.active_telem_sector, sector)?;
            self.active_telem_sector = self.active_telem_sector.wrapping_add(1);
        }
        Ok(())
    }
}
