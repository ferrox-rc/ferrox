//! Platform-agnostic serial port interface and pure, host-tested RF UART route resolver.

use crate::board::RfProtocol;
use crate::hal::{SerialDuplex, SerialFraming, SerialPort, SerialRouteConfig};
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static mut SERIAL_PORT: Option<&'static mut dyn SerialPort> = None;
static HALF_DUPLEX_ACTIVE: AtomicBool = AtomicBool::new(false);
static ROUTE_CONFIGURED: AtomicBool = AtomicBool::new(false);
static LAST_ROUTE_ENABLED: AtomicBool = AtomicBool::new(false);
static LAST_ROUTE_EXTERNAL: AtomicBool = AtomicBool::new(false);
static LAST_ROUTE_BAUD: AtomicU8 = AtomicU8::new(2);
static LAST_ROUTE_FRAMING: AtomicU8 = AtomicU8::new(0);
static LAST_ROUTE_DUPLEX: AtomicU8 = AtomicU8::new(0);
static LAST_ROUTE_TX_INV: AtomicBool = AtomicBool::new(false);

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
            tx_inverted: external_bay,
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
            tx_inverted: external_bay,
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
            tx_inverted: false,
        },
    }
}

/// Register platform-specific serial port driver for RF modules.
pub fn register_serial_port(port: &'static mut dyn SerialPort) {
    ROUTE_CONFIGURED.store(false, Ordering::Relaxed);
    unsafe { SERIAL_PORT = Some(port) };
}

pub fn init(active_high: bool) {
    ROUTE_CONFIGURED.store(false, Ordering::Relaxed);
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

/// Apply an explicit [`SerialRouteConfig`] atomically (only on state change) and record it
/// for on-radio RF diagnostics. Owns both UART peripheral routing and RF module power.
pub fn configure_route(enabled: bool, cfg: SerialRouteConfig) {
    let framing_code = match cfg.framing {
        SerialFraming::Format8N1 => 0u8,
        SerialFraming::Format8E2 => 1u8,
    };
    let duplex_code = match cfg.duplex {
        SerialDuplex::FullDuplex => 0u8,
        SerialDuplex::HalfDuplexSport => 1u8,
        SerialDuplex::SplitTxPin1RxPin5 => 2u8,
    };

    if ROUTE_CONFIGURED.load(Ordering::Relaxed)
        && LAST_ROUTE_ENABLED.load(Ordering::Relaxed) == enabled
        && LAST_ROUTE_EXTERNAL.load(Ordering::Relaxed) == cfg.external_bay
        && LAST_ROUTE_BAUD.load(Ordering::Relaxed) == cfg.baud_idx
        && LAST_ROUTE_FRAMING.load(Ordering::Relaxed) == framing_code
        && LAST_ROUTE_DUPLEX.load(Ordering::Relaxed) == duplex_code
        && LAST_ROUTE_TX_INV.load(Ordering::Relaxed) == cfg.tx_inverted
    {
        return;
    }

    ROUTE_CONFIGURED.store(true, Ordering::Relaxed);
    LAST_ROUTE_ENABLED.store(enabled, Ordering::Relaxed);
    HALF_DUPLEX_ACTIVE.store(cfg.duplex == SerialDuplex::HalfDuplexSport, Ordering::Relaxed);
    LAST_ROUTE_EXTERNAL.store(cfg.external_bay, Ordering::Relaxed);
    LAST_ROUTE_BAUD.store(cfg.baud_idx, Ordering::Relaxed);
    LAST_ROUTE_FRAMING.store(framing_code, Ordering::Relaxed);
    LAST_ROUTE_DUPLEX.store(duplex_code, Ordering::Relaxed);
    LAST_ROUTE_TX_INV.store(cfg.tx_inverted, Ordering::Relaxed);

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
        tx_inverted: LAST_ROUTE_TX_INV.load(Ordering::Relaxed),
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

/// Soft-serial bit duration in timer ticks (`16` ticks/bit, matching EdgeTX `stm32_softserial_driver`).
pub const SOFT_SERIAL_BITLEN: u16 = 16;

/// Encode `bytes` into timer `ARR` run-length pulse durations (`value = ticks - 1`) for
/// `OCMODE_TOGGLE` inverted soft-serial TX (e.g. STM32F4 `TIM8_CH1` on `PC6` / JR Bay Pin 1).
///
/// Every byte emits an even number of transitions starting on the Start bit (`0`, inverted `HIGH`)
/// and ending on the Stop bit(s) (`1`, inverted `LOW`), guaranteeing the line returns to idle `LOW`.
pub fn encode_softserial_pulses(bytes: &[u8], framing: SerialFraming, out: &mut [u16]) -> usize {
    let mut idx = 0usize;
    let last_i = bytes.len().saturating_sub(1);

    for (bi, &b) in bytes.iter().enumerate() {
        let mut lev = false; // Start bit = logic 0 (inverted HIGH)
        let mut len = SOFT_SERIAL_BITLEN;
        let mut parity: u8 = 0;

        for bit_i in 0..8 {
            let bit = ((b >> bit_i) & 1) != 0;
            parity ^= u8::from(bit);
            if bit == lev {
                len += SOFT_SERIAL_BITLEN;
            } else {
                if idx < out.len() {
                    out[idx] = len - 1;
                    idx += 1;
                }
                len = SOFT_SERIAL_BITLEN;
                lev = bit;
            }
        }

        if framing == SerialFraming::Format8E2 {
            let p_bit = parity != 0;
            if p_bit == lev {
                len += SOFT_SERIAL_BITLEN;
            } else {
                if idx < out.len() {
                    out[idx] = len - 1;
                    idx += 1;
                }
                len = SOFT_SERIAL_BITLEN;
                lev = p_bit;
            }
        }

        // Stop bits (logic 1, inverted LOW). Extend final 8N1 stop bit to 2 bit-times
        // so DMA TC -> TIM8 update ISR always has >= 17 us of clean idle margin.
        let stop_ticks = match framing {
            SerialFraming::Format8E2 => 2 * SOFT_SERIAL_BITLEN,
            SerialFraming::Format8N1 => {
                if bi == last_i {
                    2 * SOFT_SERIAL_BITLEN
                } else {
                    SOFT_SERIAL_BITLEN
                }
            }
        };

        if lev {
            len += stop_ticks;
        } else {
            if idx < out.len() {
                out[idx] = len - 1;
                idx += 1;
            }
            len = stop_ticks;
        }

        if idx < out.len() {
            out[idx] = len - 1;
            idx += 1;
        }
    }

    idx
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
        // Internal CRSF -> FullDuplex 8N1 normal polarity at selected baud
        let int_crsf = resolve_serial_route(RfProtocol::Crsf, false, true, 3);
        assert_eq!(
            int_crsf,
            SerialRouteConfig {
                external_bay: false,
                baud_idx: 3,
                framing: SerialFraming::Format8N1,
                duplex: SerialDuplex::FullDuplex,
                tx_inverted: false,
            }
        );

        // External CRSF Half-Duplex (S.Port Pin 5) -> HalfDuplexSport 8N1 normal polarity
        let ext_crsf_hd = resolve_serial_route(RfProtocol::Crsf, true, true, 2);
        assert_eq!(
            ext_crsf_hd,
            SerialRouteConfig {
                external_bay: true,
                baud_idx: 2,
                framing: SerialFraming::Format8N1,
                duplex: SerialDuplex::HalfDuplexSport,
                tx_inverted: false,
            }
        );

        // External CRSF Full-Duplex (Pins 1 & 2) -> FullDuplex 8N1 normal polarity
        let ext_crsf_fd = resolve_serial_route(RfProtocol::Crsf, true, false, 4);
        assert_eq!(
            ext_crsf_fd,
            SerialRouteConfig {
                external_bay: true,
                baud_idx: 4,
                framing: SerialFraming::Format8N1,
                duplex: SerialDuplex::FullDuplex,
                tx_inverted: false,
            }
        );

        // Internal CC2500 & MPM -> FullDuplex 100k 8E2 normal polarity
        for proto in [RfProtocol::Cc2500, RfProtocol::Mpm] {
            let int_multi = resolve_serial_route(proto, false, false, 2);
            assert_eq!(
                int_multi,
                SerialRouteConfig {
                    external_bay: false,
                    baud_idx: BAUD_IDX_MULTI_100K_8E2,
                    framing: SerialFraming::Format8E2,
                    duplex: SerialDuplex::FullDuplex,
                    tx_inverted: false,
                }
            );

            // External CC2500 & MPM -> SplitTxPin1RxPin5 100k 8E2 inverted TX (JR Pin 1)
            let ext_multi = resolve_serial_route(proto, true, false, 2);
            assert_eq!(
                ext_multi,
                SerialRouteConfig {
                    external_bay: true,
                    baud_idx: BAUD_IDX_MULTI_100K_8E2,
                    framing: SerialFraming::Format8E2,
                    duplex: SerialDuplex::SplitTxPin1RxPin5,
                    tx_inverted: true,
                }
            );
        }

        // Internal & External DSM -> 115.2k 8N1 (inverted TX on external bay Pin 1)
        let int_dsm = resolve_serial_route(RfProtocol::Dsm, false, false, 2);
        assert_eq!(int_dsm.duplex, SerialDuplex::FullDuplex);
        assert_eq!(int_dsm.baud_idx, BAUD_IDX_DSM_SAT_115K_8N1);
        assert_eq!(int_dsm.framing, SerialFraming::Format8N1);
        assert!(!int_dsm.tx_inverted);

        let ext_dsm = resolve_serial_route(RfProtocol::Dsm, true, false, 2);
        assert_eq!(ext_dsm.duplex, SerialDuplex::SplitTxPin1RxPin5);
        assert_eq!(ext_dsm.baud_idx, BAUD_IDX_DSM_SAT_115K_8N1);
        assert_eq!(ext_dsm.framing, SerialFraming::Format8N1);
        assert!(ext_dsm.tx_inverted);
    }

    #[test]
    fn test_encode_softserial_pulses_exhaustive_8e2_and_8n1() {
        // Simulate TIM8 OCMODE_TOGGLE starting from idle LOW (inverted UART):
        // Each ARR pulse toggles wire_high (first pulse -> HIGH = inverted start bit).
        // Decode back the inverted wire bits and verify 100% bit-exact recovery for all 0..=255.
        for framing in [SerialFraming::Format8E2, SerialFraming::Format8N1] {
            for byte in 0u8..=255u8 {
                let mut pulses = [0u16; 16];
                let count = encode_softserial_pulses(&[byte], framing, &mut pulses);
                assert!(count >= 2 && count % 2 == 0, "byte={byte:#04x} count={count}");

                let mut wire_high = false; // Idle LOW before first toggle
                let mut bits = [false; 16];
                let mut nbits = 0usize;
                for &arr in &pulses[..count] {
                    wire_high = !wire_high; // Toggle at start of each pulse
                    let ticks = arr + 1;
                    assert_eq!(ticks % SOFT_SERIAL_BITLEN, 0);
                    let run_bits = (ticks / SOFT_SERIAL_BITLEN) as usize;
                    for _ in 0..run_bits {
                        bits[nbits] = wire_high;
                        nbits += 1;
                    }
                }
                // After final pulse, wire must be at inverted stop level (LOW)
                assert!(!wire_high, "wire did not end LOW for byte={byte:#04x}");

                // Inverted UART: wire HIGH = logic 0, wire LOW = logic 1
                assert!(bits[0], "start bit must be HIGH (inverted 0)");
                let mut decoded = 0u8;
                for i in 0..8 {
                    let logic_one = !bits[1 + i];
                    if logic_one {
                        decoded |= 1 << i;
                    }
                }
                assert_eq!(decoded, byte);

                match framing {
                    SerialFraming::Format8E2 => {
                        assert_eq!(nbits, 12);
                        let parity_one = !bits[9];
                        assert_eq!(parity_one, (byte.count_ones() % 2) == 1);
                        assert!(!bits[10] && !bits[11], "two stop bits must be LOW");
                    }
                    SerialFraming::Format8N1 => {
                        assert_eq!(nbits, 11); // 1 start + 8 data + 2 trailing stop bits
                        assert!(!bits[9] && !bits[10], "stop bits must be LOW");
                    }
                }
            }
        }

        // Verify a full 27-byte worst-case MPM frame fits comfortably within 320 pulses
        let worst_case = [0x55u8; 27];
        let mut frame_pulses = [0u16; 320];
        let n = encode_softserial_pulses(&worst_case, SerialFraming::Format8E2, &mut frame_pulses);
        assert_eq!(n, 270);
    }
}
