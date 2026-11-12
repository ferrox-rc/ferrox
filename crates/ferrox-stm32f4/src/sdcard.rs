//! Zero-heap SPI2 SDHC microSD card block driver for STM32F4 Q X7 family boards.

use core::marker::PhantomData;
use stm32f4::stm32f407 as pac;

use crate::descriptor::{PinDef, PocketHal, Qx7HalDescriptor};

pub const SD_SECTOR_SIZE: usize = 512;

/// Dedicated pre-partition MBR gap offset for Ferrox virtual EEPROM (16 KB = sectors 256..287).
/// Verified at runtime via `SdRawGap::from_mbr` to strictly precede Partition 1 (`LBA >= 288`).
pub const FERROX_STORAGE_SECTOR_BASE: u32 = ferrox_core::storage::SdRawGap::PREFERRED_EEPROM_BASE;

/// Dedicated pre-partition MBR gap offset for circular raw telemetry logging (sectors 512..min_first_lba).
pub const FERROX_TELEM_SECTOR_BASE: u32 = ferrox_core::storage::SdRawGap::PREFERRED_TELEM_BASE;

pub struct SpiSdCard<H: Qx7HalDescriptor = PocketHal> {
    is_sdhc: bool,
    is_initialized: bool,
    _phantom: PhantomData<H>,
}

impl<H: Qx7HalDescriptor> SpiSdCard<H> {
    pub const fn new() -> Self {
        Self {
            is_sdhc: false,
            is_initialized: false,
            _phantom: PhantomData,
        }
    }

    #[inline(always)]
    fn cs_high(&self) {
        H::SD_CS.set_high();
    }

    #[inline(always)]
    fn cs_low(&self) {
        H::SD_CS.set_low();
    }

    fn spi_transfer_byte(&self, byte: u8) -> u8 {
        let spi2 = unsafe { &*pac::SPI2::ptr() };
        let mut timeout = 100_000u32;
        while (spi2.sr.read().bits() & (1 << 1)) == 0 && timeout > 0 {
            timeout -= 1;
        }
        if timeout == 0 {
            return 0xFF;
        }
        spi2.dr.write(|w| unsafe { w.bits(byte as u32) });
        timeout = 100_000;
        while (spi2.sr.read().bits() & (1 << 0)) == 0 && timeout > 0 {
            timeout -= 1;
        }
        if timeout == 0 {
            return 0xFF;
        }
        spi2.dr.read().bits() as u8
    }

    fn send_cmd(&self, cmd: u8, arg: u32, crc: u8) -> u8 {
        self.cs_low();
        self.spi_transfer_byte(0xFF); // Sync byte

        // Command packet: 6 bytes
        self.spi_transfer_byte(0x40 | cmd);
        self.spi_transfer_byte((arg >> 24) as u8);
        self.spi_transfer_byte((arg >> 16) as u8);
        self.spi_transfer_byte((arg >> 8) as u8);
        self.spi_transfer_byte(arg as u8);
        self.spi_transfer_byte(crc);

        // Wait for R1 response (up to 10 bytes)
        for _ in 0..10 {
            let res = self.spi_transfer_byte(0xFF);
            if (res & 0x80) == 0 {
                return res;
            }
        }
        0xFF
    }

    fn wait_ready(&self) -> bool {
        for _ in 0..250_000 {
            if self.spi_transfer_byte(0xFF) == 0xFF {
                return true;
            }
        }
        false
    }

    /// Initialize SPI2 peripheral and SD card in SPI mode.
    pub fn init(&mut self) -> bool {
        let rcc = unsafe { &*pac::RCC::ptr() };
        let spi2 = unsafe { &*pac::SPI2::ptr() };

        // 1. Check card detect pin (low = card inserted)
        H::SD_DETECT.enable_clock_and_input_pullup();
        if !H::SD_DETECT.is_low() {
            return false;
        }

        // 2. Enable SPI2 clock (bit 14 of APB1ENR)
        rcc.apb1enr
            .modify(|r, w| unsafe { w.bits(r.bits() | (1 << 14)) });

        // 3. Configure CS pin as output, drive high
        H::SD_CS.enable_clock_and_output(true);

        // 4. Configure SCK, MISO, MOSI as AF5 (SPI2) with High Speed
        let configure_spi_pin = |pin_def: PinDef| {
            rcc.ahb1enr
                .modify(|r, w| unsafe { w.bits(r.bits() | pin_def.rcc_ahb1_bit()) });
            let _ = rcc.ahb1enr.read().bits();
            let gpio = unsafe { &*pin_def.gpio_ptr() };
            let pin = pin_def.pin;
            let shift = pin * 2;
            gpio.moder
                .modify(|r, w| unsafe { w.bits((r.bits() & !(0x03 << shift)) | (0x02 << shift)) });
            gpio.ospeedr
                .modify(|r, w| unsafe { w.bits((r.bits() & !(0x03 << shift)) | (0x02 << shift)) });
            gpio.pupdr
                .modify(|r, w| unsafe { w.bits((r.bits() & !(0x03 << shift)) | (0x01 << shift)) });
            if pin < 8 {
                let af_shift = pin * 4;
                gpio.afrl.modify(|r, w| unsafe {
                    w.bits((r.bits() & !(0x0F << af_shift)) | (0x05 << af_shift))
                });
            } else {
                let af_shift = (pin - 8) * 4;
                gpio.afrh.modify(|r, w| unsafe {
                    w.bits((r.bits() & !(0x0F << af_shift)) | (0x05 << af_shift))
                });
            }
        };

        configure_spi_pin(H::SD_SCK);
        configure_spi_pin(H::SD_MISO);
        configure_spi_pin(H::SD_MOSI);

        // 5. Initialize SPI2 at slow speed: APB1 / 128 = 42MHz / 128 = ~328 kHz, Master, 8-bit, Mode 0
        // CR1: MSTR (bit 2), BR = 0b110 (bits 5..3 = 6 -> /128), SSM (bit 9), SSI (bit 8), SPE (bit 6)
        spi2.cr1
            .write(|w| unsafe { w.bits((1 << 2) | (0b110 << 3) | (1 << 9) | (1 << 8) | (1 << 6)) });

        // 6. Send >= 80 dummy clocks with CS high to allow SD card internal reset
        self.cs_high();
        for _ in 0..12 {
            self.spi_transfer_byte(0xFF);
        }

        // 7. CMD0: Reset card (GO_IDLE_STATE)
        let r1 = self.send_cmd(0, 0, 0x95);
        self.cs_high();
        self.spi_transfer_byte(0xFF);
        if r1 != 0x01 {
            return false;
        }

        // 8. CMD8: Check voltage range (SEND_IF_COND, 0x1AA)
        let r1 = self.send_cmd(8, 0x000001AA, 0x87);
        let mut is_v2 = false;
        if r1 == 0x01 {
            let mut r7 = [0u8; 4];
            for b in r7.iter_mut() {
                *b = self.spi_transfer_byte(0xFF);
            }
            if r7[2] == 0x01 && r7[3] == 0xAA {
                is_v2 = true;
            }
        }
        self.cs_high();
        self.spi_transfer_byte(0xFF);

        // 9. ACMD41: Initialize card
        let mut initialized = false;
        let arg = if is_v2 { 0x4000_0000 } else { 0 }; // HCS bit
        for _ in 0..1000 {
            // Send CMD55 (APP_CMD)
            let _ = self.send_cmd(55, 0, 0x65);
            self.cs_high();
            self.spi_transfer_byte(0xFF);

            // Send ACMD41 (SD_SEND_OP_COND)
            let res = self.send_cmd(41, arg, 0x77);
            self.cs_high();
            self.spi_transfer_byte(0xFF);

            if res == 0x00 {
                initialized = true;
                break;
            }
            unsafe {
                (&*pac::IWDG::ptr()).kr.write(|w| w.bits(0xAAAA));
            }
            cortex_m::asm::delay(168_000); // 1 ms delay
        }

        if !initialized {
            return false;
        }

        // 10. CMD58: Read OCR to determine SDHC / standard block mode
        if is_v2 {
            let r1 = self.send_cmd(58, 0, 0xFD);
            if r1 == 0x00 {
                let mut ocr = [0u8; 4];
                for b in ocr.iter_mut() {
                    *b = self.spi_transfer_byte(0xFF);
                }
                self.is_sdhc = (ocr[0] & 0x40) != 0; // Bit 30 = CCS
            }
            self.cs_high();
            self.spi_transfer_byte(0xFF);
        }

        // 11. Switch SPI2 to high speed: APB1 / 4 = 42MHz / 4 = 10.5 MHz (BR = 0b001)
        spi2.cr1
            .modify(|r, w| unsafe { w.bits((r.bits() & !(0x07 << 3)) | (0b001 << 3)) });

        self.is_initialized = true;
        true
    }

    /// Read a single 512-byte sector from the SD card.
    pub fn read_sector(
        &mut self,
        sector_num: u32,
        buf: &mut [u8; SD_SECTOR_SIZE],
    ) -> Result<(), ()> {
        if !self.is_initialized {
            return Err(());
        }

        let addr = if self.is_sdhc {
            sector_num
        } else {
            sector_num.checked_mul(512).ok_or(())?
        };

        if self.send_cmd(17, addr, 0xFF) != 0x00 {
            self.cs_high();
            self.spi_transfer_byte(0xFF);
            return Err(());
        }

        // Wait for data start token (0xFE)
        let mut token = 0xFF;
        for _ in 0..100_000 {
            token = self.spi_transfer_byte(0xFF);
            if token != 0xFF {
                break;
            }
        }

        if token != 0xFE {
            self.cs_high();
            self.spi_transfer_byte(0xFF);
            return Err(());
        }

        // Read 512 bytes
        for byte in buf.iter_mut() {
            *byte = self.spi_transfer_byte(0xFF);
        }

        // Read 2-byte dummy CRC
        self.spi_transfer_byte(0xFF);
        self.spi_transfer_byte(0xFF);

        self.cs_high();
        self.spi_transfer_byte(0xFF);
        Ok(())
    }

    /// Write a single 512-byte sector to the SD card.
    pub fn write_sector(&mut self, sector_num: u32, buf: &[u8; SD_SECTOR_SIZE]) -> Result<(), ()> {
        if !self.is_initialized {
            return Err(());
        }

        let addr = if self.is_sdhc {
            sector_num
        } else {
            sector_num.checked_mul(512).ok_or(())?
        };

        if self.send_cmd(24, addr, 0xFF) != 0x00 {
            self.cs_high();
            self.spi_transfer_byte(0xFF);
            return Err(());
        }

        // Send start token (0xFE)
        self.spi_transfer_byte(0xFE);

        // Send 512 bytes
        for &byte in buf.iter() {
            self.spi_transfer_byte(byte);
        }

        // Send 2 dummy CRC bytes
        self.spi_transfer_byte(0xFF);
        self.spi_transfer_byte(0xFF);

        // Receive data response token
        let res = self.spi_transfer_byte(0xFF);
        if (res & 0x1F) != 0x05 {
            // Data rejected
            self.cs_high();
            self.spi_transfer_byte(0xFF);
            return Err(());
        }

        // Wait while card is busy (MISO low)
        let ok = self.wait_ready();
        self.cs_high();
        self.spi_transfer_byte(0xFF);

        if ok {
            Ok(())
        } else {
            Err(())
        }
    }
}
