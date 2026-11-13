//! Hardware FT6236 capacitive touchscreen driver for STM32F072 (FlySky FS-i6S).

use ferrox_core::hal::TouchDriver;
use stm32f0xx_hal::pac;

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

impl TouchDriver for Stm32f0TouchDriver {
    fn init(&mut self) {
        unsafe {
            let rcc = &*pac::RCC::ptr();
            let gpioa = &*pac::GPIOA::ptr();
            let gpiob = &*pac::GPIOB::ptr();
            let gpioc = &*pac::GPIOC::ptr();
            let i2c1 = &*pac::I2C1::ptr();

            // Enable GPIOA, GPIOB, GPIOC and I2C1 clocks
            rcc.ahbenr
                .modify(|r, w| w.bits(r.bits() | (1 << 17) | (1 << 18) | (1 << 19)));
            rcc.apb1enr.modify(|r, w| w.bits(r.bits() | (1 << 21)));

            // Select HSI (8 MHz) as I2C1 clock source (I2C1SW = 00 in CFGR3)
            rcc.cfgr3.modify(|r, w| w.bits(r.bits() & !(3 << 4)));

            // PA15 as Output (TOUCH_RST)
            gpioa.moder.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !(3 << 30)) | (1 << 30))
            });

            // PC12 as Input with pull-up (TOUCH_INT)
            gpioc.moder.modify(|r, w| w.bits(r.bits() & !(3 << 24)));
            gpioc.pupdr.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !(3 << 24)) | (1 << 24))
            });

            // Pulse TOUCH_RST LOW for 20 ms at 48 MHz
            gpioa.bsrr.write(|w| w.bits(1 << (15 + 16)));
            cortex_m::asm::delay(960_000);
            gpioa.bsrr.write(|w| w.bits(1 << 15));
            cortex_m::asm::delay(960_000);

            // PB8 (SCL), PB9 (SDA) -> AF1, Open-Drain, High-Speed
            gpiob.moder.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !(0x0F << 16)) | (0x0A << 16))
            });
            gpiob.otyper.modify(|r, w| {
                let val = r.bits();
                w.bits(val | (3 << 8))
            });
            gpiob.ospeedr.modify(|r, w| {
                let val = r.bits();
                w.bits(val | (0x0F << 16))
            });
            gpiob.pupdr.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !(0x0F << 16)) | (0x05 << 16))
            });
            gpiob.afrh.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !0xFF) | 0x11)
            });

            // Configure I2C1 Fast Mode @ 400 kHz with HSI 8 MHz: PRESC=0, SCLDEL=2, SDADEL=1, SCLH=3, SCLL=6
            i2c1.cr1.write(|w| w.bits(0));
            i2c1.timingr.write(|w| {
                w.bits(
                    (2 << 20) // SCLDEL
                    | (1 << 16) // SDADEL
                    | (3 << 8)  // SCLH
                    | 6, // SCLL
                )
            });

            // Enable I2C1
            i2c1.cr1.write(|w| w.pe().set_bit());
        }
    }

    fn is_touch_int_asserted(&mut self) -> bool {
        unsafe {
            let gpioc = &*pac::GPIOC::ptr();
            (gpioc.idr.read().bits() & (1 << 12)) == 0
        }
    }

    fn read_touch(&mut self) -> Option<(u16, u16)> {
        unsafe {
            let i2c1 = &*pac::I2C1::ptr();

            // Set up write of register address 0x00
            i2c1.cr2.write(|w| {
                w.sadd()
                    .bits((0x38 << 1) as u16)
                    .nbytes()
                    .bits(1)
                    .autoend()
                    .clear_bit()
                    .rd_wrn()
                    .clear_bit()
                    .start()
                    .set_bit()
            });

            let reset_i2c1 = || {
                i2c1.cr1.modify(|_, w| w.pe().clear_bit());
                // RM0091 §26.4.3: PE must remain 0 for at least 3 APB clock cycles
                cortex_m::asm::nop();
                cortex_m::asm::nop();
                cortex_m::asm::nop();
                cortex_m::asm::nop();
                i2c1.icr.write(|w| w.bits(0x3F38));
                i2c1.cr1.modify(|_, w| w.pe().set_bit());
            };

            let mut timeout = 10_000u32;
            loop {
                let isr = i2c1.isr.read();
                if isr.nackf().bit_is_set() || isr.berr().bit_is_set() || isr.arlo().bit_is_set() {
                    reset_i2c1();
                    return None;
                }
                if isr.txis().bit_is_set() {
                    break;
                }
                timeout -= 1;
                if timeout == 0 {
                    reset_i2c1();
                    return None;
                }
            }

            i2c1.txdr.write(|w| w.txdata().bits(0x00));

            timeout = 10_000;
            loop {
                let isr = i2c1.isr.read();
                if isr.nackf().bit_is_set() || isr.berr().bit_is_set() || isr.arlo().bit_is_set() {
                    reset_i2c1();
                    return None;
                }
                if isr.tc().bit_is_set() {
                    break;
                }
                timeout -= 1;
                if timeout == 0 {
                    reset_i2c1();
                    return None;
                }
            }

            // Read 7 bytes
            i2c1.cr2.write(|w| {
                w.sadd()
                    .bits((0x38 << 1) as u16)
                    .nbytes()
                    .bits(7)
                    .autoend()
                    .set_bit()
                    .rd_wrn()
                    .set_bit()
                    .start()
                    .set_bit()
            });

            let mut buf = [0u8; 7];
            for byte in buf.iter_mut() {
                timeout = 10_000;
                loop {
                    let isr = i2c1.isr.read();
                    if isr.nackf().bit_is_set()
                        || isr.berr().bit_is_set()
                        || isr.arlo().bit_is_set()
                    {
                        reset_i2c1();
                        return None;
                    }
                    if isr.rxne().bit_is_set() {
                        break;
                    }
                    timeout -= 1;
                    if timeout == 0 {
                        reset_i2c1();
                        return None;
                    }
                }
                *byte = i2c1.rxdr.read().rxdata().bits();
            }

            let num_touches = buf[2] & 0x0F;
            if num_touches == 0 {
                return None;
            }

            let raw_x = (((buf[3] & 0x0F) as u16) << 8) | (buf[4] as u16);
            let raw_y = (((buf[5] & 0x0F) as u16) << 8) | (buf[6] as u16);

            Some((raw_x, raw_y))
        }
    }
}
