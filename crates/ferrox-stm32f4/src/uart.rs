use core::marker::PhantomData;
use ferrox_core::hal::SerialPort;
use stm32f4::stm32f407 as pac;

use crate::descriptor::{PocketHal, Qx7HalDescriptor};

const RX_BUF_SIZE: usize = 256;
static mut RX_DMA_BUF: [u8; RX_BUF_SIZE] = [0; RX_BUF_SIZE];
static mut RX_TAIL: usize = 0;

/// Dual-module capable serial port driver for STM32F407 radios.
///
/// Supports:
/// - Route 0 (`Internal`): `USART1` (PB6 TX / PB7 RX) with DMA2 Stream 2 Channel 4 and `INT_RF_PWR`
/// - Route 1 (`External (Bay)`): `USART6` (PC6 TX / PC7 RX) with DMA2 Stream 1 Channel 5 and `EXT_RF_PWR` (PD8 / PE0)
pub struct PocketSerialPort<H: Qx7HalDescriptor = PocketHal> {
    active: bool,
    route: u8,
    active_high_polarity: bool,
    last_baud_idx: u8,
    last_half_duplex: bool,
    _phantom: PhantomData<H>,
}

impl<H: Qx7HalDescriptor> PocketSerialPort<H> {
    pub const fn new() -> Self {
        Self {
            active: false,
            route: 0,
            active_high_polarity: true,
            last_baud_idx: 1,
            last_half_duplex: false,
            _phantom: PhantomData,
        }
    }
}

impl<H: Qx7HalDescriptor> Default for PocketSerialPort<H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H: Qx7HalDescriptor> SerialPort for PocketSerialPort<H> {
    fn init(&mut self, active_high: bool) {
        self.active_high_polarity = active_high;
        let rcc = unsafe { &*pac::RCC::ptr() };
        let gpiob = unsafe { &*pac::GPIOB::ptr() };
        let gpioc = unsafe { &*pac::GPIOC::ptr() };

        unsafe {
            // Enable GPIOB, GPIOC, GPIOD, GPIOE, DMA2 (AHB1) and USART1 + USART6 (APB2)
            rcc.ahb1enr.modify(|r, w| {
                w.bits(
                    r.bits()
                        | (1 << 1) // GPIOB
                        | (1 << 2) // GPIOC
                        | (1 << 3) // GPIOD
                        | (1 << 4) // GPIOE
                        | (1 << 22), // DMA2
                )
            });
            let _ = rcc.ahb1enr.read().bits();

            rcc.apb2enr.modify(|r, w| {
                w.bits(
                    r.bits()
                        | (1 << 4)  // USART1
                        | (1 << 5), // USART6
                )
            });
            let _ = rcc.apb2enr.read().bits();

            // Internal RF Boot pin (if present): GPIO Output, driven LOW (normal operation)
            if let Some(boot) = H::INT_RF_BOOTCMD {
                boot.enable_clock_and_output(false);
            }

            // Internal RF Power pin: GPIO Output, default OFF
            H::INT_RF_PWR.enable_clock_and_output(false);

            // External RF module bay power pin: GPIO Output, default OFF
            H::EXT_RF_PWR.enable_clock_and_output(false);

            // Configure USART1 GPIOs: PB6 (TX), PB7 (RX) -> AF7, Very High Speed
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

            // Configure USART6 GPIOs: PC6 (TX), PC7 (RX) -> AF8, Very High Speed
            gpioc.moder.modify(|r, w| {
                w.bits((r.bits() & !((0x03 << 12) | (0x03 << 14))) | (0x02 << 12) | (0x02 << 14))
            });
            gpioc
                .ospeedr
                .modify(|r, w| w.bits(r.bits() | (0x03 << 12) | (0x03 << 14)));
            gpioc.afrl.modify(|r, w| {
                let mask = (0x0F << 24) | (0x0F << 28);
                let af8 = (0x08 << 24) | (0x08 << 28);
                w.bits((r.bits() & !mask) | af8)
            });
        }
    }

    fn set_power_polarity(&mut self, active_high: bool) {
        self.active_high_polarity = active_high;
    }

    fn set_port_route(&mut self, route: u8) {
        let new_route = if route != 0 { 1 } else { 0 };
        if self.route != new_route {
            let was_active = self.active;
            let b_idx = self.last_baud_idx;
            let hd = self.last_half_duplex;

            if was_active {
                self.set_uart_enabled(false, b_idx, hd);
                self.set_module_power(false);
            }

            self.route = new_route;

            if was_active {
                self.set_uart_enabled(true, b_idx, hd);
                self.set_module_power(true);
            }
        }
    }

    fn set_uart_enabled(&mut self, enabled: bool, baud_idx: u8, half_duplex: bool) {
        self.last_baud_idx = baud_idx;
        self.last_half_duplex = half_duplex;

        let usart1 = unsafe { &*pac::USART1::ptr() };
        let usart6 = unsafe { &*pac::USART6::ptr() };
        let dma2 = unsafe { &*pac::DMA2::ptr() };

        if !enabled {
            self.active = false;
            unsafe {
                usart1.cr1.write(|w| w.bits(0));
                usart6.cr1.write(|w| w.bits(0));
                dma2.st[2].cr.write(|w| w.bits(0));
                dma2.st[1].cr.write(|w| w.bits(0));
            }
            return;
        }

        // Both USART1 and USART6 are clocked from APB2 = 84 MHz
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
            if self.route == 0 {
                // Internal Module: Route to USART1 (DMA2 Stream 2 Channel 4)
                usart6.cr1.write(|w| w.bits(0));
                dma2.st[1].cr.write(|w| w.bits(0));

                usart1.cr1.write(|w| w.bits(0));
                usart1.cr2.write(|w| w.bits(cr2_extra));
                let hdsel = if half_duplex { 1 << 3 } else { 0 };
                usart1.cr3.write(|w| w.bits((1 << 6) | hdsel));
                usart1.brr.write(|w| w.bits(brr));

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
                        (4 << 25)
                            | (0x01 << 16)
                            | (1 << 10)
                            | (1 << 8)
                            | (1 << 0),
                    )
                });

                RX_TAIL = 0;
                usart1
                    .cr1
                    .write(|w| w.bits((1 << 13) | (1 << 3) | (1 << 2) | cr1_extra));
            } else {
                // External Module Bay: Route to USART6 (DMA2 Stream 1 Channel 5)
                usart1.cr1.write(|w| w.bits(0));
                dma2.st[2].cr.write(|w| w.bits(0));

                usart6.cr1.write(|w| w.bits(0));
                usart6.cr2.write(|w| w.bits(cr2_extra));
                let hdsel = if half_duplex { 1 << 3 } else { 0 };
                usart6.cr3.write(|w| w.bits((1 << 6) | hdsel));
                usart6.brr.write(|w| w.bits(brr));

                let stream1 = &dma2.st[1];
                stream1.cr.write(|w| w.bits(0));
                let mut dma_timeout = 50_000u32;
                while (stream1.cr.read().bits() & 0x01) != 0 && dma_timeout > 0 {
                    dma_timeout -= 1;
                }

                dma2.lifcr.write(|w| w.bits(0x3D << 6)); // Clear Stream 1 flags

                stream1
                    .par
                    .write(|w| w.bits(core::ptr::addr_of!(usart6.dr) as u32));
                stream1
                    .m0ar
                    .write(|w| w.bits(core::ptr::addr_of_mut!(RX_DMA_BUF) as u32));
                stream1.ndtr.write(|w| w.bits(RX_BUF_SIZE as u32));

                // CHSEL = 5 (USART6_RX), MINC = 1, CIRC = 1, PL = 01 (Medium), EN = 1
                stream1.cr.write(|w| {
                    w.bits(
                        (5 << 25)
                            | (0x01 << 16)
                            | (1 << 10)
                            | (1 << 8)
                            | (1 << 0),
                    )
                });

                RX_TAIL = 0;
                usart6
                    .cr1
                    .write(|w| w.bits((1 << 13) | (1 << 3) | (1 << 2) | cr1_extra));
            }
        }

        self.active = true;
    }

    fn set_module_power(&mut self, enabled: bool) {
        if self.route == 0 {
            // Internal module
            if enabled {
                H::INT_RF_PWR.set_high();
            } else {
                H::INT_RF_PWR.set_low();
            }
            // Ensure external bay is powered down
            if self.active_high_polarity {
                H::EXT_RF_PWR.set_low();
            } else {
                H::EXT_RF_PWR.set_high();
            }
        } else {
            // External module bay
            if enabled {
                if self.active_high_polarity {
                    H::EXT_RF_PWR.set_high();
                } else {
                    H::EXT_RF_PWR.set_low();
                }
            } else if self.active_high_polarity {
                H::EXT_RF_PWR.set_low();
            } else {
                H::EXT_RF_PWR.set_high();
            }
            // Ensure internal module is powered down
            H::INT_RF_PWR.set_low();
        }
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        let usart1 = unsafe { &*pac::USART1::ptr() };
        let usart6 = unsafe { &*pac::USART6::ptr() };

        if self.route == 0 {
            for &b in bytes {
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
        } else {
            for &b in bytes {
                let mut timeout = 50_000;
                while (usart6.sr.read().bits() & (1 << 7)) == 0 && timeout > 0 {
                    timeout -= 1;
                }
                if timeout == 0 {
                    return;
                }
                unsafe {
                    let dr = core::ptr::addr_of!(usart6.dr) as *mut u8;
                    core::ptr::write_volatile(dr, b);
                }
            }
        }
    }

    fn read_byte(&mut self) -> Option<u8> {
        if !self.active {
            return None;
        }

        let dma2 = unsafe { &*pac::DMA2::ptr() };
        let ndtr = if self.route == 0 {
            dma2.st[2].ndtr.read().bits() as usize
        } else {
            dma2.st[1].ndtr.read().bits() as usize
        };
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
