//! Platform-agnostic serial port interface (CRSF / ExpressLRS).

use crate::hal::SerialPort;
use core::sync::atomic::{AtomicBool, Ordering};

static mut SERIAL_PORT: Option<&'static mut dyn SerialPort> = None;
static HALF_DUPLEX_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Register platform-specific serial port driver for CRSF.
pub fn register_serial_port(port: &'static mut dyn SerialPort) {
    unsafe { SERIAL_PORT = Some(port) };
}

pub fn init(active_high: bool) {
    if let Some(ref mut p) = unsafe { SERIAL_PORT.as_mut() } {
        p.init(active_high);
    }
}

pub fn set_power_polarity(active_high: bool) {
    if let Some(ref mut p) = unsafe { SERIAL_PORT.as_mut() } {
        p.set_power_polarity(active_high);
    }
}

pub fn set_module_power(enabled: bool) {
    if let Some(ref mut p) = unsafe { SERIAL_PORT.as_mut() } {
        p.set_module_power(enabled);
    }
}

pub fn set_uart_enabled(enabled: bool, baud_idx: u8, half_duplex: bool) {
    HALF_DUPLEX_ACTIVE.store(half_duplex, Ordering::Relaxed);
    if let Some(ref mut p) = unsafe { SERIAL_PORT.as_mut() } {
        p.set_uart_enabled(enabled, baud_idx, half_duplex);
    }
}

pub fn is_half_duplex() -> bool {
    HALF_DUPLEX_ACTIVE.load(Ordering::Relaxed)
}

/// Baud rate index for MULTI-Module DSM (100,000 baud 8E2).
pub const BAUD_IDX_MULTI_100K_8E2: u8 = 5;
/// Baud rate index for Spektrum Satellite Trainer (115,200 baud 8N1).
pub const BAUD_IDX_DSM_SAT_115K_8N1: u8 = 6;

pub fn get_brr_for_baud(baud_idx: u8) -> u16 {
    match baud_idx {
        0 => 417, // 115,200 baud (CRSF / ELRS @ 48MHz APB)
        1 => 115, // 400,000 baud
        2 => 114, // 416,666 baud
        3 => 52,  // 921,600 baud
        4 => 26,  // 1,843,200 baud
        5 => 480, // 100,000 baud (MULTI-Module DSM 8E2)
        6 => 417, // 115,200 baud (Spektrum Satellite 8N1)
        _ => 115,
    }
}

pub fn write_bytes(bytes: &[u8]) {
    #[cfg(test)]
    {
        mock::record_tx(bytes);
    }
    #[cfg(not(test))]
    if let Some(ref mut p) = unsafe { SERIAL_PORT.as_mut() } {
        p.write_bytes(bytes);
    }
}

pub fn read_byte() -> Option<u8> {
    #[cfg(test)]
    {
        mock::pop_rx_byte()
    }
    #[cfg(not(test))]
    if let Some(ref mut p) = unsafe { SERIAL_PORT.as_mut() } {
        p.read_byte()
    } else {
        None
    }
}

#[cfg(feature = "multiswitch")]
pub fn try_write_bytes(bytes: &[u8]) -> bool {
    #[cfg(test)]
    {
        if mock::tx_blocked() {
            return false;
        }
        mock::record_tx(bytes);
        true
    }
    #[cfg(not(test))]
    if let Some(ref mut p) = unsafe { SERIAL_PORT.as_mut() } {
        p.try_write_bytes(bytes)
    } else {
        false
    }
}

#[cfg(test)]
pub mod mock {
    extern crate std;
    use std::sync::Mutex;
    use std::vec::Vec;

    static RX_QUEUE: Mutex<Vec<u8>> = Mutex::new(Vec::new());
    static TX_LOG: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());
    #[cfg(feature = "multiswitch")]
    static TX_BLOCKED: Mutex<bool> = Mutex::new(false);

    #[cfg(feature = "multiswitch")]
    pub fn set_tx_blocked(blocked: bool) {
        *TX_BLOCKED.lock().unwrap() = blocked;
    }

    #[cfg(feature = "multiswitch")]
    pub fn tx_blocked() -> bool {
        *TX_BLOCKED.lock().unwrap()
    }

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
        #[cfg(feature = "multiswitch")]
        set_tx_blocked(false);
        RX_QUEUE.lock().unwrap().clear();
        TX_LOG.lock().unwrap().clear();
    }
}
