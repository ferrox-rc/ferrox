//! Platform-agnostic serial port interface (CRSF / ExpressLRS).

use core::sync::atomic::{AtomicBool, Ordering};
use crate::hal::SerialPort;

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

pub fn get_brr_for_baud(baud_idx: u8) -> u16 {
    match baud_idx {
        0 => 417,
        1 => 115,
        2 => 114,
        3 => 52,
        4 => 26,
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

/// True only when the registered driver can accept a frame without waiting.
pub fn tx_idle() -> bool {
    #[cfg(test)]
    {
        mock::tx_idle()
    }
    #[cfg(not(test))]
    unsafe {
        SERIAL_PORT.as_ref().is_some_and(|port| port.tx_idle())
    }
}

/// Attempt an all-or-nothing, nonblocking frame transmission.
pub fn try_write_bytes(bytes: &[u8]) -> bool {
    #[cfg(test)]
    {
        mock::try_record_tx(bytes)
    }
    #[cfg(not(test))]
    unsafe {
        SERIAL_PORT
            .as_mut()
            .is_some_and(|port| port.try_write_bytes(bytes))
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

#[cfg(test)]
pub mod mock {
    extern crate std;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;
    use std::vec::Vec;

    static RX_QUEUE: Mutex<Vec<u8>> = Mutex::new(Vec::new());
    static TX_LOG: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());
    static TX_BUSY: AtomicBool = AtomicBool::new(false);
    static TX_ACCEPTING: AtomicBool = AtomicBool::new(true);

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
        TX_BUSY.store(true, Ordering::Relaxed);
    }

    pub fn tx_idle() -> bool {
        !TX_BUSY.load(Ordering::Relaxed)
    }

    pub fn complete_tx() {
        TX_BUSY.store(false, Ordering::Relaxed);
    }

    pub fn set_tx_accepting(accepting: bool) {
        TX_ACCEPTING.store(accepting, Ordering::Relaxed);
    }

    pub fn try_record_tx(data: &[u8]) -> bool {
        if data.is_empty()
            || data.len() > 64
            || !TX_ACCEPTING.load(Ordering::Relaxed)
            || TX_BUSY.swap(true, Ordering::Relaxed)
        {
            return false;
        }
        TX_LOG.lock().unwrap().push(data.to_vec());
        true
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
        TX_BUSY.store(false, Ordering::Relaxed);
        TX_ACCEPTING.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiswitch_nonblocking_send_rejects_busy_and_oversize_frames() {
        mock::clear();
        assert!(!try_write_bytes(&[]));
        assert!(!try_write_bytes(&[0; 65]));
        assert!(tx_idle());
        assert!(try_write_bytes(&[1, 2, 3]));
        assert!(!tx_idle());
        assert!(!try_write_bytes(&[4, 5]));
        assert_eq!(mock::take_tx(), [std::vec![1, 2, 3]]);
        mock::complete_tx();
        assert!(try_write_bytes(&[4, 5]));
        assert_eq!(mock::take_tx(), [std::vec![4, 5]]);
        mock::clear();
    }
}
