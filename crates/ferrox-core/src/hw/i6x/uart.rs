//! Hardware USART2 driver and PC13 power switch control for CRSF / ExpressLRS.
//!
//! Pin mapping:
//! - PD5:  USART2_TX (AF0, Push-Pull, High Speed; bi-directional half-duplex in single-wire mode)
//! - PA15: USART2_RX (AF1, Pull-Up, High Speed in full-duplex mode)
//! - PC13: Module Power Switch (GPIO Output, Push-Pull, High = Power ON)
//!
//! DMA mapping (via SYSCFG_CFGR1_USART2_DMA_RMP remap):
//! - USART2_RX: DMA1 Channel 6 (circular 256-byte ring buffer)
//! - USART2_TX: DMA1 Channel 7 (linear 64-byte buffer)

#![allow(dead_code)]

#[cfg(not(test))]
use stm32f0xx_hal::pac::{self, interrupt};

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[cfg(not(test))]
const RX_BUF_SIZE: usize = 256;
#[cfg(not(test))]
const TX_BUF_SIZE: usize = 64;

#[cfg(not(test))]
const DMA_CCR_EN: u32 = 1 << 0;
#[cfg(not(test))]
const DMA_CCR_DIR: u32 = 1 << 4;
#[cfg(not(test))]
const DMA_CCR_CIRC: u32 = 1 << 5;
#[cfg(not(test))]
const DMA_CCR_MINC: u32 = 1 << 7;

#[cfg(not(test))]
static mut RX_DMA_BUF: [u8; RX_BUF_SIZE] = [0; RX_BUF_SIZE];
#[cfg(not(test))]
static mut TX_DMA_BUF: [u8; TX_BUF_SIZE] = [0; TX_BUF_SIZE];
#[cfg(not(test))]
static RX_TAIL: AtomicUsize = AtomicUsize::new(0);
#[cfg(not(test))]
static TX_BUSY: AtomicBool = AtomicBool::new(false);

static ACTIVE_HIGH: AtomicBool = AtomicBool::new(true);
static POWER_ON: AtomicBool = AtomicBool::new(false);
static HALF_DUPLEX_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Apply current PC13 pin state based on power state and active polarity.
#[cfg(not(test))]
unsafe fn apply_power_pin() {
    let pin_high = if ACTIVE_HIGH.load(Ordering::Relaxed) {
        POWER_ON.load(Ordering::Relaxed)
    } else {
        !POWER_ON.load(Ordering::Relaxed)
    };
    let gpioc = &*pac::GPIOC::ptr();
    if pin_high {
        gpioc.bsrr.write(|w| w.bs13().set_bit()); // High
    } else {
        gpioc.bsrr.write(|w| w.br13().set_bit()); // Low
    }
}

#[cfg(test)]
unsafe fn apply_power_pin() {}

/// Initialize GPIO pins: PC13 as power switch (initially OFF), PD5 and PA15 peripheral clocks.
pub fn init(active_high: bool) {
    #[cfg(not(test))]
    unsafe {
        ACTIVE_HIGH.store(active_high, Ordering::Relaxed);
        POWER_ON.store(false, Ordering::Relaxed);

        // Enable GPIOA, GPIOC, GPIOD clocks in RCC_AHBENR
        let rcc = &*pac::RCC::ptr();
        rcc.ahbenr
            .modify(|_, w| w.iopaen().set_bit().iopcen().set_bit().iopden().set_bit());

        // Configure PC13 as general purpose output
        let gpioc = &*pac::GPIOC::ptr();
        gpioc.moder.modify(|_, w| w.moder13().output());

        // Apply initial OFF state according to polarity
        apply_power_pin();
    }
    #[cfg(test)]
    let _ = active_high;
}

/// Set active power polarity for PC13: true = Active HIGH (N-type), false = Active LOW (P-type).
pub fn set_power_polarity(active_high: bool) {
    #[cfg(not(test))]
    unsafe {
        ACTIVE_HIGH.store(active_high, Ordering::Relaxed);
        apply_power_pin();
    }
    #[cfg(test)]
    let _ = active_high;
}

/// Set external module power state via PC13
pub fn set_module_power(power_on: bool) {
    #[cfg(not(test))]
    unsafe {
        POWER_ON.store(power_on, Ordering::Relaxed);
        apply_power_pin();
    }
    #[cfg(test)]
    let _ = power_on;
}

/// Baud rate divisors for 48.000 MHz clock tree
pub fn get_brr_for_baud(baud_idx: u8) -> u32 {
    match baud_idx {
        0 => 417, // 115,200 bps (48,000,000 / 115,200 = 416.66)
        1 => 115, // 416,666 bps (48,000,000 / 416,666.67 = 115.20)
        2 => 114, // 420,000 bps (48,000,000 / 420,000 = 114.285)
        3 => 52,  // 921,600 bps (48,000,000 / 921,600 = 52.08)
        4 => 26,  // 1,875,000 bps (48,000,000 / 1,875,000 = 25.6 -> 26 = 1,846,154 bps, 1.5% error)
        _ => 114, // default 420k (ELRS)
    }
}

/// Returns true if half-duplex (1-wire) mode is currently active.
pub fn is_half_duplex() -> bool {
    HALF_DUPLEX_ACTIVE.load(Ordering::Relaxed)
}

/// Write position of RX circular DMA buffer.
#[cfg(not(test))]
fn rx_head() -> usize {
    unsafe {
        let ndtr = (*pac::DMA1::ptr()).ch6.ndtr.read().ndt().bits() as usize;
        (RX_BUF_SIZE.wrapping_sub(ndtr)) & (RX_BUF_SIZE - 1)
    }
}

/// Configure and enable/disable USART2 with DMA
pub fn set_uart_enabled(enabled: bool, baud_idx: u8, half_duplex: bool) {
    HALF_DUPLEX_ACTIVE.store(half_duplex, Ordering::Relaxed);

    #[cfg(not(test))]
    unsafe {
        let rcc = &*pac::RCC::ptr();
        let gpiod = &*pac::GPIOD::ptr();
        let gpioa = &*pac::GPIOA::ptr();
        let usart2 = &*pac::USART2::ptr();
        let dma = &*pac::DMA1::ptr();

        if enabled {
            // 1. Enable USART2, SYSCFG, GPIOA, GPIOC, GPIOD, and DMA1 clocks
            rcc.apb1enr.modify(|_, w| w.usart2en().set_bit());
            rcc.apb2enr.modify(|_, w| w.syscfgen().set_bit());
            rcc.ahbenr.modify(|_, w| {
                w.iopaen()
                    .set_bit()
                    .iopcen()
                    .set_bit()
                    .iopden()
                    .set_bit()
                    .dmaen()
                    .set_bit()
            });

            // Remap USART2 DMA to Channel 6 (RX) and Channel 7 (TX) in SYSCFG_CFGR1
            let syscfg = &*pac::SYSCFG::ptr();
            syscfg.cfgr1.modify(|r, w| w.bits(r.bits() | (1 << 9)));

            // 2. Configure PD5 as AF0 (USART2_TX): MODER=10, AFRL bit 20..23 = 0000
            gpiod.moder.modify(|_, w| w.moder5().alternate());
            gpiod.ospeedr.modify(|_, w| w.ospeedr5().high_speed());
            gpiod.afrl.modify(|_, w| w.afrl5().af0());

            if half_duplex {
                // Half-duplex single-wire: PD5 Open-Drain with internal Pull-Up
                gpiod.otyper.modify(|_, w| w.ot5().open_drain());
                gpiod.pupdr.modify(|_, w| w.pupdr5().pull_up());

                // PA15 is not used in half-duplex; float as input
                gpioa.moder.modify(|_, w| w.moder15().input());
            } else {
                // Full-duplex: PD5 Push-Pull, no pull resistor
                gpiod.otyper.modify(|_, w| w.ot5().push_pull());
                gpiod.pupdr.modify(|_, w| w.pupdr5().floating());

                // Configure PA15 as AF1 (USART2_RX): MODER=10, AFRH bit 28..31 = 0001, Pull-Up
                gpioa.moder.modify(|_, w| w.moder15().alternate());
                gpioa.pupdr.modify(|_, w| w.pupdr15().pull_up());
                gpioa.afrh.modify(|_, w| w.afrh15().af1());
            }

            // 4. Reset USART2 registers
            usart2.cr1.reset();
            usart2.cr2.reset();

            // CR3: DMAR | DMAT | (if half_duplex HDSEL)
            // HDSEL = bit 3 (0x08), DMAR = bit 6 (0x40), DMAT = bit 7 (0x80)
            let cr3_val = if half_duplex { 0x08 | 0x40 | 0x80 } else { 0x40 | 0x80 };
            usart2.cr3.write(|w| w.bits(cr3_val));

            // 5. Set Baud rate divisor
            let brr = get_brr_for_baud(baud_idx);
            usart2.brr.write(|w| w.brr().bits(brr as u16));

            // Clear any pending error/status flags
            usart2.icr.write(|w| w.bits(0xFFFF_FFFF));

            // 6. Setup RX DMA (DMA1 Channel 6)
            dma.ch6.cr.write(|w| w.bits(0));
            dma.ch6
                .par
                .write(|w| w.bits(core::ptr::addr_of!((*pac::USART2::ptr()).rdr) as u32));
            dma.ch6
                .mar
                .write(|w| w.bits(core::ptr::addr_of!(RX_DMA_BUF) as *const u8 as u32));
            dma.ch6.ndtr.write(|w| w.ndt().bits(RX_BUF_SIZE as u16));
            dma.ch6
                .cr
                .write(|w| w.bits(DMA_CCR_MINC | DMA_CCR_CIRC | DMA_CCR_EN));

            // Reset RX ring buffer tail to current head
            RX_TAIL.store(rx_head(), Ordering::Relaxed);

            // 7. Setup TX DMA (DMA1 Channel 7)
            dma.ch7.cr.write(|w| w.bits(0));
            dma.ch7
                .par
                .write(|w| w.bits(core::ptr::addr_of!((*pac::USART2::ptr()).tdr) as u32));
            TX_BUSY.store(false, Ordering::Relaxed);

            // 8. Configure NVIC for USART2 (IRQ 28)
            cortex_m::peripheral::NVIC::unpend(pac::Interrupt::USART2);
            let mut cp = cortex_m::peripheral::Peripherals::steal();
            cp.NVIC.set_priority(pac::Interrupt::USART2, 0x40);
            cortex_m::peripheral::NVIC::unmask(pac::Interrupt::USART2);

            // 9. Enable UE, TE, RE, IDLEIE, TCIE
            // UE = bit 0 (0x01), RE = bit 2 (0x04), TE = bit 3 (0x08), IDLEIE = bit 4 (0x10), TCIE = bit 6 (0x40)
            usart2.cr1.write(|w| w.bits(0x01 | 0x04 | 0x08 | 0x10 | 0x40));
        } else {
            // Mask USART2 in NVIC
            cortex_m::peripheral::NVIC::mask(pac::Interrupt::USART2);

            // Disable DMA channels
            dma.ch6.cr.write(|w| w.bits(0));
            dma.ch7.cr.write(|w| w.bits(0));

            // Disable USART2
            usart2.cr1.reset();
            usart2.cr3.reset();

            // Reset indices & flags
            RX_TAIL.store(0, Ordering::Relaxed);
            TX_BUSY.store(false, Ordering::Relaxed);

            // Disable USART2 clock in RCC_APB1ENR
            rcc.apb1enr.modify(|_, w| w.usart2en().clear_bit());

            // Set PD5 and PA15 back to inputs
            gpiod.moder.modify(|_, w| w.moder5().input());
            gpiod.otyper.modify(|_, w| w.ot5().push_pull());
            gpioa.moder.modify(|_, w| w.moder15().input());
        }
    }
    #[cfg(test)]
    let _ = (enabled, baud_idx, half_duplex);
}

/// Transmit a buffer over USART2 using DMA.
/// In half-duplex mode, disables RE during TX to hardware-gate own echo bytes,
/// re-enabling it atomically in the TC interrupt.
pub fn write_bytes(bytes: &[u8]) -> usize {
    #[cfg(test)]
    {
        mock::record_tx(bytes);
        bytes.len()
    }
    #[cfg(not(test))]
    unsafe {
        let mut timeout = 50_000u32;
        while TX_BUSY.load(Ordering::Acquire) {
            timeout -= 1;
            if timeout == 0 {
                return 0;
            }
        }

        let n = bytes.len().min(TX_BUF_SIZE);
        if n == 0 {
            return 0;
        }

        for i in 0..n {
            TX_DMA_BUF[i] = bytes[i];
        }

        let usart2 = &*pac::USART2::ptr();
        let dma = &*pac::DMA1::ptr();

        if HALF_DUPLEX_ACTIVE.load(Ordering::Relaxed) {
            // Hardware-gate receiver off during transmission so echo bytes are never sampled
            usart2.cr1.modify(|r, w| w.bits(r.bits() & !(1 << 2))); // Clear RE (bit 2)
        }

        // Clear Transmission Complete flag before starting new transmit
        usart2.icr.write(|w| w.bits(1 << 6)); // TCCF = bit 6

        TX_BUSY.store(true, Ordering::Release);

        let ch7 = &dma.ch7;
        ch7.cr.write(|w| w.bits(0));
        ch7.mar
            .write(|w| w.bits(core::ptr::addr_of!(TX_DMA_BUF) as *const u8 as u32));
        ch7.ndtr.write(|w| w.ndt().bits(n as u16));
        ch7.cr.write(|w| w.bits(DMA_CCR_MINC | DMA_CCR_DIR | DMA_CCR_EN));

        n
    }
}

/// Read a byte from circular RX DMA ring buffer if available.
pub fn read_byte() -> Option<u8> {
    #[cfg(test)]
    {
        mock::pop_rx_byte()
    }
    #[cfg(not(test))]
    unsafe {
        let usart2 = &*pac::USART2::ptr();
        if usart2.isr.read().ore().bit_is_set() {
            usart2.icr.write(|w| w.orecf().clear());
        }

        let head = rx_head();
        let tail = RX_TAIL.load(Ordering::Relaxed);
        if tail == head {
            None
        } else {
            core::sync::atomic::compiler_fence(Ordering::SeqCst);
            let byte = RX_DMA_BUF[tail];
            core::sync::atomic::compiler_fence(Ordering::SeqCst);
            RX_TAIL.store((tail + 1) & (RX_BUF_SIZE - 1), Ordering::Release);
            Some(byte)
        }
    }
}

#[cfg(not(test))]
#[interrupt]
fn USART2() {
    unsafe {
        let usart2 = &*pac::USART2::ptr();
        let isr = usart2.isr.read();

        // Clear overrun error if present
        if isr.ore().bit_is_set() {
            usart2.icr.write(|w| w.orecf().clear());
        }

        // Transmission Complete: hardware-re-enable RE and clear busy flag
        if isr.tc().bit_is_set() {
            usart2.icr.write(|w| w.tccf().clear());
            usart2.cr1.modify(|r, w| w.bits(r.bits() | (1 << 2))); // Set RE (bit 2)
            TX_BUSY.store(false, Ordering::Release);
        }

        // Line idle flag
        if isr.idle().bit_is_set() {
            usart2.icr.write(|w| w.idlecf().clear());
        }
    }
}

#[cfg(test)]
pub mod mock {
    extern crate std;
    use std::sync::Mutex;
    use std::vec::Vec;

    static RX_QUEUE: Mutex<Vec<u8>> = Mutex::new(Vec::new());
    static TX_LOG: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());

    pub fn push_rx_bytes(bytes: &[u8]) {
        let mut q = RX_QUEUE.lock().unwrap();
        q.extend_from_slice(bytes);
    }

    pub fn pop_rx_byte() -> Option<u8> {
        let mut q = RX_QUEUE.lock().unwrap();
        if q.is_empty() {
            None
        } else {
            Some(q.remove(0))
        }
    }

    pub fn record_tx(data: &[u8]) {
        let mut log = TX_LOG.lock().unwrap();
        log.push(data.to_vec());
    }

    pub fn take_tx() -> Vec<Vec<u8>> {
        let mut log = TX_LOG.lock().unwrap();
        std::mem::take(&mut *log)
    }

    pub fn tx_count() -> usize {
        TX_LOG.lock().unwrap().len()
    }

    pub fn clear() {
        RX_QUEUE.lock().unwrap().clear();
        TX_LOG.lock().unwrap().clear();
    }
}
