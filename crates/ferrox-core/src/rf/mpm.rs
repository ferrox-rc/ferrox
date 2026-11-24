//! 4-in-1 Multi-Protocol Module (MPM) Catalog & Shared MULTI Serial Engine.
//!
//! Controls internal and external 4-in-1 / CC2500 MULTI-Modules via 100,000 baud `8E2` serial stream.
//! The genuine TI CC2500 protocol catalog lives separately in [`crate::rf::cc2500`]; both
//! [`Cc2500Protocol`] and [`MpmProtocol`] delegate 26/27-byte MULTI framing and bidirectional
//! telemetry decoding to this shared engine with zero duplicated state machines.

#![allow(static_mut_refs)]

use crate::mixer::NUM_CHANNELS;
pub use crate::rf::cc2500::{Cc2500Protocol, MultiProtoSpec};

/// UART baud index for 100,000 baud 8E2 (Even Parity, 2 Stop Bits) MULTI-Module stream.
pub const BAUD_IDX_MULTI_100K_8E2: u8 = crate::uart::BAUD_IDX_MULTI_100K_8E2;

/// MULTI-Module Wire Protocol IDs:
pub const MPM_PROTO_FLYSKY: u8 = 1;
pub const MPM_PROTO_HUBSAN: u8 = 2;
pub const MPM_PROTO_FRSKY_D: u8 = 3;
pub const MPM_PROTO_DSM: u8 = 6;
pub const MPM_PROTO_DEVO: u8 = 7;
pub const MPM_PROTO_SYMAX: u8 = 10;
pub const MPM_PROTO_SLT: u8 = 11;
pub const MPM_PROTO_BAYANG: u8 = 14;
pub const MPM_PROTO_FRSKY_X: u8 = 15;
pub const MPM_PROTO_SFHSS: u8 = 21;
pub const MPM_PROTO_FRSKY_V: u8 = 25;
pub const MPM_PROTO_AFHDS2A: u8 = 28;
pub const MPM_PROTO_CORONA: u8 = 36;
pub const MPM_PROTO_HITEC: u8 = 39;
pub const MPM_PROTO_REDPINE: u8 = 50;
pub const MPM_PROTO_HOTT: u8 = 57;
pub const MPM_PROTO_FRSKY_X2: u8 = 64;
pub const MPM_PROTO_RLINK: u8 = 67;
pub const MPM_PROTO_SKYARTEC: u8 = 70;
pub const MPM_PROTO_RADIOLINK: u8 = 74;
pub const MPM_PROTO_OMP: u8 = 76;
pub const MPM_PROTO_MLINK: u8 = 77;

/// Authoritative 4-in-1 MPM protocol table (25 entries across CC2500/A7105/CYRF6936/NRF24L01).
pub const MPM_PROTOCOLS: &[MultiProtoSpec] = &[
    MultiProtoSpec { wire_id: MPM_PROTO_FRSKY_X, default_sub: 0, name: "FrSkyX" },
    MultiProtoSpec { wire_id: MPM_PROTO_FRSKY_X, default_sub: 2, name: "FrSkyX LBT" },
    MultiProtoSpec { wire_id: MPM_PROTO_FRSKY_X2, default_sub: 0, name: "FrSkyX2" },
    MultiProtoSpec { wire_id: MPM_PROTO_FRSKY_X2, default_sub: 2, name: "FrSkyX2 LBT" },
    MultiProtoSpec { wire_id: MPM_PROTO_FRSKY_D, default_sub: 0, name: "FrSkyD" },
    MultiProtoSpec { wire_id: MPM_PROTO_FRSKY_V, default_sub: 0, name: "FrSkyV" },
    MultiProtoSpec { wire_id: MPM_PROTO_AFHDS2A, default_sub: 0, name: "AFHDS2A" },
    MultiProtoSpec { wire_id: MPM_PROTO_FLYSKY, default_sub: 0, name: "FlySky" },
    MultiProtoSpec { wire_id: MPM_PROTO_DSM, default_sub: 1, name: "DSMX" },
    MultiProtoSpec { wire_id: MPM_PROTO_DSM, default_sub: 0, name: "DSM2" },
    MultiProtoSpec { wire_id: MPM_PROTO_SFHSS, default_sub: 0, name: "SFHSS" },
    MultiProtoSpec { wire_id: MPM_PROTO_HOTT, default_sub: 0, name: "HoTT" },
    MultiProtoSpec { wire_id: MPM_PROTO_CORONA, default_sub: 0, name: "Corona" },
    MultiProtoSpec { wire_id: MPM_PROTO_HITEC, default_sub: 0, name: "Hitec" },
    MultiProtoSpec { wire_id: MPM_PROTO_REDPINE, default_sub: 0, name: "Redpine" },
    MultiProtoSpec { wire_id: MPM_PROTO_DEVO, default_sub: 0, name: "Devo" },
    MultiProtoSpec { wire_id: MPM_PROTO_HUBSAN, default_sub: 0, name: "Hubsan" },
    MultiProtoSpec { wire_id: MPM_PROTO_BAYANG, default_sub: 0, name: "Bayang" },
    MultiProtoSpec { wire_id: MPM_PROTO_SYMAX, default_sub: 0, name: "SymaX" },
    MultiProtoSpec { wire_id: MPM_PROTO_SLT, default_sub: 0, name: "SLT" },
    MultiProtoSpec { wire_id: MPM_PROTO_RADIOLINK, default_sub: 0, name: "RadioLink" },
    MultiProtoSpec { wire_id: MPM_PROTO_MLINK, default_sub: 0, name: "M-Link" },
    MultiProtoSpec { wire_id: MPM_PROTO_OMP, default_sub: 0, name: "OMP" },
    MultiProtoSpec { wire_id: MPM_PROTO_SKYARTEC, default_sub: 0, name: "Skyartec" },
    MultiProtoSpec { wire_id: MPM_PROTO_RLINK, default_sub: 0, name: "RLINK" },
];

/// 4-in-1 Multi-Protocol Module selector (stored in `ModelConfig::rx_serial_proto` when `RfProtocol::Mpm`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MpmProtocol(u8);

impl MpmProtocol {
    pub const COUNT: u8 = MPM_PROTOCOLS.len() as u8;

    #[inline(always)]
    pub const fn from_u8(val: u8) -> Self {
        if val < Self::COUNT {
            Self(val)
        } else {
            Self(0)
        }
    }

    #[inline(always)]
    pub const fn as_u8(self) -> u8 {
        self.0
    }

    #[inline(always)]
    pub const fn spec(self) -> &'static MultiProtoSpec {
        &MPM_PROTOCOLS[self.0 as usize]
    }

    #[inline(always)]
    pub const fn name(self) -> &'static str {
        self.spec().name
    }

    #[inline(always)]
    pub const fn mpm_proto_id(self) -> u8 {
        self.spec().wire_id
    }

    #[inline(always)]
    pub const fn default_sub_proto(self) -> u8 {
        self.spec().default_sub
    }

    #[inline(always)]
    pub const fn next(self) -> Self {
        Self((self.0 + 1) % Self::COUNT)
    }

    #[inline(always)]
    pub const fn prev(self) -> Self {
        Self((self.0 + Self::COUNT - 1) % Self::COUNT)
    }
}

/// Decoded Telemetry state from MPM / CC2500 S.Port, Hub, HoTT, or AFHDS2A stream.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct MpmTelemetry {
    pub connected: bool,
    pub has_module_status: bool,
    pub status_flags: u8,
    pub fw_ver: [u8; 4],
    pub rssi: u8,
    pub lq: u8,
    pub rx_batt_mv: u16,
    pub alt_m: i16,
    pub alt_dm: i16,
    pub vspeed_cms: i16,
    pub has_vspeed: bool,
    pub rpm: u16,
    pub temp_c_x10: i16,
    pub last_rx_batt_ms: u32,
    pub last_alt_ms: u32,
    pub last_vspd_ms: u32,
    pub last_rpm_ms: u32,
    pub last_temp_ms: u32,
    pub packets_sent: u16,
    pub packets_received: u16,
}

impl MpmTelemetry {
    pub const fn new() -> Self {
        Self {
            connected: false,
            has_module_status: false,
            status_flags: 0,
            fw_ver: [0; 4],
            rssi: 0,
            lq: 0,
            rx_batt_mv: 0,
            alt_m: 0,
            alt_dm: 0,
            vspeed_cms: 0,
            has_vspeed: false,
            rpm: 0,
            temp_c_x10: 0,
            last_rx_batt_ms: 0,
            last_alt_ms: 0,
            last_vspd_ms: 0,
            last_rpm_ms: 0,
            last_temp_ms: 0,
            packets_sent: 0,
            packets_received: 0,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum RxParseState {
    Idle,
    GotM,
    GotP,
    GotType(u8),
    Payload {
        pkt_type: u8,
        expected_len: u8,
        idx: u8,
    },
}

/// Returns `true` if `wire_id` is a FrSky D16 protocol (`FrSkyX` or `FrSkyX2`) supporting
/// Multi v1.3+ D16 bind options (Ch1-8 / Ch9-16, Telemetry ON / OFF).
#[inline(always)]
pub const fn is_d16_protocol(wire_id: u8) -> bool {
    wire_id == MPM_PROTO_FRSKY_X || wire_id == MPM_PROTO_FRSKY_X2
}

/// Human-readable label for FrSky D16 bind option (`0..=3`).
#[inline(always)]
pub const fn d16_bind_opt_name(opt: u8) -> &'static str {
    match opt & 0x03 {
        0 => "1-8 Tel ON",
        1 => "1-8 Tel OFF",
        2 => "9-16 Tel ON",
        _ => "9-16 Tel OFF",
    }
}

static mut MPM_ENABLED: bool = false;
static mut MPM_TELEM: MpmTelemetry = MpmTelemetry::new();
static mut LAST_TX_MS: u32 = 0;
static mut LAST_RX_PACKET_MS: u32 = 0;
static mut BIND_START_MS: u32 = 0;
static mut PREV_BIND_REQ: bool = false;
static mut BIND_CONFIRMED_BY_MODULE: bool = false;
static mut RX_STATE: RxParseState = RxParseState::Idle;
static mut RX_PAYLOAD: [u8; 40] = [0; 40];

/// Enable or disable MPM protocol state (UART routing and module power are owned by `uart::configure_route`).
pub fn set_enabled(enabled: bool) {
    unsafe {
        if enabled != MPM_ENABLED {
            MPM_ENABLED = enabled;
            if !enabled {
                MPM_TELEM = MpmTelemetry::new();
                LAST_TX_MS = 0;
                LAST_RX_PACKET_MS = 0;
                BIND_START_MS = 0;
                PREV_BIND_REQ = false;
                BIND_CONFIRMED_BY_MODULE = false;
                RX_STATE = RxParseState::Idle;
            }
        }
    }
}

/// Returns true if the MPM serial driver is currently active.
pub fn is_enabled() -> bool {
    unsafe { MPM_ENABLED }
}

/// Retrieve a copy of the latest MPM telemetry state.
pub fn get_telemetry() -> MpmTelemetry {
    unsafe { MPM_TELEM }
}

/// Periodic MPM/CC2500 serial frame transmission (`~111 Hz` / `9 ms` interval).
///
/// `raw_mpm_rx_flags` is `ModelConfig::mpm_rx_num` (`bits 3:0` = rx_num, `5:4` = d16_bind_opt,
/// `6` = low_power, `7` = ch8_mode); `raw_target_flags` is `ModelConfig::rf_module_target`
/// (`bit 0` = external_bay, `1` = autobind).
#[allow(clippy::too_many_arguments)]
pub fn update_link(
    now_ms: u32,
    channels_us: &[u16; NUM_CHANNELS],
    spec: &MultiProtoSpec,
    raw_mpm_rx_flags: u8,
    freq_tune: i8,
    bind_req: bool,
    range_check: bool,
    raw_target_flags: u8,
) {
    unsafe {
        if !MPM_ENABLED {
            return;
        }

        if bind_req && !PREV_BIND_REQ {
            BIND_START_MS = now_ms.max(1);
            BIND_CONFIRMED_BY_MODULE = false;
        } else if !bind_req {
            BIND_START_MS = 0;
            BIND_CONFIRMED_BY_MODULE = false;
        }
        PREV_BIND_REQ = bind_req;

        // Auto-complete bind after 10 seconds fallback so pilot doesn't stay stuck in bind mode
        let binding = if bind_req {
            if BIND_START_MS != 0 && now_ms.wrapping_sub(BIND_START_MS) >= 10_000 {
                crate::rf::set_bind_mode(false);
                BIND_START_MS = 0;
                BIND_CONFIRMED_BY_MODULE = false;
                false
            } else {
                true
            }
        } else {
            false
        };

        if now_ms.wrapping_sub(LAST_TX_MS) >= 9 {
            LAST_TX_MS = now_ms;
            let rx_num = raw_mpm_rx_flags & 0x0F;
            let d16_bind_opt = (raw_mpm_rx_flags >> 4) & 0x03;
            let low_power = (raw_mpm_rx_flags & 0x40) != 0;
            let ch8_mode = (raw_mpm_rx_flags & 0x80) != 0;
            let external_bay = (raw_target_flags & 0x01) != 0;
            let autobind = (raw_target_flags & 0x02) != 0;
            let sub_proto = if is_d16_protocol(spec.wire_id) && ch8_mode {
                spec.default_sub | 0x01
            } else {
                spec.default_sub
            };
            let mut frame = [0u8; 28];
            let len = pack_mpm_frame(
                channels_us,
                spec,
                sub_proto,
                rx_num,
                freq_tune,
                binding,
                range_check,
                external_bay,
                d16_bind_opt,
                low_power,
                autobind,
                &mut frame,
            );
            crate::uart::write_bytes(&frame[..len]);
            MPM_TELEM.packets_sent = MPM_TELEM.packets_sent.wrapping_add(1);
        }
    }
}

/// Poll incoming UART bytes and decode `MULTI_TELEMETRY` (`'M'`, `'P'`, `type`, `len`, `data...`).
pub fn poll_telemetry(now_ms: u32) {
    unsafe {
        if !MPM_ENABLED {
            return;
        }

        let mut budget = 64u8;
        while budget > 0 {
            budget -= 1;
            let Some(b) = crate::uart::read_byte() else {
                break;
            };
            feed_telemetry_byte(b, now_ms, &mut MPM_TELEM, &mut RX_STATE, &mut RX_PAYLOAD);
            if MPM_TELEM.connected {
                LAST_RX_PACKET_MS = now_ms;
            }
        }

        if LAST_RX_PACKET_MS != 0 && now_ms.wrapping_sub(LAST_RX_PACKET_MS) > 2000 {
            MPM_TELEM.connected = false;
            MPM_TELEM.rssi = 0;
            MPM_TELEM.lq = 0;
        }
    }
}

/// Feed a single byte into the `MULTI_TELEMETRY` (`MP`) state machine.
fn feed_telemetry_byte(
    b: u8,
    now_ms: u32,
    telem: &mut MpmTelemetry,
    state: &mut RxParseState,
    buf: &mut [u8; 40],
) {
    match *state {
        RxParseState::Idle => {
            if b == b'M' {
                *state = RxParseState::GotM;
            }
        }
        RxParseState::GotM => {
            if b == b'P' {
                *state = RxParseState::GotP;
            } else if b == b'M' {
                *state = RxParseState::GotM;
            } else {
                *state = RxParseState::Idle;
            }
        }
        RxParseState::GotP => {
            if (1..=0x15).contains(&b) {
                *state = RxParseState::GotType(b);
            } else {
                *state = RxParseState::Idle;
            }
        }
        RxParseState::GotType(pkt_type) => {
            if b > 0 && (b as usize) <= buf.len() {
                *state = RxParseState::Payload {
                    pkt_type,
                    expected_len: b,
                    idx: 0,
                };
            } else {
                *state = RxParseState::Idle;
            }
        }
        RxParseState::Payload {
            pkt_type,
            expected_len,
            idx,
        } => {
            buf[idx as usize] = b;
            let next_idx = idx + 1;
            if next_idx >= expected_len {
                process_mpm_packet(pkt_type, &buf[..expected_len as usize], now_ms, telem);
                *state = RxParseState::Idle;
            } else {
                *state = RxParseState::Payload {
                    pkt_type,
                    expected_len,
                    idx: next_idx,
                };
            }
        }
    }
}

/// Decode a complete `MULTI_TELEMETRY` payload by type.
pub fn process_mpm_packet(pkt_type: u8, payload: &[u8], now_ms: u32, telem: &mut MpmTelemetry) {
    match pkt_type {
        // 0x01: Multi-Module status packet (flags, major, minor, revision, patch)
        // Flags: 0x01=InputDetected, 0x02=SerialMode, 0x04=ProtoValid, 0x08=Binding
        0x01 => {
            if !payload.is_empty() {
                telem.has_module_status = true;
                telem.status_flags = payload[0];
                if payload.len() >= 5 {
                    telem.fw_ver = [payload[1], payload[2], payload[3], payload[4]];
                }
                let in_bind = (payload[0] & 0x08) != 0;
                unsafe {
                    if in_bind && BIND_START_MS != 0 && PREV_BIND_REQ {
                        BIND_CONFIRMED_BY_MODULE = true;
                    } else if !in_bind && BIND_CONFIRMED_BY_MODULE && PREV_BIND_REQ {
                        // Module confirmed bind entry and subsequently finished binding
                        crate::rf::set_bind_mode(false);
                        BIND_CONFIRMED_BY_MODULE = false;
                    }
                }
            }
        }
        // 0x02: FrSky S.Port telemetry frame (9 bytes: phys_id, prim=0x10, app_lo, app_hi, v0..v3, crc)
        0x02 => {
            if payload.len() >= 8 && payload[1] == 0x10 {
                let app_id = u16::from_le_bytes([payload[2], payload[3]]);
                let val_u32 = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
                let val_i32 = val_u32 as i32;
                telem.connected = true;
                telem.packets_received = telem.packets_received.wrapping_add(1);

                match app_id {
                    // RSSI
                    0xF101 => {
                        let rssi = (val_u32 & 0xFF).min(100) as u8;
                        telem.rssi = rssi;
                        telem.lq = rssi;
                    }
                    // RxBatt / A1 (0.05176V per count -> ~52 mV per count)
                    0xF104 | 0xF102 => {
                        let raw = val_u32 & 0xFF;
                        telem.rx_batt_mv = ((raw * 13200) / 255) as u16;
                        telem.last_rx_batt_ms = now_ms;
                    }
                    // VFAS (0.01V)
                    0x0210..=0x021F => {
                        telem.rx_batt_mv = (val_u32.saturating_mul(10)).min(65535) as u16;
                        telem.last_rx_batt_ms = now_ms;
                    }
                    // Baro Altitude (cm)
                    0x0100..=0x010F => {
                        telem.alt_dm = (val_i32 / 10).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
                        telem.alt_m = (val_i32 / 100).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
                        telem.last_alt_ms = now_ms;
                    }
                    // Vario Vertical Speed (cm/s)
                    0x0110..=0x011F => {
                        telem.vspeed_cms = val_i32.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
                        telem.has_vspeed = true;
                        telem.last_vspd_ms = now_ms;
                    }
                    // Temperature T1 (°C)
                    0x0400..=0x040F => {
                        telem.temp_c_x10 = (val_i32.saturating_mul(10))
                            .clamp(i16::MIN as i32, i16::MAX as i32)
                            as i16;
                        telem.last_temp_ms = now_ms;
                    }
                    // RPM
                    0x0500..=0x050F => {
                        telem.rpm = val_u32.min(65535) as u16;
                        telem.last_rpm_ms = now_ms;
                    }
                    _ => {}
                }
            }
        }
        // 0x03: FrSky Hub / D8 link telemetry (9 bytes: 0xFE, a1, a2, rx_rssi, tx_rssi, ...)
        0x03 => {
            if payload.len() >= 5 && payload[0] == 0xFE {
                let a1 = payload[1] as u32;
                let rx_rssi = payload[3].min(100);
                telem.connected = rx_rssi > 0;
                telem.rssi = rx_rssi;
                telem.lq = rx_rssi;
                telem.rx_batt_mv = ((a1 * 13200) / 255) as u16;
                telem.last_rx_batt_ms = now_ms;
                telem.packets_received = telem.packets_received.wrapping_add(1);
            }
        }
        // 0x06 / 0x0C: FlySky AFHDS 2A telemetry via MPM
        0x06 | 0x0C => {
            if payload.len() >= 9 {
                telem.connected = true;
                telem.packets_received = telem.packets_received.wrapping_add(1);
                // Parse 4-byte sensor tuples [type, id, lo, hi] starting after header byte
                let mut offset = 1usize;
                while offset + 4 <= payload.len() {
                    let s_type = payload[offset];
                    let raw_u16 = u16::from_le_bytes([payload[offset + 2], payload[offset + 3]]);
                    let raw_i16 = raw_u16 as i16;
                    match s_type {
                        0x00 | 0x03 => {
                            telem.rx_batt_mv = raw_u16.saturating_mul(10);
                            telem.last_rx_batt_ms = now_ms;
                        }
                        0x01 => {
                            telem.temp_c_x10 = raw_i16.saturating_sub(400);
                            telem.last_temp_ms = now_ms;
                        }
                        0x02 => {
                            telem.rpm = raw_u16;
                            telem.last_rpm_ms = now_ms;
                        }
                        0x41 => {
                            telem.alt_dm = raw_i16;
                            telem.alt_m = raw_i16 / 10;
                            telem.last_alt_ms = now_ms;
                        }
                        0x42 => {
                            telem.vspeed_cms = raw_i16;
                            telem.has_vspeed = true;
                            telem.last_vspd_ms = now_ms;
                        }
                        0xFC | 0xFE => {
                            let lq = (raw_u16 & 0xFF).min(100) as u8;
                            telem.rssi = lq;
                            telem.lq = lq;
                        }
                        _ => {}
                    }
                    offset += 4;
                }
            }
        }
        // 0x07 / 0x0E: Graupner HoTT telemetry via MPM
        0x07 | 0x0E if payload.len() >= 6 => {
            telem.connected = true;
            telem.packets_received = telem.packets_received.wrapping_add(1);
            // MPM HoTT packet: [rssi/lqi, rx_volt (0.1V), temp, alt_lo, alt_hi, vario_lo, vario_hi]
            let rssi = payload[0].min(100);
            telem.rssi = rssi;
            telem.lq = rssi;
            if payload[1] > 0 {
                telem.rx_batt_mv = (payload[1] as u16) * 100;
                telem.last_rx_batt_ms = now_ms;
            }
            if payload.len() >= 5 {
                let raw_alt = u16::from_le_bytes([payload[3], payload[4]]) as i32;
                if raw_alt > 0 {
                    let alt_m = (raw_alt - 500).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
                    telem.alt_m = alt_m;
                    telem.alt_dm = alt_m.saturating_mul(10);
                    telem.last_alt_ms = now_ms;
                }
            }
            if payload.len() >= 7 {
                let raw_vspd = u16::from_le_bytes([payload[5], payload[6]]) as i32;
                if raw_vspd > 0 {
                    telem.vspeed_cms =
                        (raw_vspd - 30000).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
                    telem.has_vspeed = true;
                    telem.last_vspd_ms = now_ms;
                }
            }
        }
        _ => {}
    }
}

/// Format a 27- or 28-byte Multi v1.3+ CC2500/4-in-1 serial frame (100,000 baud 8E2):
/// - Byte 0: `0x55` (`proto_id & 0x20 == 0`) or `0x54` (`proto_id & 0x20 != 0`)
/// - Byte 1: `(bind << 7) | (auto_bind << 6) | (range_check << 5) | (proto_id & 0x1F)`
/// - Byte 2: `(low_power << 7) | ((sub_proto & 0x07) << 4) | (rx_num & 0x0F)`
/// - Byte 3: `option` (`-127..+127`, frequency fine tuning)
/// - Bytes 4..26: 16 channels packed as 11-bit values (`0..2047`, 22 bytes total)
/// - Byte 26 (Multi v1.3+): `(proto_id & 0xC0) | (rx_num & 0x30) | (external_bay << 3)`
/// - Byte 27 (optional, when binding FrSky D16 `FrSkyX`/`FrSkyX2`): `d16_bind_opt & 0x03`
///   (`0` = Ch1-8 Tel ON, `1` = Ch1-8 Tel OFF, `2` = Ch9-16 Tel ON, `3` = Ch9-16 Tel OFF).
#[allow(clippy::too_many_arguments)]
pub fn pack_mpm_frame(
    channels_us: &[u16; NUM_CHANNELS],
    spec: &MultiProtoSpec,
    sub_proto: u8,
    rx_num: u8,
    freq_tune: i8,
    binding: bool,
    range_check: bool,
    external_bay: bool,
    d16_bind_opt: u8,
    low_power: bool,
    autobind: bool,
    out: &mut [u8; 28],
) -> usize {
    let proto_id = spec.wire_id;
    // MULTI-Module uses 0x55 for bit5==0 (0..31, 64..95) and 0x54 for bit5==1 (32..63, 96..127).
    out[0] = if (proto_id & 0x20) != 0 { 0x54 } else { 0x55 };

    let mut b1 = proto_id & 0x1F;
    if binding {
        b1 |= 0x80;
    }
    if autobind && proto_id != MPM_PROTO_DSM {
        b1 |= 0x40;
    }
    if range_check {
        b1 |= 0x20;
    }
    out[1] = b1;

    let mut b2 = ((sub_proto & 0x07) << 4) | (rx_num & 0x0F);
    if low_power {
        b2 |= 0x80;
    }
    out[2] = b2;
    out[3] = freq_tune as u8;

    // Pack 16 channels as 11-bit values (988..2012 us -> 172..1811)
    let mut bit_acc: u32 = 0;
    let mut bits_in_acc: u8 = 0;
    let mut byte_idx = 4;

    for &us in channels_us[..16].iter() {
        let us_clamped = us.clamp(988, 2012) as u32;
        let val11 = (((us_clamped - 988) * 1639) >> 10).saturating_add(172).min(2047);
        bit_acc |= val11 << bits_in_acc;
        bits_in_acc += 11;
        while bits_in_acc >= 8 && byte_idx < 26 {
            out[byte_idx] = (bit_acc & 0xFF) as u8;
            bit_acc >>= 8;
            bits_in_acc -= 8;
            byte_idx += 1;
        }
    }

    // Multi v1.3+ byte 26 (Multiprotocol.ino: protocol |= rx_ok_buff[26] & 0xC0,
    // RX_num |= rx_ok_buff[26] & 0x30, bit 3 = invert telemetry for external bay).
    out[26] = (proto_id & 0xC0) | (rx_num & 0x30) | if external_bay { 0x08 } else { 0x00 };

    if binding && is_d16_protocol(proto_id) {
        out[27] = d16_bind_opt & 0x03;
        28
    } else {
        27
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_mpm_frame_structure() {
        let chs = [1500u16; NUM_CHANNELS];
        let mut frame = [0u8; 28];

        let len = pack_mpm_frame(
            &chs,
            Cc2500Protocol::FrSkyX.spec(),
            0,
            1,
            -15,
            false,
            false,
            false,
            0,
            false,
            false,
            &mut frame,
        );

        assert_eq!(len, 27);
        assert_eq!(frame[0], 0x55);
        assert_eq!(frame[1], MPM_PROTO_FRSKY_X);
        assert_eq!(frame[2], 0x01); // Sub 0, RxNum 1
        assert_eq!(frame[3] as i8, -15); // Frequency tuning
        assert_eq!(frame[26], 0x00); // Internal: normal telemetry, proto < 64
    }

    #[test]
    fn test_pack_mpm_frame_binding_and_range() {
        let chs = [1500u16; NUM_CHANNELS];
        let mut frame = [0u8; 28];

        // FrSkyD (D8) does not append D16 bind option byte 27
        let len = pack_mpm_frame(
            &chs,
            Cc2500Protocol::FrSkyD.spec(),
            0,
            0,
            0,
            true,
            true,
            true,
            3,
            true,
            true,
            &mut frame,
        );

        assert_eq!(len, 27);
        assert_eq!(frame[0], 0x55);
        assert_eq!(frame[1], MPM_PROTO_FRSKY_D | 0x80 | 0x40 | 0x20);
        assert_eq!(frame[2], 0x80); // Low power bit 7
        assert_eq!(frame[26], 0x08); // External bay telemetry invert bit

        // FrSkyX (D16) appends byte 27 = d16_bind_opt when binding
        let len_d16 = pack_mpm_frame(
            &chs,
            Cc2500Protocol::FrSkyX.spec(),
            1, // CH_8 sub-protocol
            1,
            0,
            true,
            false,
            true,
            2, // Ch9-16 Telem ON
            false,
            false,
            &mut frame,
        );
        assert_eq!(len_d16, 28);
        assert_eq!(frame[1], MPM_PROTO_FRSKY_X | 0x80);
        assert_eq!(frame[2], (1 << 4) | 1);
        assert_eq!(frame[26], 0x08);
        assert_eq!(frame[27], 2);
    }

    #[test]
    fn test_pack_mpm_frame_extended_proto_id() {
        let chs = [1500u16; NUM_CHANNELS];
        let mut frame = [0u8; 28];

        assert_eq!(
            pack_mpm_frame(
                &chs,
                Cc2500Protocol::Corona.spec(),
                0,
                0,
                0,
                false,
                false,
                false,
                0,
                false,
                false,
                &mut frame
            ),
            27
        );
        assert_eq!(frame[0], 0x54);
        assert_eq!(frame[1], MPM_PROTO_CORONA & 0x1F);
        assert_eq!(frame[26], 0x00);

        assert_eq!(
            pack_mpm_frame(
                &chs,
                Cc2500Protocol::Hitec.spec(),
                0,
                0,
                0,
                false,
                false,
                false,
                0,
                false,
                false,
                &mut frame
            ),
            27
        );
        assert_eq!(frame[0], 0x54);
        assert_eq!(frame[1], MPM_PROTO_HITEC & 0x1F);

        // FrSkyX2 (ID = 64 = 0x40) sets bits 7:6 of byte 26 = 0x40 (+ 0x08 external invert)
        assert_eq!(
            pack_mpm_frame(
                &chs,
                Cc2500Protocol::FrSkyX2Lbt.spec(),
                Cc2500Protocol::FrSkyX2Lbt.default_sub_proto(),
                2,
                0,
                true,
                false,
                true,
                1, // Ch1-8 Telem OFF
                false,
                false,
                &mut frame,
            ),
            28
        );
        assert_eq!(frame[0], 0x55);
        assert_eq!(frame[1], 0x80); // Bind flag + (64 & 0x1F == 0)
        assert_eq!(frame[2], (2 << 4) | 2);
        assert_eq!(frame[26], 0x40 | 0x08);
        assert_eq!(frame[27], 1);

        // Verify MpmProtocol table bounds & cycling
        assert_eq!(MpmProtocol::COUNT, 25);
        let mut m = MpmProtocol::from_u8(0);
        for _ in 0..MpmProtocol::COUNT {
            assert!(!m.name().is_empty());
            m = m.next();
        }
        assert_eq!(m.as_u8(), 0);
    }

    #[test]
    fn test_mpm_sport_and_hott_telemetry_parsing() {
        let mut telem = MpmTelemetry::new();
        let mut state = RxParseState::Idle;
        let mut buf = [0u8; 40];

        // 1. S.Port Altitude packet (0x0100 = 12340 cm -> 1234 dm, 123 m)
        let alt_cm: i32 = 12340;
        let vb = alt_cm.to_le_bytes();
        let sport_pkt = [
            b'M', b'P', 0x02, 9, 0x98, 0x10, 0x00, 0x01, vb[0], vb[1], vb[2], vb[3], 0x00,
        ];
        for &b in &sport_pkt {
            feed_telemetry_byte(b, 1000, &mut telem, &mut state, &mut buf);
        }
        assert!(telem.connected);
        assert_eq!(telem.alt_dm, 1234);
        assert_eq!(telem.alt_m, 123);

        // 2. HoTT telemetry packet (RSSI=92, RxV=5.2V (52), Temp=0, Alt=650 (150m), Vario=30120 (+120 cm/s))
        let hott_pkt = [
            b'M',
            b'P',
            0x07,
            7,
            92,
            52,
            0,
            (650u16 & 0xFF) as u8,
            (650u16 >> 8) as u8,
            (30120u16 & 0xFF) as u8,
            (30120u16 >> 8) as u8,
        ];
        for &b in &hott_pkt {
            feed_telemetry_byte(b, 1100, &mut telem, &mut state, &mut buf);
        }
        assert_eq!(telem.rssi, 92);
        assert_eq!(telem.rx_batt_mv, 5200);
        assert_eq!(telem.alt_m, 150);
        assert_eq!(telem.alt_dm, 1500);
        assert_eq!(telem.vspeed_cms, 120);
        assert!(telem.has_vspeed);
    }
}
