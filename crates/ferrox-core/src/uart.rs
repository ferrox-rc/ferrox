//! Platform-agnostic serial port interface and pure, host-tested RF UART route resolver.

use crate::board::RfProtocol;
use crate::hal::{SerialDuplex, SerialFraming, SerialPort, SerialRouteConfig};
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static mut SERIAL_PORT: Option<&'static mut dyn SerialPort> = None;
static HALF_DUPLEX_ACTIVE: AtomicBool = AtomicBool::new(false);
static LAST_ROUTE_EXTERNAL: AtomicBool = AtomicBool::new(false);
static LAST_ROUTE_BAUD: AtomicU8 = AtomicU8::new(2);
static LAST_ROUTE_FRAMING: AtomicU8 = AtomicU8::new(0);
static LAST_ROUTE_DUPLEX: AtomicU8 = AtomicU8::new(0);

/// Baud rate index for MULTI-Module / CC2500 (100,000 baud 8E2).
pub const BAUD_IDX_MULTI_100K_8E2: u8 = 5;
/// Baud rate index for Spektrum Satellite / DSM (115,200 baud 8N1).
pub const BAUD_IDX_DSM_SAT_115K_8N1: u8 = 6;

/// Pure, host-tested RF UART route resolver (`CRAP <= 5`).
///
/// Maps `(RfProtocol, external_bay, crsf_half_duplex, crsf_baud_idx)` into an explicit
/// [`SerialRouteConfig`] so MCU HAL drivers (`ferrox-stm32f4/src/uart.rs`) contain zero
/// implicit `baud_idx` policy heuristics.
pub const fn resolve_serial_route(
    protocol: RfProtocol,
    external_bay: bool,
    crsf_half_duplex: bool,
    crsf_baud_idx: u8,
) -> SerialRouteConfig {
    match protocol {
        RfProtocol::Cc2500 | RfProtocol::Mpm => SerialRouteConfig {
            external_bay,
            baud_idx: BAUD_IDX_MULTI_100K_8E2,
            framing: SerialFraming::Format8E2,
            duplex: if external_bay {
                SerialDuplex::SplitTxPin1RxPin5
            } else {
                SerialDuplex::FullDuplex
            },
        },
        RfProtocol::Dsm => SerialRouteConfig {
            external_bay,
            baud_idx: BAUD_IDX_DSM_SAT_115K_8N1,
            framing: SerialFraming::Format8N1,
            duplex: if external_bay {
                SerialDuplex::SplitTxPin1RxPin5
            } else {
                SerialDuplex::FullDuplex
            },
        },
        RfProtocol::Crsf | RfProtocol::Afhds2a => SerialRouteConfig {
            external_bay,
            baud_idx: if crsf_baud_idx <= 4 { crsf_baud_idx } else { 2 },
            framing: SerialFraming::Format8N1,
            duplex: if external_bay && crsf_half_duplex {
                SerialDuplex::HalfDuplexSport
            } else {
                SerialDuplex::FullDuplex
            },
        },
    }
}

/// Register platform-specific serial port driver for RF modules.
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

pub fn set_port_route(route: u8) {
    LAST_ROUTE_EXTERNAL.store(route != 0, Ordering::Relaxed);
    if let Some(ref mut p) = unsafe { SERIAL_PORT.as_mut() } {
        p.set_port_route(route);
    }
}

pub fn set_uart_enabled(enabled: bool, baud_idx: u8, half_duplex: bool) {
    HALF_DUPLEX_ACTIVE.store(half_duplex, Ordering::Relaxed);
    LAST_ROUTE_BAUD.store(baud_idx, Ordering::Relaxed);
    if let Some(ref mut p) = unsafe { SERIAL_PORT.as_mut() } {
        p.set_uart_enabled(enabled, baud_idx, half_duplex);
    }
}

/// Apply an explicit [`SerialRouteConfig`] atomically and record it for on-radio RF diagnostics.
pub fn configure_route(enabled: bool, cfg: SerialRouteConfig) {
    HALF_DUPLEX_ACTIVE.store(cfg.duplex == SerialDuplex::HalfDuplexSport, Ordering::Relaxed);
    LAST_ROUTE_EXTERNAL.store(cfg.external_bay, Ordering::Relaxed);
    LAST_ROUTE_BAUD.store(cfg.baud_idx, Ordering::Relaxed);
    LAST_ROUTE_FRAMING.store(
        match cfg.framing {
            SerialFraming::Format8N1 => 0,
            SerialFraming::Format8E2 => 1,
        },
        Ordering::Relaxed,
    );
    LAST_ROUTE_DUPLEX.store(
        match cfg.duplex {
            SerialDuplex::FullDuplex => 0,
            SerialDuplex::HalfDuplexSport => 1,
            SerialDuplex::SplitTxPin1RxPin5 => 2,
        },
        Ordering::Relaxed,
    );
    if let Some(ref mut p) = unsafe { SERIAL_PORT.as_mut() } {
        p.configure_route(enabled, cfg);
    }
}

/// Return the currently configured [`SerialRouteConfig`] for diagnostics display.
pub fn active_route() -> SerialRouteConfig {
    let framing = match LAST_ROUTE_FRAMING.load(Ordering::Relaxed) {
        1 => SerialFraming::Format8E2,
        _ => SerialFraming::Format8N1,
    };
    let duplex = match LAST_ROUTE_DUPLEX.load(Ordering::Relaxed) {
        1 => SerialDuplex::HalfDuplexSport,
        2 => SerialDuplex::SplitTxPin1RxPin5,
        _ => SerialDuplex::FullDuplex,
    };
    SerialRouteConfig {
        external_bay: LAST_ROUTE_EXTERNAL.load(Ordering::Relaxed),
        baud_idx: LAST_ROUTE_BAUD.load(Ordering::Relaxed),
        framing,
        duplex,
    }
}

pub fn is_half_duplex() -> bool {
    HALF_DUPLEX_ACTIVE.load(Ordering::Relaxed)
}

pub fn tx_error_count() -> u32 {
    if let Some(p) = unsafe { SERIAL_PORT.as_ref() } {
        p.tx_error_count()
    } else {
        0
    }
}

pub fn rx_byte_count() -> u32 {
    if let Some(p) = unsafe { SERIAL_PORT.as_ref() } {
        p.rx_byte_count()
    } else {
        0
    }
}

pub fn get_brr_for_baud(baud_idx: u8) -> u16 {
    match baud_idx {
        0 => 417, // 115,200 baud (CRSF / ELRS @ 48MHz APB)
        1 => 115, // 400,000 baud
        2 => 114, // 416,666 baud
        3 => 52,  // 921,600 baud
        4 => 26,  // 1,843,200 baud
        5 => 480, // 100,000 baud (MULTI-Module / CC2500 8E2)
        6 => 417, // 115,200 baud (Spektrum Satellite / DSM 8N1)
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
    use std::sync::Mutex;
    use std::vec::Vec;

    static RX_QUEUE: Mutex<Vec<u8>> = Mutex::new(Vec::new());
    static TX_LOG: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());
    static TX_BLOCKED: Mutex<bool> = Mutex::new(false);

    pub fn set_tx_blocked(blocked: bool) {
        *TX_BLOCKED.lock().unwrap() = blocked;
    }

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
        RX_QUEUE.lock().unwrap().clear();
        TX_LOG.lock().unwrap().clear();
        *TX_BLOCKED.lock().unwrap() = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_serial_route_exhaustive_matrix() {
        // Internal CRSF -> FullDuplex 8N1 at selected baud
        let int_crsf = resolve_serial_route(RfProtocol::Crsf, false, true, 3);
        assert_eq!(
            int_crsf,
            SerialRouteConfig {
                external_bay: false,
                baud_idx: 3,
                framing: SerialFraming::Format8N1,
                duplex: SerialDuplex::FullDuplex,
            }
        );

        // External CRSF Half-Duplex (S.Port Pin 5) -> HalfDuplexSport 8N1
        let ext_crsf_hd = resolve_serial_route(RfProtocol::Crsf, true, true, 2);
        assert_eq!(
            ext_crsf_hd,
            SerialRouteConfig {
                external_bay: true,
                baud_idx: 2,
                framing: SerialFraming::Format8N1,
                duplex: SerialDuplex::HalfDuplexSport,
            }
        );

        // External CRSF Full-Duplex (Pins 1 & 2) -> FullDuplex 8N1
        let ext_crsf_fd = resolve_serial_route(RfProtocol::Crsf, true, false, 4);
        assert_eq!(
            ext_crsf_fd,
            SerialRouteConfig {
                external_bay: true,
                baud_idx: 4,
                framing: SerialFraming::Format8N1,
                duplex: SerialDuplex::FullDuplex,
            }
        );

        // Internal CC2500 & MPM -> FullDuplex 100k 8E2
        for proto in [RfProtocol::Cc2500, RfProtocol::Mpm] {
            let int_multi = resolve_serial_route(proto, false, false, 2);
            assert_eq!(
                int_multi,
                SerialRouteConfig {
                    external_bay: false,
                    baud_idx: BAUD_IDX_MULTI_100K_8E2,
                    framing: SerialFraming::Format8E2,
                    duplex: SerialDuplex::FullDuplex,
                }
            );

            // External CC2500 & MPM -> SplitTxPin1RxPin5 100k 8E2
            let ext_multi = resolve_serial_route(proto, true, false, 2);
            assert_eq!(
                ext_multi,
                SerialRouteConfig {
                    external_bay: true,
                    baud_idx: BAUD_IDX_MULTI_100K_8E2,
                    framing: SerialFraming::Format8E2,
                    duplex: SerialDuplex::SplitTxPin1RxPin5,
                }
            );
        }

        // Internal & External DSM -> 115.2k 8N1
        let int_dsm = resolve_serial_route(RfProtocol::Dsm, false, false, 2);
        assert_eq!(int_dsm.duplex, SerialDuplex::FullDuplex);
        assert_eq!(int_dsm.baud_idx, BAUD_IDX_DSM_SAT_115K_8N1);
        assert_eq!(int_dsm.framing, SerialFraming::Format8N1);

        let ext_dsm = resolve_serial_route(RfProtocol::Dsm, true, false, 2);
        assert_eq!(ext_dsm.duplex, SerialDuplex::SplitTxPin1RxPin5);
        assert_eq!(ext_dsm.baud_idx, BAUD_IDX_DSM_SAT_115K_8N1);
        assert_eq!(ext_dsm.framing, SerialFraming::Format8N1);
    }
}
