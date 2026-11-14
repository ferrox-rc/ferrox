use core::marker::PhantomData;
use ferrox_core::hal::SerialPort;
use stm32f4::stm32f407 as pac;

use crate::descriptor::{PocketHal, Qx7HalDescriptor};

const RX_BUF_SIZE: usize = 256;
static mut RX_DMA_BUF: [u8; RX_BUF_SIZE] = [0; RX_BUF_SIZE];
static mut RX_TAIL: usize = 0;

#[cfg(feature = "multiswitch")]
const TX_BUF_SIZE: usize = 64;
#[cfg(feature = "multiswitch")]
static mut TX_DMA_BUF: [u8; TX_BUF_SIZE] = [0; TX_BUF_SIZE];
#[cfg(feature = "multiswitch")]
const TX_DMA_FLAGS: u32 = 0x3D << 22;

pub struct PocketSerialPort<H: Qx7HalDescriptor = PocketHal> {
    active: bool,
    _phantom: PhantomData<H>,
}

impl<H: Qx7HalDescriptor> PocketSerialPort<H> {
    pub const fn new() -> Self {
        Self {
            active: false,
            _phantom: PhantomData,
        }
    }

    #[cfg(feature = "multiswitch")]
    fn stop_tx_dma() {
        let dma2 = unsafe { &*pac::DMA2::ptr() };
        let usart1 = unsafe { &*pac::USART1::ptr() };
        let stream = &dma2.st[7];
        unsafe {
            stream.cr.modify(|r, w| w.bits(r.bits() & !1));
        }
        let mut timeout = 50_000;
        while stream.cr.read().bits() & 1 != 0 && timeout > 0 {
            timeout -= 1;
        }
        assert_eq!(
            stream.cr.read().bits() & 1,
            0,
            "USART1 TX DMA stop timed out"
        );
        unsafe {
            usart1.cr3.modify(|r, w| w.bits(r.bits() & !(1 << 7)));
            dma2.hifcr.write(|w| w.bits(TX_DMA_FLAGS));
        }
    }
}

impl<H: Qx7HalDescriptor> Default for PocketSerialPort<H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H: Qx7HalDescriptor> SerialPort for PocketSerialPort<H> {
    #[cfg(feature = "multiswitch")]
    fn try_write_bytes(&mut self, bytes: &[u8]) -> bool {
        if !self.active || bytes.is_empty() || bytes.len() > TX_BUF_SIZE {
            return false;
        }
        let dma2 = unsafe { &*pac::DMA2::ptr() };
        let usart1 = unsafe { &*pac::USART1::ptr() };
        let stream = &dma2.st[7];
        if stream.cr.read().bits() & 1 != 0 || usart1.sr.read().bits() & (1 << 6) == 0 {
            return false;
        }
        if dma2.hisr.read().bits() & (0x0D << 22) != 0 {
            unsafe {
                dma2.hifcr.write(|w| w.bits(TX_DMA_FLAGS));
            }
            return false;
        }
        // RM0090 Table 44: USART1 TX uses DMA2 Stream 7, Channel 4.
        unsafe {
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::Acquire);
            let buffer = core::ptr::addr_of_mut!(TX_DMA_BUF).cast::<u8>();
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, bytes.len());
            dma2.hifcr.write(|w| w.bits(TX_DMA_FLAGS));
            stream
                .par
                .write(|w| w.bits(core::ptr::addr_of!(usart1.dr) as u32));
            stream.m0ar.write(|w| w.bits(buffer as u32));
            stream.ndtr.write(|w| w.bits(bytes.len() as u32));
            stream.fcr.write(|w| w.bits(0));
            stream
                .cr
                .write(|w| w.bits((4 << 25) | (1 << 16) | (1 << 10) | (1 << 6)));
            usart1.cr3.modify(|r, w| w.bits(r.bits() | (1 << 7)));
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::Release);
            cortex_m::asm::dmb();
            stream.cr.modify(|r, w| w.bits(r.bits() | 1));
        }
        true
    }

    fn init(&mut self, _active_high: bool) {
        let rcc = unsafe { &*pac::RCC::ptr() };
        let gpiob = unsafe { &*pac::GPIOB::ptr() };
        let gpiod = unsafe { &*pac::GPIOD::ptr() };

        unsafe {
            // Enable GPIOB, GPIOC, GPIOD, DMA2 (AHB1) and USART1 (APB2)
            rcc.ahb1enr
                .modify(|r, w| w.bits(r.bits() | (1 << 1) | (1 << 2) | (1 << 3) | (1 << 22)));
            let _ = rcc.ahb1enr.read().bits();
            rcc.apb2enr.modify(|r, w| w.bits(r.bits() | (1 << 4)));
            let _ = rcc.apb2enr.read().bits();

            // Internal RF Boot pin (if present): GPIO Output, driven LOW (normal operation)
            if let Some(boot) = H::INT_RF_BOOTCMD {
                boot.enable_clock_and_output(false);
            }

            // Internal RF Power pin: GPIO Output, default OFF
            H::INT_RF_PWR.enable_clock_and_output(false);

            // PD8 (EXTMODULE_PWR): GPIO Output, default OFF
            gpiod
                .moder
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 16)) | (0x01 << 16)));
            gpiod.bsrr.write(|w| w.bits(1 << (8 + 16))); // PD8 LOW

            // PB6 (TX), PB7 (RX): AF7 (USART1), Very High Speed
            gpiob.moder.modify(|r, w| {
                w.bits((r.bits() & !((0x03 << 12) | (0x03 << 14))) | (0x02 << 12) | (0x02 << 14))
            });
            gpiob
                .ospeedr
                .modify(|r, w| w.bits(r.bits() | (0x03 << 12) | (0x03 << 14)));
            gpiob.afrl.modify(|r, w| {
                let mask = (0x0F << 24) | (0x0F << 28);
                let af7 = (0x07 << 24) | (0x07 << 28);
                w.bits((r.bits() & !mask) | af7)
            });
        }
    }

    fn set_power_polarity(&mut self, _active_high: bool) {}

    fn set_uart_enabled(&mut self, enabled: bool, baud_idx: u8, half_duplex: bool) {
        let usart1 = unsafe { &*pac::USART1::ptr() };
        let dma2 = unsafe { &*pac::DMA2::ptr() };

        #[cfg(feature = "multiswitch")]
        Self::stop_tx_dma();

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
            let hdsel = if half_duplex { 1 << 3 } else { 0 };
            usart1.cr3.write(|w| w.bits((1 << 6) | hdsel)); // DMAR = 1, optional HDSEL
            usart1.brr.write(|w| w.bits(brr));

            // Configure DMA2 Stream 2 (Channel 4): Circular, Memory increment, Byte size
            let stream2 = &dma2.st[2];
            stream2.cr.write(|w| w.bits(0));
            let mut dma_timeout = 50_000u32;
            while (stream2.cr.read().bits() & 0x01) != 0 && dma_timeout > 0 {
                dma_timeout -= 1;
            }

            dma2.lifcr.write(|w| w.bits(0x3D << 16)); // Clear Stream 2 flags

            stream2
                .par
                .write(|w| w.bits(core::ptr::addr_of!(usart1.dr) as u32));
            stream2
                .m0ar
                .write(|w| w.bits(core::ptr::addr_of_mut!(RX_DMA_BUF) as u32));
            stream2.ndtr.write(|w| w.bits(RX_BUF_SIZE as u32));

            // CHSEL = 4, MINC = 1, CIRC = 1, PL = 01 (Medium), EN = 1
            stream2.cr.write(|w| {
                w.bits(
                    (4 << 25)      // Channel 4
                    | (0x01 << 16) // Priority Medium
                    | (1 << 10)    // MINC
                    | (1 << 8)     // CIRC
                    | (1 << 0), // EN
                )
            });

            RX_TAIL = 0;

            // Enable USART1: UE = 1, TE = 1, RE = 1
            usart1
                .cr1
                .write(|w| w.bits((1 << 13) | (1 << 3) | (1 << 2) | cr1_extra));
        }

        self.active = true;
    }

    fn set_module_power(&mut self, enabled: bool) {
        if enabled {
            H::INT_RF_PWR.set_high();
        } else {
            H::INT_RF_PWR.set_low();
        }
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        let usart1 = unsafe { &*pac::USART1::ptr() };

        #[cfg(feature = "multiswitch")]
        {
            let dma2 = unsafe { &*pac::DMA2::ptr() };
            let mut timeout = 50_000;
            while (dma2.st[7].cr.read().bits() & 1 != 0 || usart1.sr.read().bits() & (1 << 6) == 0)
                && timeout > 0
            {
                timeout -= 1;
            }
            if timeout == 0 {
                panic!("USART1 TX DMA completion timed out");
            }
            unsafe {
                usart1.cr3.modify(|r, w| w.bits(r.bits() & !(1 << 7)));
            }
        }

        for &b in bytes {
            // Wait for TXE (Transmit data register empty, bit 7 in SR)
            let mut timeout = 50_000;
            while (usart1.sr.read().bits() & (1 << 7)) == 0 && timeout > 0 {
                timeout -= 1;
            }
            if timeout == 0 {
                return;
            }
            unsafe {
                let dr = core::ptr::addr_of!(usart1.dr) as *mut u8;
                core::ptr::write_volatile(dr, b);
            }
        }
    }

    fn read_byte(&mut self) -> Option<u8> {
        if !self.active {
            return None;
        }

        let dma2 = unsafe { &*pac::DMA2::ptr() };
        let ndtr = dma2.st[2].ndtr.read().bits() as usize;
        let head = RX_BUF_SIZE.saturating_sub(ndtr) % RX_BUF_SIZE;

        unsafe {
            if RX_TAIL != head {
                let b = core::ptr::read_volatile(&RX_DMA_BUF[RX_TAIL]);
                RX_TAIL = (RX_TAIL + 1) % RX_BUF_SIZE;
                Some(b)
            } else {
                None
            }
        }
    }
}
