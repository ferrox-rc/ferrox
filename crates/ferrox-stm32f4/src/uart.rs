use ferrox_core::hal::SerialPort;
use stm32f4::stm32f407 as pac;

const RX_BUF_SIZE: usize = 256;
static mut RX_DMA_BUF: [u8; RX_BUF_SIZE] = [0; RX_BUF_SIZE];
static mut RX_TAIL: usize = 0;

pub struct PocketSerialPort {
    active: bool,
}

impl PocketSerialPort {
    pub const fn new() -> Self {
        Self { active: false }
    }
}

impl Default for PocketSerialPort {
    fn default() -> Self {
        Self::new()
    }
}

impl SerialPort for PocketSerialPort {
    fn init(&mut self, _active_high: bool) {
        let rcc = unsafe { &*pac::RCC::ptr() };
        let gpiob = unsafe { &*pac::GPIOB::ptr() };
        let gpioc = unsafe { &*pac::GPIOC::ptr() };
        let gpiod = unsafe { &*pac::GPIOD::ptr() };

        unsafe {
            // Enable GPIOB, GPIOC, GPIOD, DMA2 (AHB1) and USART1 (APB2)
            rcc.ahb1enr.modify(|r, w| {
                w.bits(r.bits() | (1 << 1) | (1 << 2) | (1 << 3) | (1 << 22))
            });
            rcc.apb2enr.modify(|r, w| w.bits(r.bits() | (1 << 4)));

            // PB1 (INTMODULE_BOOTCMD): GPIO Output, driven LOW (normal operation)
            gpiob
                .moder
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 2)) | (0x01 << 2)));
            gpiob.bsrr.write(|w| w.bits(1 << (1 + 16))); // PB1 LOW

            // PC4 (INTMODULE_PWR): GPIO Output, default OFF
            gpioc
                .moder
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 8)) | (0x01 << 8)));
            gpioc.bsrr.write(|w| w.bits(1 << (4 + 16))); // PC4 LOW

            // PD8 (EXTMODULE_PWR): GPIO Output, default OFF
            gpiod
                .moder
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 16)) | (0x01 << 16)));
            gpiod.bsrr.write(|w| w.bits(1 << (8 + 16))); // PD8 LOW

            // PB6 (TX), PB7 (RX): AF7 (USART1), Very High Speed
            gpiob.moder.modify(|r, w| {
                w.bits((r.bits() & !((0x03 << 12) | (0x03 << 14))) | (0x02 << 12) | (0x02 << 14))
            });
            gpiob.ospeedr.modify(|r, w| {
                w.bits(r.bits() | (0x03 << 12) | (0x03 << 14))
            });
            gpiob.afrl.modify(|r, w| {
                let mask = (0x0F << 24) | (0x0F << 28);
                let af7 = (0x07 << 24) | (0x07 << 28);
                w.bits((r.bits() & !mask) | af7)
            });
        }
    }

    fn set_power_polarity(&mut self, _active_high: bool) {}

    fn set_uart_enabled(&mut self, enabled: bool, baud_idx: u8, _half_duplex: bool) {
        let usart1 = unsafe { &*pac::USART1::ptr() };
        let dma2 = unsafe { &*pac::DMA2::ptr() };

        if !enabled {
            self.active = false;
            unsafe {
                usart1.cr1.write(|w| w.bits(0));
                dma2.st[2].cr.write(|w| w.bits(0));
            }
            return;
        }

        // USART1 clock on APB2 = 84 MHz
        // BRR = 84_000_000 / baud_rate
        let (brr, cr1_extra, cr2_extra) = match baud_idx {
            0 => (729, 0, 0), // 115200 baud
            1 => (210, 0, 0), // 400000 baud
            2 => (200, 0, 0), // 420000 baud
            3 => (91, 0, 0),  // 921600 baud
            4 => (45, 0, 0),  // 1875000 baud
            5 => (
                840,
                (1 << 12) | (1 << 10), // M=1 (9-bit), PCE=1 (even parity)
                0x02 << 12,            // STOP=2 (2 stop bits)
            ),
            6 => (729, 0, 0), // DSM Satellite 115200 8N1
            _ => (210, 0, 0), // fallback 400k
        };

        unsafe {
            // Disable USART1 while reconfiguring
            usart1.cr1.write(|w| w.bits(0));
            usart1.cr2.write(|w| w.bits(cr2_extra));
            usart1.cr3.write(|w| w.bits(1 << 6)); // DMAR = 1 (DMA Receiver Enable)
            usart1.brr.write(|w| w.bits(brr));

            // Configure DMA2 Stream 2 (Channel 4): Circular, Memory increment, Byte size
            let stream2 = &dma2.st[2];
            stream2.cr.write(|w| w.bits(0));
            while stream2.cr.read().bits() & 0x01 != 0 {}

            dma2.lifcr.write(|w| w.bits(0x3D << 16)); // Clear Stream 2 flags

            stream2
                .par
                .write(|w| w.bits(core::ptr::addr_of!(usart1.dr) as u32));
            stream2
                .m0ar
                .write(|w| w.bits(core::ptr::addr_of_mut!(RX_DMA_BUF) as u32));
            stream2.ndtr.write(|w| w.bits(RX_BUF_SIZE as u32));

            // CHSEL = 4, MINC = 1, CIRC = 1, EN = 1
            stream2.cr.write(|w| {
                w.bits(
                    (4 << 25)  // Channel 4
                    | (1 << 10) // MINC
                    | (1 << 8)  // CIRC
                    | (1 << 0), // EN
                )
            });

            RX_TAIL = 0;

            // Enable USART1: UE=1, TE=1, RE=1 + cr1_extra
            usart1
                .cr1
                .write(|w| w.bits((1 << 13) | (1 << 3) | (1 << 2) | cr1_extra));
        }

        self.active = true;
    }

    fn set_module_power(&mut self, enabled: bool) {
        let gpioc = unsafe { &*pac::GPIOC::ptr() };
        let gpiod = unsafe { &*pac::GPIOD::ptr() };
        unsafe {
            if enabled {
                gpioc.bsrr.write(|w| w.bits(1 << 4)); // PC4 HIGH (Internal RF ON)
                gpiod.bsrr.write(|w| w.bits(1 << 8)); // PD8 HIGH (External Bay ON)
            } else {
                gpioc.bsrr.write(|w| w.bits(1 << (4 + 16))); // PC4 LOW
                gpiod.bsrr.write(|w| w.bits(1 << (8 + 16))); // PD8 LOW
            }
        }
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        let usart1 = unsafe { &*pac::USART1::ptr() };
        unsafe {
            for &b in bytes {
                // Wait for TXE (bit 7)
                while (usart1.sr.read().bits() & (1 << 7)) == 0 {}
                usart1.dr.write(|w| w.bits(b as u32));
            }
            // Wait for TC (bit 6)
            while (usart1.sr.read().bits() & (1 << 6)) == 0 {}
        }
    }

    fn read_byte(&mut self) -> Option<u8> {
        let dma2 = unsafe { &*pac::DMA2::ptr() };
        let stream2 = &dma2.st[2];
        let ndtr = stream2.ndtr.read().bits() as usize;
        let head = RX_BUF_SIZE.saturating_sub(ndtr) % RX_BUF_SIZE;
        let tail = unsafe { RX_TAIL };

        if head != tail {
            let byte = unsafe { RX_DMA_BUF[tail] };
            unsafe {
                RX_TAIL = (tail + 1) % RX_BUF_SIZE;
            }
            Some(byte)
        } else {
            None
        }
    }
}
