//! Hardware USART1 driver (PB6/PB7 on J15) and PF6 power switch control (H1) for CRSF / ExpressLRS.
//!
//! Pin mapping:
//! - PB6: USART1_TX (AF0, Push-Pull, High Speed; bi-directional half-duplex in single-wire mode)
//! - PB7: USART1_RX (AF0, Pull-Up, High Speed in full-duplex mode)
//! - PF6: Module Power Switch on H1 (GPIO Output, Push-Pull; Right pin PF6, Left pin GND)
//!
//! DMA mapping (STM32F072 default):
//! - USART1_RX: DMA1 Channel 3 (circular 256-byte ring buffer)
//! - USART1_TX: DMA1 Channel 2 (linear 64-byte buffer)

#![allow(dead_code)]

#[cfg(not(test))]
use stm32f0xx_hal::pac::{self, interrupt};

use core::sync::atomic::{AtomicBool, Ordering};
#[cfg(not(test))]
use core::sync::atomic::AtomicUsize;

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

/// Apply current PF6 pin state based on power state and active polarity.
#[cfg(not(test))]
unsafe fn apply_power_pin() {
    let pin_high = if ACTIVE_HIGH.load(Ordering::Relaxed) {
        POWER_ON.load(Ordering::Relaxed)
    } else {
        !POWER_ON.load(Ordering::Relaxed)
    };
    let gpiof = &*pac::GPIOF::ptr();
    if pin_high {
        gpiof.bsrr.write(|w| w.bs6().set_bit()); // High
    } else {
        gpiof.bsrr.write(|w| w.br6().set_bit()); // Low
    }
}

#[cfg(test)]
unsafe fn apply_power_pin() {}

/// Initialize GPIO pins: PF6 as power switch (initially OFF), PB6 and PB7 peripheral clocks.
pub fn init(active_high: bool) {
    #[cfg(not(test))]
    unsafe {
        ACTIVE_HIGH.store(active_high, Ordering::Relaxed);
        POWER_ON.store(false, Ordering::Relaxed);

        // Enable GPIOB, GPIOF clocks in RCC_AHBENR
        let rcc = &*pac::RCC::ptr();
        rcc.ahbenr
            .modify(|_, w| w.iopben().set_bit().iopfen().set_bit());

        // Configure PF6 as general purpose output
        let gpiof = &*pac::GPIOF::ptr();
        gpiof.moder.modify(|_, w| w.moder6().output());

        // Apply initial OFF state according to polarity
        apply_power_pin();
    }
    #[cfg(test)]
    let _ = active_high;
}

/// Set active power polarity for PF6 (H1): true = Active HIGH (N-type), false = Active LOW (P-type).
pub fn set_power_polarity(active_high: bool) {
    #[cfg(not(test))]
    unsafe {
        ACTIVE_HIGH.store(active_high, Ordering::Relaxed);
        apply_power_pin();
    }
    #[cfg(test)]
    let _ = active_high;
}

/// Set external module power state via PF6 (H1 right pin)
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
        let ndtr = (*pac::DMA1::ptr()).ch3.ndtr.read().ndt().bits() as usize;
        (RX_BUF_SIZE.wrapping_sub(ndtr)) & (RX_BUF_SIZE - 1)
    }
}

/// Configure and enable/disable USART1 (full-duplex or single-wire half-duplex on PB6) with DMA
pub fn set_uart_enabled(enabled: bool, baud_idx: u8, half_duplex: bool) {
    HALF_DUPLEX_ACTIVE.store(half_duplex, Ordering::Relaxed);

    #[cfg(not(test))]
    unsafe {
        let rcc = &*pac::RCC::ptr();
        let gpiob = &*pac::GPIOB::ptr();
        let usart1 = &*pac::USART1::ptr();
        let dma = &*pac::DMA1::ptr();

        if enabled {
            // 1. Enable USART1, GPIOB, GPIOF, and DMA1 clocks
            rcc.apb2enr.modify(|_, w| w.usart1en().set_bit());
            rcc.ahbenr
                .modify(|_, w| w.iopben().set_bit().iopfen().set_bit().dmaen().set_bit());

            // 2. Configure PB6 as AF0 (USART1_TX): MODER=10, OSPEEDR=11, AFRL6=0000
            gpiob.moder.modify(|_, w| w.moder6().alternate());
            gpiob.ospeedr.modify(|_, w| w.ospeedr6().high_speed());
            gpiob.afrl.modify(|_, w| w.afrl6().af0());

            if half_duplex {
                // Half-duplex single-wire on PB6: Open-Drain with internal Pull-Up
                gpiob.otyper.modify(|_, w| w.ot6().open_drain());
                gpiob.pupdr.modify(|_, w| w.pupdr6().pull_up());

                // Leave PB7 as general floating input
                gpiob.moder.modify(|_, w| w.moder7().input());
            } else {
                // Full-duplex: PB6 Push-Pull, no pull resistor
                gpiob.otyper.modify(|_, w| w.ot6().push_pull());
                gpiob.pupdr.modify(|_, w| w.pupdr6().floating());

                // 3. Configure PB7 as AF0 (USART1_RX): MODER=10, PUPDR=01 (pull-up), AFRL7=0000
                gpiob.moder.modify(|_, w| w.moder7().alternate());
                gpiob.pupdr.modify(|_, w| w.pupdr7().pull_up());
                gpiob.afrl.modify(|_, w| w.afrl7().af0());
            }

            // 4. Reset USART1 registers
            usart1.cr1.reset();
            usart1.cr2.reset();

            // CR3: DMAR | DMAT | (if half_duplex HDSEL)
            // HDSEL = bit 3 (0x08), DMAR = bit 6 (0x40), DMAT = bit 7 (0x80)
            let cr3_val = if half_duplex { 0x08 | 0x40 | 0x80 } else { 0x40 | 0x80 };
            usart1.cr3.write(|w| w.bits(cr3_val));

            // 5. Set Baud rate divisor
            let brr = get_brr_for_baud(baud_idx);
            usart1.brr.write(|w| w.brr().bits(brr as u16));

            // Clear any pending error/status flags
            usart1.icr.write(|w| w.bits(0xFFFF_FFFF));

            // 6. Setup RX DMA (DMA1 Channel 3)
            dma.ch3.cr.write(|w| w.bits(0));
            dma.ch3
                .par
                .write(|w| w.bits(core::ptr::addr_of!((*pac::USART1::ptr()).rdr) as u32));
            dma.ch3
                .mar
                .write(|w| w.bits(core::ptr::addr_of!(RX_DMA_BUF) as *const u8 as u32));
            dma.ch3.ndtr.write(|w| w.ndt().bits(RX_BUF_SIZE as u16));
            dma.ch3
                .cr
                .write(|w| w.bits(DMA_CCR_MINC | DMA_CCR_CIRC | DMA_CCR_EN));

            // Reset RX ring buffer tail to current head
            RX_TAIL.store(rx_head(), Ordering::Relaxed);

            // 7. Setup TX DMA (DMA1 Channel 2)
            dma.ch2.cr.write(|w| w.bits(0));
            dma.ch2
                .par
                .write(|w| w.bits(core::ptr::addr_of!((*pac::USART1::ptr()).tdr) as u32));
            TX_BUSY.store(false, Ordering::Relaxed);

            // 8. Configure NVIC for USART1 (IRQ 27)
            cortex_m::peripheral::NVIC::unpend(pac::Interrupt::USART1);
            let mut cp = cortex_m::peripheral::Peripherals::steal();
            cp.NVIC.set_priority(pac::Interrupt::USART1, 0x40);
            cortex_m::peripheral::NVIC::unmask(pac::Interrupt::USART1);

            // 9. Enable UE, TE, RE, IDLEIE, TCIE
            // UE = bit 0 (0x01), RE = bit 2 (0x04), TE = bit 3 (0x08), IDLEIE = bit 4 (0x10), TCIE = bit 6 (0x40)
            usart1.cr1.write(|w| w.bits(0x01 | 0x04 | 0x08 | 0x10 | 0x40));
        } else {
            // Mask USART1 in NVIC
            cortex_m::peripheral::NVIC::mask(pac::Interrupt::USART1);

            // Disable DMA channels
            dma.ch3.cr.write(|w| w.bits(0));
            dma.ch2.cr.write(|w| w.bits(0));

            // Disable USART1
            usart1.cr1.reset();
            usart1.cr3.reset();

            // Reset indices & flags
            RX_TAIL.store(0, Ordering::Relaxed);
            TX_BUSY.store(false, Ordering::Relaxed);

            // Disable USART1 clock in RCC_APB2ENR
            rcc.apb2enr.modify(|_, w| w.usart1en().clear_bit());

            // Set PB6 and PB7 back to inputs
            gpiob
                .moder
                .modify(|_, w| w.moder6().input().moder7().input());
            gpiob.otyper.modify(|_, w| w.ot6().push_pull());
        }
    }
    #[cfg(test)]
    let _ = (enabled, baud_idx, half_duplex);
}

/// Transmit a buffer over USART1 using DMA.
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

        TX_DMA_BUF[..n].copy_from_slice(&bytes[..n]);

        let usart1 = &*pac::USART1::ptr();
        let dma = &*pac::DMA1::ptr();

        if HALF_DUPLEX_ACTIVE.load(Ordering::Relaxed) {
            // Hardware-gate receiver off during transmission so echo bytes are never sampled
            usart1.cr1.modify(|r, w| w.bits(r.bits() & !(1 << 2))); // Clear RE (bit 2)
        }

        // Clear Transmission Complete flag before starting new transmit
        usart1.icr.write(|w| w.bits(1 << 6)); // TCCF = bit 6

        TX_BUSY.store(true, Ordering::Release);

        let ch2 = &dma.ch2;
        ch2.cr.write(|w| w.bits(0));
        ch2.mar
            .write(|w| w.bits(core::ptr::addr_of!(TX_DMA_BUF) as *const u8 as u32));
        ch2.ndtr.write(|w| w.ndt().bits(n as u16));
        ch2.cr.write(|w| w.bits(DMA_CCR_MINC | DMA_CCR_DIR | DMA_CCR_EN));

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
        let usart1 = &*pac::USART1::ptr();
        if usart1.isr.read().ore().bit_is_set() {
            usart1.icr.write(|w| w.orecf().clear());
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
fn USART1() {
    unsafe {
        let usart1 = &*pac::USART1::ptr();
        let isr = usart1.isr.read();

        // Clear overrun error if present
        if isr.ore().bit_is_set() {
            usart1.icr.write(|w| w.orecf().clear());
        }

        // Transmission Complete: hardware-re-enable RE and clear busy flag
        if isr.tc().bit_is_set() {
            usart1.icr.write(|w| w.tccf().clear());
            usart1.cr1.modify(|r, w| w.bits(r.bits() | (1 << 2))); // Set RE (bit 2)
            TX_BUSY.store(false, Ordering::Release);
        }

        // Line idle flag
        if isr.idle().bit_is_set() {
            usart1.icr.write(|w| w.idlecf().clear());
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
