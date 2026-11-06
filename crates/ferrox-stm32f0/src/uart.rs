//! Hardware USART drivers for CRSF / ExpressLRS on STM32F072.

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use ferrox_core::hal::SerialPort;
use stm32f0xx_hal::pac::{self, interrupt};

const RX_BUF_SIZE: usize = 256;
const TX_BUF_SIZE: usize = 64;

const DMA_CCR_EN: u32 = 1 << 0;
const DMA_CCR_DIR: u32 = 1 << 4;
const DMA_CCR_CIRC: u32 = 1 << 5;
const DMA_CCR_MINC: u32 = 1 << 7;

static mut I6X_RX_DMA_BUF: [u8; RX_BUF_SIZE] = [0; RX_BUF_SIZE];
static mut I6X_TX_DMA_BUF: [u8; TX_BUF_SIZE] = [0; TX_BUF_SIZE];
static I6X_RX_TAIL: AtomicUsize = AtomicUsize::new(0);
static I6X_TX_BUSY: AtomicBool = AtomicBool::new(false);

static mut I6S_RX_DMA_BUF: [u8; RX_BUF_SIZE] = [0; RX_BUF_SIZE];
static mut I6S_TX_DMA_BUF: [u8; TX_BUF_SIZE] = [0; TX_BUF_SIZE];
static I6S_RX_TAIL: AtomicUsize = AtomicUsize::new(0);
static I6S_TX_BUSY: AtomicBool = AtomicBool::new(false);

/// USART2 CRSF driver for FlySky FS-i6X (PD5 TX / PA15 RX, PC13 Power).
pub struct I6xSerialPort {
    active_high: bool,
    power_on: bool,
    half_duplex: bool,
}

impl I6xSerialPort {
    pub const fn new() -> Self {
        Self {
            active_high: true,
            power_on: false,
            half_duplex: false,
        }
    }

    unsafe fn apply_power_pin(&self) {
        let pin_high = if self.active_high {
            self.power_on
        } else {
            !self.power_on
        };

        let gpioc = &*pac::GPIOC::ptr();
        if pin_high {
            gpioc.bsrr.write(|w| w.bs13().set_bit());
        } else {
            gpioc.bsrr.write(|w| w.br13().set_bit());
        }
    }
}

impl Default for I6xSerialPort {
    fn default() -> Self {
        Self::new()
    }
}

impl SerialPort for I6xSerialPort {
    fn init(&mut self, active_high: bool) {
        self.active_high = active_high;
        self.power_on = false;

        unsafe {
            let rcc = &*pac::RCC::ptr();
            let gpioc = &*pac::GPIOC::ptr();

            rcc.ahbenr.modify(|r, w| w.bits(r.bits() | (1 << 19)));

            gpioc.moder.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !(3 << 26)) | (1 << 26))
            });

            self.apply_power_pin();
        }
    }

    fn set_power_polarity(&mut self, active_high: bool) {
        self.active_high = active_high;
        unsafe {
            self.apply_power_pin();
        }
    }

    fn set_module_power(&mut self, enabled: bool) {
        self.power_on = enabled;
        unsafe {
            self.apply_power_pin();
        }
    }

    fn set_uart_enabled(&mut self, enabled: bool, baud_idx: u8, half_duplex: bool) {
        self.half_duplex = half_duplex;
        unsafe {
            let rcc = &*pac::RCC::ptr();
            let usart2 = &*pac::USART2::ptr();
            let dma = &*pac::DMA1::ptr();
            let syscfg = &*pac::SYSCFG::ptr();
            let gpiod = &*pac::GPIOD::ptr();
            let gpioa = &*pac::GPIOA::ptr();

            if !enabled {
                cortex_m::peripheral::NVIC::mask(pac::Interrupt::USART2);
                usart2.cr1.write(|w| w.bits(0));
                rcc.apb1enr.modify(|r, w| w.bits(r.bits() & !(1 << 17)));
                dma.ch6.cr.write(|w| w.bits(0));
                dma.ch7.cr.write(|w| w.bits(0));
                I6X_TX_BUSY.store(false, Ordering::Release);
                return;
            }

            rcc.ahbenr
                .modify(|r, w| w.bits(r.bits() | (1 << 0) | (1 << 17) | (1 << 20)));
            rcc.apb1enr.modify(|r, w| w.bits(r.bits() | (1 << 17)));
            rcc.apb2enr.modify(|r, w| w.bits(r.bits() | (1 << 0)));

            syscfg.cfgr1.modify(|r, w| w.bits(r.bits() | (1 << 10)));

            gpiod.moder.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !(3 << 10)) | (2 << 10))
            });
            gpiod.afrl.modify(|r, w| {
                let val = r.bits();
                w.bits(val & !(0xF << 20))
            });

            if !half_duplex {
                gpioa.moder.modify(|r, w| {
                    let val = r.bits();
                    w.bits((val & !(3 << 30)) | (2 << 30))
                });
                gpioa.afrh.modify(|r, w| {
                    let val = r.bits();
                    w.bits((val & !(0xF << 28)) | (1 << 28))
                });
                gpioa.pupdr.modify(|r, w| {
                    let val = r.bits();
                    w.bits((val & !(3 << 30)) | (1 << 30))
                });
            }

            usart2.cr1.write(|w| w.bits(0));

            let brr = match baud_idx {
                0 => 417,
                1 => 115,
                2 => 114,
                3 => 52,
                4 => 26,
                _ => 115,
            };
            usart2.brr.write(|w| w.bits(brr));

            if half_duplex {
                usart2.cr3.write(|w| w.bits((1 << 3) | (1 << 6) | (1 << 7)));
            } else {
                usart2.cr3.write(|w| w.bits((1 << 6) | (1 << 7)));
            }

            // RX DMA (Channel 6)
            dma.ch6.cr.write(|w| w.bits(0));
            dma.ch6
                .par
                .write(|w| w.bits(core::ptr::addr_of!(usart2.rdr) as u32));
            dma.ch6.mar.write(|w| w.bits(I6X_RX_DMA_BUF.as_ptr() as u32));
            dma.ch6.ndtr.write(|w| w.bits(RX_BUF_SIZE as u32));
            dma.ch6
                .cr
                .write(|w| w.bits(DMA_CCR_CIRC | DMA_CCR_MINC | DMA_CCR_EN));

            // TX DMA (Channel 7)
            dma.ch7.cr.write(|w| w.bits(0));
            dma.ch7
                .par
                .write(|w| w.bits(core::ptr::addr_of!(usart2.tdr) as u32));
            dma.ch7.mar.write(|w| w.bits(I6X_TX_DMA_BUF.as_ptr() as u32));

            I6X_RX_TAIL.store(0, Ordering::Release);
            I6X_TX_BUSY.store(false, Ordering::Release);

            usart2.icr.write(|w| w.bits(0xFFFF_FFFF));

            cortex_m::peripheral::NVIC::unpend(pac::Interrupt::USART2);
            let mut cp = cortex_m::peripheral::Peripherals::steal();
            cp.NVIC.set_priority(pac::Interrupt::USART2, 0x40);
            cortex_m::peripheral::NVIC::unmask(pac::Interrupt::USART2);

            // Enable USART, TX, RX, TCIE
            usart2
                .cr1
                .write(|w| w.bits((1 << 0) | (1 << 2) | (1 << 3) | (1 << 6)));
        }
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        if bytes.is_empty() || bytes.len() > TX_BUF_SIZE {
            return;
        }

        unsafe {
            let usart2 = &*pac::USART2::ptr();
            let dma = &*pac::DMA1::ptr();

            let mut timeout = 20_000u32;
            while I6X_TX_BUSY.load(Ordering::Acquire) && timeout > 0 {
                timeout -= 1;
            }

            I6X_TX_BUSY.store(true, Ordering::Release);

            if self.half_duplex {
                usart2.cr1.modify(|r, w| w.bits(r.bits() & !(1 << 2)));
            }

            I6X_TX_DMA_BUF[..bytes.len()].copy_from_slice(bytes);

            dma.ch7.cr.write(|w| w.bits(0));
            dma.ifcr.write(|w| w.bits(0x0F << 24));
            dma.ch7.ndtr.write(|w| w.bits(bytes.len() as u32));
            dma.ch7
                .cr
                .write(|w| w.bits(DMA_CCR_DIR | DMA_CCR_MINC | DMA_CCR_EN));
        }
    }

    fn read_byte(&mut self) -> Option<u8> {
        unsafe {
            let usart2 = &*pac::USART2::ptr();
            if usart2.isr.read().ore().bit_is_set() {
                usart2.icr.write(|w| w.orecf().clear());
            }

            let dma = &*pac::DMA1::ptr();
            let remaining = dma.ch6.ndtr.read().bits() as usize;
            let head = (RX_BUF_SIZE - remaining) & (RX_BUF_SIZE - 1);
            let tail = I6X_RX_TAIL.load(Ordering::Relaxed);

            if tail == head {
                None
            } else {
                let byte = I6X_RX_DMA_BUF[tail];
                I6X_RX_TAIL.store((tail + 1) & (RX_BUF_SIZE - 1), Ordering::Release);
                Some(byte)
            }
        }
    }
}

#[interrupt]
fn USART2() {
    unsafe {
        let usart2 = &*pac::USART2::ptr();
        let isr = usart2.isr.read();

        if isr.ore().bit_is_set() {
            usart2.icr.write(|w| w.orecf().clear());
        }

        if isr.tc().bit_is_set() {
            usart2.icr.write(|w| w.tccf().clear());
            usart2.cr1.modify(|r, w| w.bits(r.bits() | (1 << 2)));
            I6X_TX_BUSY.store(false, Ordering::Release);
        }

        if isr.idle().bit_is_set() {
            usart2.icr.write(|w| w.idlecf().clear());
        }
    }
}

/// USART1 CRSF driver for FlySky FS-i6S (PB6 TX / PB7 RX, PF6 Power).
pub struct I6sSerialPort {
    active_high: bool,
    power_on: bool,
    half_duplex: bool,
}

impl I6sSerialPort {
    pub const fn new() -> Self {
        Self {
            active_high: true,
            power_on: false,
            half_duplex: false,
        }
    }

    unsafe fn apply_power_pin(&self) {
        let pin_high = if self.active_high {
            self.power_on
        } else {
            !self.power_on
        };

        let gpiof = &*pac::GPIOF::ptr();
        if pin_high {
            gpiof.bsrr.write(|w| w.bits(1 << 6));
        } else {
            gpiof.bsrr.write(|w| w.bits(1 << (6 + 16)));
        }
    }
}

impl Default for I6sSerialPort {
    fn default() -> Self {
        Self::new()
    }
}

impl SerialPort for I6sSerialPort {
    fn init(&mut self, active_high: bool) {
        self.active_high = active_high;
        self.power_on = false;

        unsafe {
            let rcc = &*pac::RCC::ptr();
            let gpiof = &*pac::GPIOF::ptr();

            rcc.ahbenr.modify(|r, w| w.bits(r.bits() | (1 << 22)));

            gpiof.moder.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !(3 << 12)) | (1 << 12))
            });

            self.apply_power_pin();
        }
    }

    fn set_power_polarity(&mut self, active_high: bool) {
        self.active_high = active_high;
        unsafe {
            self.apply_power_pin();
        }
    }

    fn set_module_power(&mut self, enabled: bool) {
        self.power_on = enabled;
        unsafe {
            self.apply_power_pin();
        }
    }

    fn set_uart_enabled(&mut self, enabled: bool, baud_idx: u8, half_duplex: bool) {
        self.half_duplex = half_duplex;
        unsafe {
            let rcc = &*pac::RCC::ptr();
            let usart1 = &*pac::USART1::ptr();
            let dma = &*pac::DMA1::ptr();
            let gpiob = &*pac::GPIOB::ptr();

            if !enabled {
                cortex_m::peripheral::NVIC::mask(pac::Interrupt::USART1);
                usart1.cr1.write(|w| w.bits(0));
                rcc.apb2enr.modify(|r, w| w.bits(r.bits() & !(1 << 14)));
                dma.ch3.cr.write(|w| w.bits(0));
                dma.ch2.cr.write(|w| w.bits(0));
                I6S_TX_BUSY.store(false, Ordering::Release);
                return;
            }

            rcc.ahbenr
                .modify(|r, w| w.bits(r.bits() | (1 << 0) | (1 << 18)));
            rcc.apb2enr.modify(|r, w| w.bits(r.bits() | (1 << 14)));

            // PB6 = AF0 (TX), PB7 = AF0 (RX)
            gpiob.moder.modify(|r, w| {
                let val = r.bits();
                w.bits((val & !(0x0F << 12)) | (0x0A << 12))
            });
            gpiob.afrl.modify(|r, w| {
                let val = r.bits();
                w.bits(val & !(0xFF << 24))
            });

            usart1.cr1.write(|w| w.bits(0));

            let brr = match baud_idx {
                0 => 417,
                1 => 115,
                2 => 114,
                3 => 52,
                4 => 26,
                _ => 115,
            };
            usart1.brr.write(|w| w.bits(brr));

            if half_duplex {
                usart1.cr3.write(|w| w.bits((1 << 3) | (1 << 6) | (1 << 7)));
            } else {
                usart1.cr3.write(|w| w.bits((1 << 6) | (1 << 7)));
            }

            // RX DMA (Channel 3 for USART1_RX)
            dma.ch3.cr.write(|w| w.bits(0));
            dma.ch3
                .par
                .write(|w| w.bits(core::ptr::addr_of!(usart1.rdr) as u32));
            dma.ch3.mar.write(|w| w.bits(I6S_RX_DMA_BUF.as_ptr() as u32));
            dma.ch3.ndtr.write(|w| w.bits(RX_BUF_SIZE as u32));
            dma.ch3
                .cr
                .write(|w| w.bits(DMA_CCR_CIRC | DMA_CCR_MINC | DMA_CCR_EN));

            // TX DMA (Channel 2 for USART1_TX)
            dma.ch2.cr.write(|w| w.bits(0));
            dma.ch2
                .par
                .write(|w| w.bits(core::ptr::addr_of!(usart1.tdr) as u32));
            dma.ch2.mar.write(|w| w.bits(I6S_TX_DMA_BUF.as_ptr() as u32));

            I6S_RX_TAIL.store(0, Ordering::Release);
            I6S_TX_BUSY.store(false, Ordering::Release);

            usart1.icr.write(|w| w.bits(0xFFFF_FFFF));

            cortex_m::peripheral::NVIC::unpend(pac::Interrupt::USART1);
            let mut cp = cortex_m::peripheral::Peripherals::steal();
            cp.NVIC.set_priority(pac::Interrupt::USART1, 0x40);
            cortex_m::peripheral::NVIC::unmask(pac::Interrupt::USART1);

            usart1
                .cr1
                .write(|w| w.bits((1 << 0) | (1 << 2) | (1 << 3) | (1 << 6)));
        }
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        if bytes.is_empty() || bytes.len() > TX_BUF_SIZE {
            return;
        }

        unsafe {
            let usart1 = &*pac::USART1::ptr();
            let dma = &*pac::DMA1::ptr();

            let mut timeout = 20_000u32;
            while I6S_TX_BUSY.load(Ordering::Acquire) && timeout > 0 {
                timeout -= 1;
            }

            I6S_TX_BUSY.store(true, Ordering::Release);

            if self.half_duplex {
                usart1.cr1.modify(|r, w| w.bits(r.bits() & !(1 << 2)));
            }

            I6S_TX_DMA_BUF[..bytes.len()].copy_from_slice(bytes);

            dma.ch2.cr.write(|w| w.bits(0));
            dma.ifcr.write(|w| w.bits(0x0F << 4));
            dma.ch2.ndtr.write(|w| w.bits(bytes.len() as u32));
            dma.ch2
                .cr
                .write(|w| w.bits(DMA_CCR_DIR | DMA_CCR_MINC | DMA_CCR_EN));
        }
    }

    fn read_byte(&mut self) -> Option<u8> {
        unsafe {
            let usart1 = &*pac::USART1::ptr();
            if usart1.isr.read().ore().bit_is_set() {
                usart1.icr.write(|w| w.orecf().clear());
            }

            let dma = &*pac::DMA1::ptr();
            let remaining = dma.ch3.ndtr.read().bits() as usize;
            let head = (RX_BUF_SIZE - remaining) & (RX_BUF_SIZE - 1);
            let tail = I6S_RX_TAIL.load(Ordering::Relaxed);

            if tail == head {
                None
            } else {
                let byte = I6S_RX_DMA_BUF[tail];
                I6S_RX_TAIL.store((tail + 1) & (RX_BUF_SIZE - 1), Ordering::Release);
                Some(byte)
            }
        }
    }
}

#[interrupt]
fn USART1() {
    unsafe {
        let usart1 = &*pac::USART1::ptr();
        let isr = usart1.isr.read();

        if isr.ore().bit_is_set() {
            usart1.icr.write(|w| w.orecf().clear());
        }

        if isr.tc().bit_is_set() {
            usart1.icr.write(|w| w.tccf().clear());
            usart1.cr1.modify(|r, w| w.bits(r.bits() | (1 << 2)));
            I6S_TX_BUSY.store(false, Ordering::Release);
        }

        if isr.idle().bit_is_set() {
            usart1.icr.write(|w| w.idlecf().clear());
        }
    }
}
