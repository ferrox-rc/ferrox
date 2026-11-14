//! Multi-Protocol Module (MPM) and CC2500 Serial Protocol Driver.
//!
//! Controls internal and external 4-in-1 / CC2500 MULTI-Modules via 100,000 baud 8E2 serial stream.
//! Supports FrSky D16 (FCC & LBT), FrSky D8, Graupner HoTT, Futaba SFHSS, FlySky AFHDS 2A / V1,
//! Spektrum DSMX, Corona, and Hitec protocols with frequency fine-tuning and bidirectional telemetry decoding.

#![allow(static_mut_refs)]

use crate::mixer::NUM_CHANNELS;

/// UART baud index for 100,000 baud 8E2 (Even Parity, 2 Stop Bits) MULTI-Module stream.
pub const BAUD_IDX_MULTI_100K_8E2: u8 = 4;

/// MULTI-Module Protocol IDs:
pub const MPM_PROTO_FLYSKY: u8 = 1; // FlySky AFHDS (V1)
pub const MPM_PROTO_FRSKY_D: u8 = 3; // FrSky D8
pub const MPM_PROTO_DSM: u8 = 6; // Spektrum DSM2 / DSMX
pub const MPM_PROTO_FRSKY_X: u8 = 15; // FrSky D16 / X (FCC & LBT)
pub const MPM_PROTO_SFHSS: u8 = 21; // Futaba SFHSS
pub const MPM_PROTO_FRSKY_V: u8 = 25; // FrSky V8
pub const MPM_PROTO_AFHDS2A: u8 = 28; // FlySky AFHDS 2A
pub const MPM_PROTO_CORONA: u8 = 36; // Corona
pub const MPM_PROTO_HITEC: u8 = 39; // Hitec Optima/Minima
pub const MPM_PROTO_HOTT: u8 = 57; // Graupner HoTT

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Cc2500Protocol {
    FrSkyD16 = 0,
    FrSkyD8 = 1,
    Sfhss = 2,
    Corona = 3,
    Hitec = 4,
    FrSkyLbt = 5,
    Hott = 6,
    FlySky2a = 7,
    FlySkyStd = 8,
    Dsmx = 9,
}

impl Cc2500Protocol {
    pub const fn from_u8(val: u8) -> Self {
        match val {
            0 => Self::FrSkyD16,
            1 => Self::FrSkyD8,
            2 => Self::Sfhss,
            3 => Self::Corona,
            4 => Self::Hitec,
            5 => Self::FrSkyLbt,
            6 => Self::Hott,
            7 => Self::FlySky2a,
            8 => Self::FlySkyStd,
            9 => Self::Dsmx,
            _ => Self::FrSkyD16,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::FrSkyD16 => "FRSKY D16",
            Self::FrSkyLbt => "FRSKY LBT",
            Self::FrSkyD8 => "FRSKY D8",
            Self::Hott => "GRAUPNER HOTT",
            Self::Sfhss => "FUTABA SFHSS",
            Self::FlySky2a => "FLYSKY 2A",
            Self::FlySkyStd => "FLYSKY V1",
            Self::Dsmx => "SPEKTRUM DSMX",
            Self::Corona => "CORONA",
            Self::Hitec => "HITEC",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::FrSkyD16 => Self::FrSkyLbt,
            Self::FrSkyLbt => Self::FrSkyD8,
            Self::FrSkyD8 => Self::Hott,
            Self::Hott => Self::Sfhss,
            Self::Sfhss => Self::FlySky2a,
            Self::FlySky2a => Self::FlySkyStd,
            Self::FlySkyStd => Self::Dsmx,
            Self::Dsmx => Self::Corona,
            Self::Corona => Self::Hitec,
            Self::Hitec => Self::FrSkyD16,
        }
    }

    pub const fn prev(self) -> Self {
        match self {
            Self::FrSkyD16 => Self::Hitec,
            Self::FrSkyLbt => Self::FrSkyD16,
            Self::FrSkyD8 => Self::FrSkyLbt,
            Self::Hott => Self::FrSkyD8,
            Self::Sfhss => Self::Hott,
            Self::FlySky2a => Self::Sfhss,
            Self::FlySkyStd => Self::FlySky2a,
            Self::Dsmx => Self::FlySkyStd,
            Self::Corona => Self::Dsmx,
            Self::Hitec => Self::Corona,
        }
    }

    pub const fn mpm_proto_id(self) -> u8 {
        match self {
            Self::FrSkyD16 | Self::FrSkyLbt => MPM_PROTO_FRSKY_X,
            Self::FrSkyD8 => MPM_PROTO_FRSKY_D,
            Self::Sfhss => MPM_PROTO_SFHSS,
            Self::Corona => MPM_PROTO_CORONA,
            Self::Hitec => MPM_PROTO_HITEC,
            Self::Hott => MPM_PROTO_HOTT,
            Self::FlySky2a => MPM_PROTO_AFHDS2A,
            Self::FlySkyStd => MPM_PROTO_FLYSKY,
            Self::Dsmx => MPM_PROTO_DSM,
        }
    }

    pub const fn default_sub_proto(self) -> u8 {
        match self {
            Self::FrSkyLbt => 2, // EU_16 (LBT)
            Self::Dsmx => 1,     // DSMX_22
            _ => 0,
        }
    }
}

/// Decoded Telemetry state from MPM / CC2500 S.Port, Hub, HoTT, or AFHDS2A stream.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct MpmTelemetry {
    pub connected: bool,
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

static mut MPM_ENABLED: bool = false;
static mut MPM_TELEM: MpmTelemetry = MpmTelemetry::new();
static mut LAST_TX_MS: u32 = 0;
static mut LAST_RX_PACKET_MS: u32 = 0;
static mut BIND_START_MS: u32 = 0;
static mut PREV_BIND_REQ: bool = false;
static mut RX_STATE: RxParseState = RxParseState::Idle;
static mut RX_PAYLOAD: [u8; 40] = [0; 40];

/// Enable or disable the MPM serial driver (`100,000 baud 8E2`) and module power rail.
pub fn set_enabled(enabled: bool) {
    unsafe {
        if enabled != MPM_ENABLED {
            MPM_ENABLED = enabled;
            crate::uart::set_uart_enabled(enabled, BAUD_IDX_MULTI_100K_8E2, false);
            crate::uart::set_module_power(enabled);
            if !enabled {
                MPM_TELEM = MpmTelemetry::new();
                LAST_TX_MS = 0;
                LAST_RX_PACKET_MS = 0;
                BIND_START_MS = 0;
                PREV_BIND_REQ = false;
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

/// Periodic MPM serial frame transmission (`~111 Hz` / `9 ms` interval).
#[allow(clippy::too_many_arguments)]
pub fn update_link(
    now_ms: u32,
    channels_us: &[u16; NUM_CHANNELS],
    proto: Cc2500Protocol,
    rx_num: u8,
    freq_tune: i8,
    bind_req: bool,
    range_check: bool,
) {
    unsafe {
        if !MPM_ENABLED {
            return;
        }

        if bind_req && !PREV_BIND_REQ {
            BIND_START_MS = now_ms.max(1);
        } else if !bind_req {
            BIND_START_MS = 0;
        }
        PREV_BIND_REQ = bind_req;

        // Auto-complete bind after 5 seconds so pilot doesn't stay stuck in bind mode
        let binding = if bind_req {
            if BIND_START_MS != 0 && now_ms.wrapping_sub(BIND_START_MS) >= 5000 {
                crate::rf::set_bind_mode(false);
                BIND_START_MS = 0;
                false
            } else {
                true
            }
        } else {
            false
        };

        if now_ms.wrapping_sub(LAST_TX_MS) >= 9 {
            LAST_TX_MS = now_ms;
            let mut frame = [0u8; 26];
            pack_mpm_frame(
                channels_us,
                proto,
                proto.default_sub_proto(),
                rx_num,
                freq_tune,
                binding,
                range_check,
                &mut frame,
            );
            crate::uart::write_bytes(&frame);
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
        0x01 => {
            if !payload.is_empty() {
                // Bit 0 = Wait for bind; Bit 2 = Valid protocol
                let in_bind = (payload[0] & 0x02) != 0;
                if !in_bind && unsafe { BIND_START_MS != 0 && PREV_BIND_REQ } {
                    // Module finished binding on its own
                    crate::rf::set_bind_mode(false);
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

/// Format a 26-byte MULTI-Module CC2500/4-in-1 serial frame (100,000 baud 8E2):
/// - Byte 0: `0x55` (protocols 0..31) or `0x54` (protocols 32..63)
/// - Byte 1: `(bind << 7) | (auto_bind << 6) | (range_check << 5) | (proto_id & 0x1F)`
/// - Byte 2: `(sub_proto << 4) | (rx_num & 0x0F)`
/// - Byte 3: `option` (`-127..+127`, frequency fine tuning)
/// - Bytes 4..26: 16 channels packed as 11-bit values (`0..2047`, 22 bytes total).
#[allow(clippy::too_many_arguments)]
pub fn pack_mpm_frame(
    channels_us: &[u16; NUM_CHANNELS],
    proto: Cc2500Protocol,
    sub_proto: u8,
    rx_num: u8,
    freq_tune: i8,
    binding: bool,
    range_check: bool,
    out: &mut [u8; 26],
) {
    let proto_id = proto.mpm_proto_id();
    // MULTI-Module protocol uses 0x55 for protocols 0..31 and 0x54 for protocols 32..63 (bit 5).
    out[0] = if (proto_id & 0x20) != 0 { 0x54 } else { 0x55 };

    let mut b1 = proto_id & 0x1F;
    if binding {
        b1 |= 0x80;
    }
    if range_check {
        b1 |= 0x20;
    }
    out[1] = b1;

    out[2] = ((sub_proto & 0x07) << 4) | (rx_num & 0x0F);
    out[3] = freq_tune as u8;

    // Pack 16 channels as 11-bit values (988..2012 us -> 172..1811)
    let mut bit_acc: u32 = 0;
    let mut bits_in_acc: u8 = 0;
    let mut byte_idx = 4;

    for &us in channels_us[..16].iter() {
        let us_clamped = us.clamp(988, 2012) as u32;
        let val11 = (((us_clamped - 988) * 1639) / 1024 + 172).min(2047);
        bit_acc |= val11 << bits_in_acc;
        bits_in_acc += 11;
        while bits_in_acc >= 8 && byte_idx < 26 {
            out[byte_idx] = (bit_acc & 0xFF) as u8;
            bit_acc >>= 8;
            bits_in_acc -= 8;
            byte_idx += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_mpm_frame_structure() {
        let chs = [1500u16; NUM_CHANNELS];
        let mut frame = [0u8; 26];

        pack_mpm_frame(
            &chs,
            Cc2500Protocol::FrSkyD16,
            0,
            1,
            -15,
            false,
            false,
            &mut frame,
        );

        assert_eq!(frame[0], 0x55);
        assert_eq!(frame[1], MPM_PROTO_FRSKY_X);
        assert_eq!(frame[2], 0x01); // Sub 0, RxNum 1
        assert_eq!(frame[3] as i8, -15); // Frequency tuning
    }

    #[test]
    fn test_pack_mpm_frame_binding_and_range() {
        let chs = [1500u16; NUM_CHANNELS];
        let mut frame = [0u8; 26];

        pack_mpm_frame(
            &chs,
            Cc2500Protocol::FrSkyD8,
            0,
            0,
            0,
            true,
            true,
            &mut frame,
        );

        assert_eq!(frame[0], 0x55);
        assert_eq!(frame[1], MPM_PROTO_FRSKY_D | 0x80 | 0x20);
    }

    #[test]
    fn test_pack_mpm_frame_extended_proto_id() {
        let chs = [1500u16; NUM_CHANNELS];
        let mut frame = [0u8; 26];

        pack_mpm_frame(
            &chs,
            Cc2500Protocol::Corona,
            0,
            0,
            0,
            false,
            false,
            &mut frame,
        );
        assert_eq!(frame[0], 0x54);
        assert_eq!(frame[1], MPM_PROTO_CORONA & 0x1F);

        pack_mpm_frame(
            &chs,
            Cc2500Protocol::Hitec,
            0,
            0,
            0,
            false,
            false,
            &mut frame,
        );
        assert_eq!(frame[0], 0x54);
        assert_eq!(frame[1], MPM_PROTO_HITEC & 0x1F);

        pack_mpm_frame(
            &chs,
            Cc2500Protocol::Hott,
            Cc2500Protocol::Hott.default_sub_proto(),
            2,
            0,
            false,
            false,
            &mut frame,
        );
        assert_eq!(frame[0], 0x54);
        assert_eq!(frame[1], MPM_PROTO_HOTT & 0x1F);
        assert_eq!(frame[2], 2);

        pack_mpm_frame(
            &chs,
            Cc2500Protocol::FlySky2a,
            0,
            0,
            0,
            false,
            false,
            &mut frame,
        );
        assert_eq!(frame[0], 0x55);
        assert_eq!(frame[1], MPM_PROTO_AFHDS2A);
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
