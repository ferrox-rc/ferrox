use core::marker::PhantomData;
use core::sync::atomic::{AtomicBool, Ordering};
use ferrox_core::hal::{SerialFraming, SerialPort};
use stm32f4::stm32f407 as pac;
use stm32f4::stm32f407::interrupt;

use crate::descriptor::{PinDef, PocketHal, Port, Qx7HalDescriptor};

const RX_BUF_SIZE: usize = 256;
static mut RX_DMA_BUF: [u8; RX_BUF_SIZE] = [0; RX_BUF_SIZE];
static mut RX_TAIL: usize = 0;

/// Max pulse transitions for a 27-byte MPM frame (`27 * 10 = 270`, padded to `320` for margin).
const SOFT_PULSE_BUF_SIZE: usize = 320;
static mut TIM8_PULSE_BUF: [u16; SOFT_PULSE_BUF_SIZE] = [0; SOFT_PULSE_BUF_SIZE];
static SOFT_TX_BUSY: AtomicBool = AtomicBool::new(false);

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
    /// Inverted TX on `PC6` via `TIM8_CH1` + `DMA2_Stream1` Ch 7 (or `USART6` if non-inverted),
    /// plus `USART2` RX on `PD6` (`PD4` held in RX mode), `EXT_RF_PWR`.
    ExternalSplitMpm,
}

/// Multi-bus serial port driver for STM32F407 radios.
///
/// Supports internal RF (`USART1`) and all 5-pin JR/Nano external module bay configurations
/// (`USART2` Pin 5 S.Port half-duplex with `PD4` direction control, `USART6` Pin 1/2 full-duplex,
/// and split `TIM8_CH1` inverted TX on Pin 1 + `USART2` RX on Pin 5 for 4-in-1 / CC2500 / DSM).
pub struct Stm32f4SerialPort<H: Qx7HalDescriptor = PocketHal> {
    active: bool,
    route: u8,
    bus_mode: BusMode,
    tx_inverted: bool,
    framing: SerialFraming,
    active_high_polarity: bool,
    last_baud_idx: u8,
    last_half_duplex: bool,
    last_cfg: Option<ferrox_core::hal::SerialRouteConfig>,
    rx_bytes: u32,
    tx_errors: u32,
    _phantom: PhantomData<H>,
}

impl<H: Qx7HalDescriptor> Stm32f4SerialPort<H> {
    pub const fn new() -> Self {
        Self {
            active: false,
            route: 0,
            bus_mode: BusMode::InternalUsart1,
            tx_inverted: false,
            framing: SerialFraming::Format8N1,
            active_high_polarity: true,
            last_baud_idx: 1,
            last_half_duplex: false,
            last_cfg: None,
            rx_bytes: 0,
            tx_errors: 0,
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

    /// Switch `PC6` alternate function between `AF3` (`TIM8_CH1`) and `AF8` (`USART6_TX`).
    #[inline(always)]
    unsafe fn set_pc6_af(af: u32) {
        let gpioc = &*pac::GPIOC::ptr();
        gpioc.afrl.modify(|r, w| {
            let mask = 0x0Fu32 << 24;
            w.bits((r.bits() & !mask) | ((af & 0x0F) << 24))
        });
    }

    /// Stop `TIM8` soft-serial TX and `DMA2_Stream1`, holding `PC6` in `FORCED_INACTIVE` (`LOW`).
    unsafe fn stop_tim8_softserial() {
        cortex_m::peripheral::NVIC::mask(pac::Interrupt::DMA2_STREAM1);
        cortex_m::peripheral::NVIC::mask(pac::Interrupt::TIM8_UP_TIM13);

        let tim8 = &*pac::TIM8::ptr();
        let dma2 = &*pac::DMA2::ptr();

        tim8.cr1.write(|w| w.bits(0));
        tim8.dier.write(|w| w.bits(0));
        // OC1M = 0b100 (Forced Inactive -> OC1Ref = 0 -> PC6 LOW), OC1PE = 1
        tim8.ccmr1_output().write(|w| w.bits((0x04 << 4) | (1 << 3)));
        tim8.sr.write(|w| w.bits(0));

        let stream1 = &dma2.st[1];
        stream1.cr.write(|w| w.bits(0));
        let mut timeout = 50_000u32;
        while (stream1.cr.read().bits() & 0x01) != 0 && timeout > 0 {
            timeout -= 1;
        }
        dma2.lifcr.write(|w| w.bits(0x3D << 6));

        cortex_m::peripheral::NVIC::unpend(pac::Interrupt::DMA2_STREAM1);
        cortex_m::peripheral::NVIC::unpend(pac::Interrupt::TIM8_UP_TIM13);
        SOFT_TX_BUSY.store(false, Ordering::Release);
    }

    #[inline(always)]
    fn bus_mode_from_route(cfg: ferrox_core::hal::SerialRouteConfig) -> BusMode {
        if !cfg.external_bay {
            BusMode::InternalUsart1
        } else {
            match cfg.duplex {
                ferrox_core::hal::SerialDuplex::FullDuplex => BusMode::ExternalFullUsart6,
                ferrox_core::hal::SerialDuplex::HalfDuplexSport => BusMode::ExternalSportUsart2,
                ferrox_core::hal::SerialDuplex::SplitTxPin1RxPin5 => BusMode::ExternalSplitMpm,
            }
        }
    }

    fn apply_route_config(
        &mut self,
        enabled: bool,
        cfg: ferrox_core::hal::SerialRouteConfig,
    ) {
        use ferrox_core::hal::SerialDuplex;

        self.last_baud_idx = cfg.baud_idx;
        self.last_half_duplex = matches!(cfg.duplex, SerialDuplex::HalfDuplexSport);
        self.route = u8::from(cfg.external_bay);
        self.tx_inverted = cfg.tx_inverted;
        self.framing = cfg.framing;
        self.last_cfg = Some(cfg);

        let usart1 = unsafe { &*pac::USART1::ptr() };
        let usart2 = unsafe { &*pac::USART2::ptr() };
        let usart6 = unsafe { &*pac::USART6::ptr() };
        let tim8 = unsafe { &*pac::TIM8::ptr() };
        let dma2 = unsafe { &*pac::DMA2::ptr() };

        // Disable all 3 USARTs, TIM8 soft-serial, and RX/TX DMA streams before reconfiguring
        unsafe {
            Self::stop_tim8_softserial();
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
        let (brr_apb2, brr_apb1) = match cfg.baud_idx {
            0 => (729, 365), // 115200 baud
            1 => (210, 105), // 400000 baud
            2 => (200, 100), // 420000 baud
            3 => (91, 46),   // 921600 baud
            4 => (45, 22),   // 1875000 baud
            5 => (840, 420), // 100000 baud (MPM / CC2500)
            6 => (729, 365), // 115200 baud (DSM Satellite)
            _ => (210, 105), // fallback 400k
        };
        let (cr1_extra, cr2_extra) = match cfg.framing {
            SerialFraming::Format8N1 => (0u32, 0u32),
            SerialFraming::Format8E2 => (
                (1 << 12) | (1 << 10), // M=1 (9-bit), PCE=1 (even parity)
                0x02 << 12,            // STOP=2 (2 stop bits)
            ),
        };

        let mode = Self::bus_mode_from_route(cfg);
        self.bus_mode = mode;

        unsafe {
            match mode {
                BusMode::InternalUsart1 => {
                    usart1.cr2.write(|w| w.bits(cr2_extra));
                    let hdsel = if self.last_half_duplex { 1 << 3 } else { 0 };
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
                    Self::set_pc6_af(8);
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
                    // - Pin 1 TX (PC6): inverted soft-serial via TIM8_CH1 (AF3) + DMA2_Stream1 Ch 7
                    //   (STM32F407 USART6 lacks hardware TXINV; EdgeTX uses TIM8_CH1 on PC6).
                    // - Pin 5 RX (PD6): normal-polarity telemetry via USART2_RX (PD4 held in RX).
                    if cfg.tx_inverted {
                        Self::set_pc6_af(3); // AF3 = TIM8_CH1

                        // TIM8 clock = 168 MHz (2 * APB2 84 MHz, RM0090 §7.2)
                        let psc: u32 = match cfg.baud_idx {
                            5 => 104, // 168 MHz / 105 = 1.6 MHz -> 16 ticks = exact 100,000 baud
                            6 => 90,  // 168 MHz / 91 = 1.846 MHz -> 16 ticks = 115,385 baud (+0.16%)
                            _ => 104,
                        };
                        tim8.cr1.write(|w| w.bits(0));
                        tim8.psc.write(|w| w.bits(psc));
                        tim8.arr.write(|w| w.bits(0xFFFF));
                        tim8.ccr1().write(|w| w.bits(0));
                        // OC1M = 0b100 (Forced Inactive -> PC6 LOW = inverted idle), OC1PE = 1
                        tim8.ccmr1_output()
                            .write(|w| w.bits((0x04 << 4) | (1 << 3)));
                        // CC1E = 1, CC1P = 0 (active HIGH, so OC1Ref=0 drives PC6 LOW)
                        tim8.ccer.write(|w| w.bits(1 << 0));
                        // MOE = 1 (Main Output Enable, required for advanced timer TIM8)
                        tim8.bdtr.write(|w| w.bits(1 << 15));
                        // Latch prescaler shadow register via UG, then clear UIF
                        tim8.egr.write(|w| w.bits(1 << 0));
                        tim8.sr.write(|w| w.bits(0));

                        // Unmask high-priority DMA2_Stream1 and TIM8_UP_TIM13 IRQs
                        let mut cp = cortex_m::peripheral::Peripherals::steal();
                        cp.NVIC.set_priority(pac::Interrupt::DMA2_STREAM1, 0x20);
                        cp.NVIC.set_priority(pac::Interrupt::TIM8_UP_TIM13, 0x20);
                        cortex_m::peripheral::NVIC::unpend(pac::Interrupt::DMA2_STREAM1);
                        cortex_m::peripheral::NVIC::unpend(pac::Interrupt::TIM8_UP_TIM13);
                        cortex_m::peripheral::NVIC::unmask(pac::Interrupt::DMA2_STREAM1);
                        cortex_m::peripheral::NVIC::unmask(pac::Interrupt::TIM8_UP_TIM13);
                    } else {
                        Self::set_pc6_af(8); // AF8 = USART6_TX
                        usart6.cr2.write(|w| w.bits(cr2_extra));
                        usart6.cr3.write(|w| w.bits(0));
                        usart6.brr.write(|w| w.bits(brr_apb2));
                        usart6
                            .cr1
                            .write(|w| w.bits((1 << 13) | (1 << 3) | cr1_extra)); // UE | TE
                    }

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

    /// Transmit `bytes` via `TIM8_CH1` + `DMA2_Stream1` (Ch 7) inverted soft-serial on `PC6`.
    fn try_write_tim8_inverted(&mut self, bytes: &[u8]) -> bool {
        if SOFT_TX_BUSY.load(Ordering::Acquire) {
            return false;
        }

        let pulse_len = unsafe {
            ferrox_core::uart::encode_softserial_pulses(bytes, self.framing, &mut TIM8_PULSE_BUF)
        };
        if pulse_len == 0 {
            return true;
        }

        SOFT_TX_BUSY.store(true, Ordering::Release);

        unsafe {
            let tim8 = &*pac::TIM8::ptr();
            let dma2 = &*pac::DMA2::ptr();
            let stream1 = &dma2.st[1];

            // Ensure DMA2 Stream 1 is disabled and flags cleared (RM0090 §10.3.17)
            stream1.cr.write(|w| w.bits(0));
            let mut timeout = 50_000u32;
            while (stream1.cr.read().bits() & 0x01) != 0 && timeout > 0 {
                timeout -= 1;
            }
            if timeout == 0 {
                SOFT_TX_BUSY.store(false, Ordering::Release);
                self.tx_errors = self.tx_errors.saturating_add(1);
                return false;
            }
            dma2.lifcr.write(|w| w.bits(0x3D << 6));

            stream1
                .par
                .write(|w| w.bits(core::ptr::addr_of!(tim8.arr) as u32));
            stream1
                .m0ar
                .write(|w| w.bits(core::ptr::addr_of!(TIM8_PULSE_BUF) as u32));
            stream1.ndtr.write(|w| w.bits(pulse_len as u32));

            // Prepare TIM8: CNT = 0xFFFF, ARR = 0xFFFF, CCR1 = 0, OC1M = TOGGLE (0b011).
            // On the first tick, CNT wraps 0xFFFF -> 0: UEV triggers DMA to load pulses[0]
            // into ARR while CNT == CCR1 == 0 toggles PC6 LOW -> HIGH (inverted Start bit).
            tim8.cr1.write(|w| w.bits(0));
            tim8.dier.write(|w| w.bits(0));
            tim8.sr.write(|w| w.bits(0));
            tim8.ccr1().write(|w| w.bits(0));
            tim8.arr.write(|w| w.bits(0xFFFF));
            tim8.cnt.write(|w| w.bits(0xFFFF));
            tim8.ccmr1_output()
                .write(|w| w.bits((0x03 << 4) | (1 << 3))); // OC1M = TOGGLE, OC1PE = 1

            // Enable TIM8 Update DMA request (UDE, bit 8)
            tim8.dier.write(|w| w.bits(1 << 8));

            // CHSEL = 7 (TIM8_UP), PL = 11 (VeryHigh), MSIZE = 01 (16b), PSIZE = 01 (16b),
            // MINC = 1, DIR = 01 (M2P), TCIE = 1, EN = 1
            stream1.cr.write(|w| {
                w.bits(
                    (7 << 25)
                        | (0x03 << 16)
                        | (0x01 << 13)
                        | (0x01 << 11)
                        | (1 << 10)
                        | (0x01 << 6)
                        | (1 << 4)
                        | (1 << 0),
                )
            });

            // Start TIM8 counter
            tim8.cr1.write(|w| w.bits(1 << 0));
        }

        true
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

            // Enable TIM8, USART1, USART6 (APB2 = 84 MHz; TIM8 clk = 168 MHz) and USART2 (APB1 = 42 MHz)
            rcc.apb2enr.modify(|r, w| {
                w.bits(
                    r.bits()
                        | (1 << 1) // TIM8
                        | (1 << 4) // USART1
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

            // Configure USART1 GPIOs: PB6 (TX), PB7 (RX, pull-up) -> AF7, Very High Speed
            gpiob.moder.modify(|r, w| {
                w.bits((r.bits() & !((0x03 << 12) | (0x03 << 14))) | (0x02 << 12) | (0x02 << 14))
            });
            gpiob
                .ospeedr
                .modify(|r, w| w.bits(r.bits() | (0x03 << 12) | (0x03 << 14)));
            gpiob
                .pupdr
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 14)) | (0x01 << 14)));
            gpiob.afrl.modify(|r, w| {
                let mask = (0x0F << 24) | (0x0F << 28);
                let af7 = (0x07 << 24) | (0x07 << 28);
                w.bits((r.bits() & !mask) | af7)
            });

            // Configure USART6/TIM8 GPIOs: PC6 (TX, AF8 or AF3), PC7 (RX, pull-up) -> Very High Speed
            gpioc.moder.modify(|r, w| {
                w.bits((r.bits() & !((0x03 << 12) | (0x03 << 14))) | (0x02 << 12) | (0x02 << 14))
            });
            gpioc
                .ospeedr
                .modify(|r, w| w.bits(r.bits() | (0x03 << 12) | (0x03 << 14)));
            gpioc
                .pupdr
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 14)) | (0x01 << 14)));
            gpioc.afrl.modify(|r, w| {
                let mask = (0x0F << 24) | (0x0F << 28);
                let af8 = (0x08 << 24) | (0x08 << 28);
                w.bits((r.bits() & !mask) | af8)
            });

            // Configure USART2 GPIOs: PD5 (TX), PD6 (RX, pull-up) -> AF7, Very High Speed
            gpiod.moder.modify(|r, w| {
                w.bits((r.bits() & !((0x03 << 10) | (0x03 << 12))) | (0x02 << 10) | (0x02 << 12))
            });
            gpiod
                .ospeedr
                .modify(|r, w| w.bits(r.bits() | (0x03 << 10) | (0x03 << 12)));
            gpiod
                .pupdr
                .modify(|r, w| w.bits((r.bits() & !(0x03 << 12)) | (0x01 << 12)));
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
        use ferrox_core::hal::{SerialDuplex, SerialRouteConfig};
        let external_bay = self.route != 0;
        let framing = if baud_idx == 5 {
            SerialFraming::Format8E2
        } else {
            SerialFraming::Format8N1
        };
        let duplex = if external_bay && (baud_idx == 5 || baud_idx == 6) {
            SerialDuplex::SplitTxPin1RxPin5
        } else if half_duplex {
            SerialDuplex::HalfDuplexSport
        } else {
            SerialDuplex::FullDuplex
        };
        let tx_inverted = duplex == SerialDuplex::SplitTxPin1RxPin5;
        self.apply_route_config(
            enabled,
            SerialRouteConfig {
                external_bay,
                baud_idx,
                framing,
                duplex,
                tx_inverted,
            },
        );
    }

    fn configure_route(&mut self, enabled: bool, cfg: ferrox_core::hal::SerialRouteConfig) {
        if self.active == enabled && self.last_cfg == Some(cfg) {
            return;
        }
        let target_route = u8::from(cfg.external_bay);
        if (self.route != target_route && self.active) || !enabled {
            self.set_module_power(false);
        }
        self.apply_route_config(enabled, cfg);
        self.set_module_power(enabled);
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
        let _ = self.try_write_bytes(bytes);
    }

    fn try_write_bytes(&mut self, bytes: &[u8]) -> bool {
        if !self.active || bytes.is_empty() {
            return true;
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
                        self.tx_errors = self.tx_errors.saturating_add(1);
                        return false;
                    }
                    unsafe {
                        let dr = core::ptr::addr_of!(usart1.dr) as *mut u8;
                        core::ptr::write_volatile(dr, b);
                    }
                }
                true
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
                        self.tx_errors = self.tx_errors.saturating_add(1);
                        return false;
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
                if tc_timeout == 0 {
                    self.tx_errors = self.tx_errors.saturating_add(1);
                    false
                } else {
                    true
                }
            }
            BusMode::ExternalSplitMpm if self.tx_inverted => self.try_write_tim8_inverted(bytes),
            BusMode::ExternalFullUsart6 | BusMode::ExternalSplitMpm => {
                let usart6 = unsafe { &*pac::USART6::ptr() };
                for &b in bytes {
                    let mut timeout = 50_000;
                    while (usart6.sr.read().bits() & (1 << 7)) == 0 && timeout > 0 {
                        timeout -= 1;
                    }
                    if timeout == 0 {
                        self.tx_errors = self.tx_errors.saturating_add(1);
                        return false;
                    }
                    unsafe {
                        let dr = core::ptr::addr_of!(usart6.dr) as *mut u8;
                        core::ptr::write_volatile(dr, b);
                    }
                }
                true
            }
        }
    }

    fn tx_error_count(&self) -> u32 {
        self.tx_errors
    }

    fn rx_byte_count(&self) -> u32 {
        self.rx_bytes
    }

    fn read_byte(&mut self) -> Option<u8> {
        if !self.active {
            return None;
        }

        let byte = match self.bus_mode {
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
        };

        if byte.is_some() {
            self.rx_bytes = self.rx_bytes.wrapping_add(1);
        }
        byte
    }
}

/// DMA2 Stream 1 Transfer Complete ISR for `TIM8_CH1` inverted soft-serial TX.
/// Switches `OC1M` to `PWM1` (`CCR1 = 0` -> `OC1Ref = 0` -> `PC6 = LOW`) and arms `TIM8`
/// update IRQ so `TIM8` stops cleanly at the end of the final stop bit with zero glitch.
#[interrupt]
fn DMA2_STREAM1() {
    unsafe {
        let dma2 = &*pac::DMA2::ptr();
        if (dma2.lisr.read().bits() & (1 << 11)) != 0 {
            dma2.lifcr.write(|w| w.bits(0x3D << 6));
            let tim8 = &*pac::TIM8::ptr();
            tim8.sr.write(|w| w.bits(0));
            // Switch OC1M from TOGGLE to PWM1 (0b110) with CCR1=0 -> holds OC1Ref=0 (PC6 LOW)
            tim8.ccr1().write(|w| w.bits(0));
            tim8.ccmr1_output()
                .write(|w| w.bits((0x06 << 4) | (1 << 3)));
            // Enable UIE (bit 0) and disable UDE (bit 8)
            tim8.dier.write(|w| w.bits(1 << 0));
        }
    }
}

/// TIM8 Update ISR: halts counter and forces `OC1M = FORCED_INACTIVE` (`PC6 = LOW`) once the
/// final stop bit finishes.
#[interrupt]
fn TIM8_UP_TIM13() {
    unsafe {
        let tim8 = &*pac::TIM8::ptr();
        if (tim8.sr.read().bits() & (1 << 0)) != 0 {
            tim8.sr.write(|w| w.bits(0));
            tim8.dier.write(|w| w.bits(0));
            tim8.ccmr1_output()
                .write(|w| w.bits((0x04 << 4) | (1 << 3))); // FORCED_INACTIVE -> PC6 LOW
            tim8.cr1.write(|w| w.bits(0));
            SOFT_TX_BUSY.store(false, Ordering::Release);
        }
    }
}
