use core::marker::PhantomData;
use ferrox_core::hal::SerialPort;
use stm32f4::stm32f407 as pac;

use crate::descriptor::{PinDef, PocketHal, Port, Qx7HalDescriptor};

const RX_BUF_SIZE: usize = 256;
static mut RX_DMA_BUF: [u8; RX_BUF_SIZE] = [0; RX_BUF_SIZE];
static mut RX_TAIL: usize = 0;

/// External bay S.Port direction control pin (`PD4` on STM32F4 Taranis / RadioMaster / Jumper).
const TELEMETRY_DIR_PIN: PinDef = PinDef::new(Port::D, 4);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum BusMode {
    /// Internal RF module: `USART1` (`PB6` TX / `PB7` RX), DMA2 Stream 2 Ch 4, `INT_RF_PWR`.
    InternalUsart1,
    /// External JR/Nano bay CRSF Half-Duplex (Pins 3, 4, 5 `S.Port`):
    /// `USART2` (`PD5` TX / `PD6` RX) + `PD4` hardware direction toggle, `EXT_RF_PWR`.
    ExternalSportUsart2,
    /// External JR/Nano bay CRSF Full-Duplex (Pins 1, 2, 3, 4):
    /// `USART6` (`PC6` TX / `PC7` RX), DMA2 Stream 1 Ch 5, `EXT_RF_PWR`.
    ExternalFullUsart6,
    /// External JR/Nano bay MPM 4-in-1 / CC2500 / DSM (Pin 1 TX + Pin 5 RX):
    /// `USART6` TX (`PC6`, 100k 8E2) + `USART2` RX (`PD6`, `PD4` held in RX mode), `EXT_RF_PWR`.
    ExternalSplitMpm,
}

/// Multi-bus serial port driver for STM32F407 radios.
///
/// Supports internal RF (`USART1`) and all 5-pin JR/Nano external module bay configurations
/// (`USART2` Pin 5 S.Port half-duplex with `PD4` direction control, `USART6` Pin 1/2 full-duplex,
/// and split `USART6` TX + `USART2` RX for 4-in-1 / CC2500 / DSM external modules).
pub struct Stm32f4SerialPort<H: Qx7HalDescriptor = PocketHal> {
    active: bool,
    route: u8,
    bus_mode: BusMode,
    active_high_polarity: bool,
    last_baud_idx: u8,
    last_half_duplex: bool,
    _phantom: PhantomData<H>,
}

impl<H: Qx7HalDescriptor> Stm32f4SerialPort<H> {
    pub const fn new() -> Self {
        Self {
            active: false,
            route: 0,
            bus_mode: BusMode::InternalUsart1,
            active_high_polarity: true,
            last_baud_idx: 1,
            last_half_duplex: false,
            _phantom: PhantomData,
        }
    }

    #[inline(always)]
    fn set_sport_dir_tx() {
        if H::TELEMETRY_DIR_TX_HIGH {
            TELEMETRY_DIR_PIN.set_high();
        } else {
            TELEMETRY_DIR_PIN.set_low();
        }
    }

    #[inline(always)]
    fn set_sport_dir_rx() {
        if H::TELEMETRY_DIR_TX_HIGH {
            TELEMETRY_DIR_PIN.set_low();
        } else {
            TELEMETRY_DIR_PIN.set_high();
        }
    }

    fn select_bus_mode(route: u8, baud_idx: u8, half_duplex: bool) -> BusMode {
        if route == 0 {
            BusMode::InternalUsart1
        } else if baud_idx == 5 || baud_idx == 6 {
            BusMode::ExternalSplitMpm
        } else if half_duplex {
            BusMode::ExternalSportUsart2
        } else {
            BusMode::ExternalFullUsart6
        }
    }
}

impl<H: Qx7HalDescriptor> Default for Stm32f4SerialPort<H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H: Qx7HalDescriptor> SerialPort for Stm32f4SerialPort<H> {
    fn init(&mut self, active_high: bool) {
        self.active_high_polarity = active_high;
        let rcc = unsafe { &*pac::RCC::ptr() };
        let gpiob = unsafe { &*pac::GPIOB::ptr() };
        let gpioc = unsafe { &*pac::GPIOC::ptr() };
        let gpiod = unsafe { &*pac::GPIOD::ptr() };

        unsafe {
            // Enable GPIOB, GPIOC, GPIOD, GPIOE, DMA2 (AHB1)
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

            // Enable USART1 + USART6 (APB2 = 84 MHz) and USART2 (APB1 = 42 MHz)
            rcc.apb2enr.modify(|r, w| {
                w.bits(
                    r.bits()
                        | (1 << 4)  // USART1
                        | (1 << 5), // USART6
                )
            });
            let _ = rcc.apb2enr.read().bits();

            rcc.apb1enr.modify(|r, w| w.bits(r.bits() | (1 << 17))); // USART2
            let _ = rcc.apb1enr.read().bits();

            // Internal RF Boot pin (if present): GPIO Output, driven LOW (normal operation)
            if let Some(boot) = H::INT_RF_BOOTCMD {
                boot.enable_clock_and_output(false);
            }

            // Internal & External RF Power pins: GPIO Output, default OFF
            H::INT_RF_PWR.enable_clock_and_output(false);
            H::EXT_RF_PWR.enable_clock_and_output(false);

            // External S.Port direction pin (PD4): GPIO Output, default RX
            TELEMETRY_DIR_PIN.enable_clock_and_output(!H::TELEMETRY_DIR_TX_HIGH);

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

            // Configure USART2 GPIOs: PD5 (TX), PD6 (RX) -> AF7, Very High Speed
            gpiod.moder.modify(|r, w| {
                w.bits((r.bits() & !((0x03 << 10) | (0x03 << 12))) | (0x02 << 10) | (0x02 << 12))
            });
            gpiod
                .ospeedr
                .modify(|r, w| w.bits(r.bits() | (0x03 << 10) | (0x03 << 12)));
            gpiod.afrl.modify(|r, w| {
                let mask = (0x0F << 20) | (0x0F << 24);
                let af7 = (0x07 << 20) | (0x07 << 24);
                w.bits((r.bits() & !mask) | af7)
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
        let usart2 = unsafe { &*pac::USART2::ptr() };
        let usart6 = unsafe { &*pac::USART6::ptr() };
        let dma2 = unsafe { &*pac::DMA2::ptr() };

        // Disable all 3 USARTs and RX DMA streams before reconfiguring
        unsafe {
            usart1.cr1.write(|w| w.bits(0));
            usart2.cr1.write(|w| w.bits(0));
            usart6.cr1.write(|w| w.bits(0));
            dma2.st[2].cr.write(|w| w.bits(0));
            dma2.st[1].cr.write(|w| w.bits(0));
        }
        Self::set_sport_dir_rx();

        if !enabled {
            self.active = false;
            return;
        }

        // APB2 = 84 MHz (USART1, USART6); APB1 = 42 MHz (USART2)
        let (brr_apb2, brr_apb1, cr1_extra, cr2_extra) = match baud_idx {
            0 => (729, 365, 0, 0), // 115200 baud
            1 => (210, 105, 0, 0), // 400000 baud
            2 => (200, 100, 0, 0), // 420000 baud
            3 => (91, 46, 0, 0),   // 921600 baud
            4 => (45, 22, 0, 0),   // 1875000 baud
            5 => (
                840,
                420,
                (1 << 12) | (1 << 10), // M=1 (9-bit), PCE=1 (even parity)
                0x02 << 12,            // STOP=2 (2 stop bits)
            ),
            6 => (729, 365, 0, 0), // DSM Satellite 115200 8N1
            _ => (210, 105, 0, 0), // fallback 400k
        };

        let mode = Self::select_bus_mode(self.route, baud_idx, half_duplex);
        self.bus_mode = mode;

        unsafe {
            match mode {
                BusMode::InternalUsart1 => {
                    usart1.cr2.write(|w| w.bits(cr2_extra));
                    let hdsel = if half_duplex { 1 << 3 } else { 0 };
                    usart1.cr3.write(|w| w.bits((1 << 6) | hdsel));
                    usart1.brr.write(|w| w.bits(brr_apb2));

                    let stream2 = &dma2.st[2];
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
                    // CHSEL = 4 (USART1_RX), MINC = 1, CIRC = 1, PL = 01, EN = 1
                    stream2.cr.write(|w| {
                        w.bits((4 << 25) | (0x01 << 16) | (1 << 10) | (1 << 8) | (1 << 0))
                    });

                    RX_TAIL = 0;
                    usart1
                        .cr1
                        .write(|w| w.bits((1 << 13) | (1 << 3) | (1 << 2) | cr1_extra));
                }
                BusMode::ExternalSportUsart2 => {
                    // External CRSF Half-Duplex on JR Pin 5 (S.Port):
                    // USART2 (PD5 TX / PD6 RX) with external PD4 direction buffer.
                    // Uses RXNE polling (avoids DMA1 Stream 5 conflict with DAC1 audio).
                    usart2.cr2.write(|w| w.bits(cr2_extra));
                    usart2.cr3.write(|w| w.bits(0));
                    usart2.brr.write(|w| w.bits(brr_apb1));
                    usart2
                        .cr1
                        .write(|w| w.bits((1 << 13) | (1 << 3) | (1 << 2) | cr1_extra));
                }
                BusMode::ExternalFullUsart6 => {
                    // External CRSF Full-Duplex on JR Pins 1/2: USART6 (PC6 TX / PC7 RX)
                    usart6.cr2.write(|w| w.bits(cr2_extra));
                    usart6.cr3.write(|w| w.bits(1 << 6)); // DMAR
                    usart6.brr.write(|w| w.bits(brr_apb2));

                    let stream1 = &dma2.st[1];
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
                    // CHSEL = 5 (USART6_RX), MINC = 1, CIRC = 1, PL = 01, EN = 1
                    stream1.cr.write(|w| {
                        w.bits((5 << 25) | (0x01 << 16) | (1 << 10) | (1 << 8) | (1 << 0))
                    });

                    RX_TAIL = 0;
                    usart6
                        .cr1
                        .write(|w| w.bits((1 << 13) | (1 << 3) | (1 << 2) | cr1_extra));
                }
                BusMode::ExternalSplitMpm => {
                    // External MPM 4-in-1 / CC2500 / DSM:
                    // TX on JR Pin 1 via USART6_TX (PC6), RX telemetry on JR Pin 5 via USART2_RX (PD6)
                    usart6.cr2.write(|w| w.bits(cr2_extra));
                    usart6.cr3.write(|w| w.bits(0));
                    usart6.brr.write(|w| w.bits(brr_apb2));
                    usart6
                        .cr1
                        .write(|w| w.bits((1 << 13) | (1 << 3) | cr1_extra)); // UE | TE

                    usart2.cr2.write(|w| w.bits(cr2_extra));
                    usart2.cr3.write(|w| w.bits(0));
                    usart2.brr.write(|w| w.bits(brr_apb1));
                    usart2
                        .cr1
                        .write(|w| w.bits((1 << 13) | (1 << 2) | cr1_extra)); // UE | RE
                }
            }
        }

        self.active = true;
    }

    fn set_module_power(&mut self, enabled: bool) {
        if self.route == 0 {
            if enabled {
                H::INT_RF_PWR.set_high();
            } else {
                H::INT_RF_PWR.set_low();
            }
            if self.active_high_polarity {
                H::EXT_RF_PWR.set_low();
            } else {
                H::EXT_RF_PWR.set_high();
            }
        } else {
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
            H::INT_RF_PWR.set_low();
        }
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        if !self.active || bytes.is_empty() {
            return;
        }

        match self.bus_mode {
            BusMode::InternalUsart1 => {
                let usart1 = unsafe { &*pac::USART1::ptr() };
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
            }
            BusMode::ExternalSportUsart2 => {
                let usart2 = unsafe { &*pac::USART2::ptr() };
                Self::set_sport_dir_tx();
                for &b in bytes {
                    let mut timeout = 50_000;
                    while (usart2.sr.read().bits() & (1 << 7)) == 0 && timeout > 0 {
                        timeout -= 1;
                    }
                    if timeout == 0 {
                        Self::set_sport_dir_rx();
                        return;
                    }
                    unsafe {
                        let dr = core::ptr::addr_of!(usart2.dr) as *mut u8;
                        core::ptr::write_volatile(dr, b);
                    }
                }
                // Wait for Transmit Complete (TC, bit 6) before releasing Pin 5 back to RX
                let mut tc_timeout = 50_000;
                while (usart2.sr.read().bits() & (1 << 6)) == 0 && tc_timeout > 0 {
                    tc_timeout -= 1;
                }
                Self::set_sport_dir_rx();
                // Drain any local echo byte received during half-duplex TX
                while (usart2.sr.read().bits() & (1 << 5)) != 0 {
                    let _ = usart2.dr.read().bits();
                }
            }
            BusMode::ExternalFullUsart6 | BusMode::ExternalSplitMpm => {
                let usart6 = unsafe { &*pac::USART6::ptr() };
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
    }

    fn read_byte(&mut self) -> Option<u8> {
        if !self.active {
            return None;
        }

        match self.bus_mode {
            BusMode::InternalUsart1 => {
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
            BusMode::ExternalFullUsart6 => {
                let dma2 = unsafe { &*pac::DMA2::ptr() };
                let ndtr = dma2.st[1].ndtr.read().bits() as usize;
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
            BusMode::ExternalSportUsart2 | BusMode::ExternalSplitMpm => {
                let usart2 = unsafe { &*pac::USART2::ptr() };
                let sr = usart2.sr.read().bits();
                // Clear ORE/FE/NE if set by reading DR
                if (sr & ((1 << 5) | (1 << 3))) != 0 {
                    Some((usart2.dr.read().bits() & 0xFF) as u8)
                } else {
                    None
                }
            }
        }
    }
}
