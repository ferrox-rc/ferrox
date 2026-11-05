//! Spektrum DSM2 / DSMX MULTI-Module Serial Protocol, Satellite Trainer Decoder,
//! and Opt-In Forward Programming (`0x09`) / TextGen (`0x0C`) Configuration Engine.

use crate::board::DsmSubProtocol;
use crate::mixer::NUM_CHANNELS;

/// MULTI-Module protocol ID for DSM (`6`).
pub const MULTI_PROTO_DSM: u8 = 6;
/// Spektrum Telemetry Sensor ID: Flight Log (`0x7F` / `0xFE`).
pub const TELEM_ID_FLIGHT_LOG: u8 = 0x7F;
/// Spektrum Telemetry Sensor ID: Forward Programming (`0x09`).
pub const TELEM_ID_FWD_PROG: u8 = 0x09;
/// Spektrum Telemetry Sensor ID: TextGen Menu (`0x0C`).
pub const TELEM_ID_TEXTGEN: u8 = 0x0C;

/// Decoded Spektrum Flight Log link telemetry.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct DsmTelemetry {
    pub connected: bool,
    pub fades_a: u16,
    pub fades_b: u16,
    pub frame_losses: u16,
    pub holds: u16,
    pub rx_voltage_mv: u16,
    pub rssi_pct: u8,
}

/// Format a 26-byte MULTI-Module DSM serial frame (100,000 baud 8E2):
/// - Byte 0: `0x55` (Stream header for protocols 0..31)
/// - Byte 1: `(bind << 7) | (auto_bind << 6) | (range_check << 5) | MULTI_PROTO_DSM`
/// - Byte 2: `(sub_proto << 4) | num_channels` (e.g. `sub_proto`: 0=DSMX-11, 1=DSMX-22, 2=DSM2-11, 3=DSM2-22)
/// - Byte 3: `option` (`0`)
/// - Bytes 4..26: 16 channels packed as 11-bit values (`0..2047`, 22 bytes total).
pub fn pack_multi_dsm_frame(
    channels_us: &[u16; NUM_CHANNELS],
    sub_proto: DsmSubProtocol,
    binding: bool,
    range_check: bool,
    out: &mut [u8; 26],
) {
    out[0] = 0x55;
    let mut b1 = MULTI_PROTO_DSM & 0x1F;
    if binding {
        b1 |= 0x80;
    }
    if range_check {
        b1 |= 0x20;
    }
    out[1] = b1;

    let multi_sub: u8 = match sub_proto {
        DsmSubProtocol::Dsmx11 => 0,
        DsmSubProtocol::Dsmx22 => 1,
        DsmSubProtocol::Dsm2_11 => 2,
        DsmSubProtocol::Dsm2_22 => 3,
    };
    out[2] = (multi_sub << 4) | 12; // 12 channels standard DSMX
    out[3] = 0;

    // Pack 16 channels as 11-bit values (988..2012 us -> 172..1811)
    let mut bit_acc: u32 = 0;
    let mut bits_in_acc: u8 = 0;
    let mut byte_idx = 4;

    for ch in 0..16 {
        let us = channels_us[ch].clamp(988, 2012) as u32;
        let val11 = (((us - 988) * 1639) / 1024 + 172).min(2047);
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

/// Decode a 16-byte Spektrum Satellite Remote Receiver frame (115,200 baud 8N1) into up to 12 channels in µs (`1000..2000`).
/// Supports both 11-bit (2048-res) and 10-bit (1024-res) frame formats.
pub fn decode_satellite_frame(frame: &[u8; 16], is_11bit: bool, ch_out: &mut [u16; 12]) -> bool {
    // Bytes 0..2: Fades & system status; Bytes 2..16: 7 channel words per frame
    for slot in 0..7 {
        let hi = frame[2 + slot * 2];
        let lo = frame[3 + slot * 2];
        if hi == 0xFF && lo == 0xFF {
            continue;
        }
        let word = ((hi as u16) << 8) | (lo as u16);
        let (ch_id, raw_val, max_val) = if is_11bit {
            (((word >> 11) & 0x0F) as usize, (word & 0x07FF) as u32, 2048u32)
        } else {
            (((word >> 10) & 0x0F) as usize, (word & 0x03FF) as u32, 1024u32)
        };
        if ch_id < 12 {
            ch_out[ch_id] = (988 + (raw_val * 1024) / max_val) as u16;
        }
    }
    true
}

// ============================================================================
// Opt-In Spektrum Forward Programming (0x09) & TextGen (0x0C) Config Engine
// ============================================================================

#[cfg(feature = "dsm-fwd-prog")]
pub mod fwd_prog {
    pub const MAX_FP_LINES: usize = 6;
    pub const MAX_LINE_LEN: usize = 20;

    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub enum DsmConfigMode {
        Disconnected,
        Connecting,
        ForwardProgMenu,
        TextGenScreen,
    }

    #[derive(Copy, Clone, Debug)]
    pub struct FpMenuLine {
        pub id: u8,
        pub text: [u8; MAX_LINE_LEN],
        pub text_len: u8,
        pub val_text: [u8; 10],
        pub val_len: u8,
        pub editable: bool,
        pub min_val: i16,
        pub max_val: i16,
        pub cur_val: i16,
    }

    impl FpMenuLine {
        pub const fn empty() -> Self {
            Self {
                id: 0,
                text: [b' '; MAX_LINE_LEN],
                text_len: 0,
                val_text: [b' '; 10],
                val_len: 0,
                editable: false,
                min_val: 0,
                max_val: 0,
                cur_val: 0,
            }
        }
    }

    pub struct DsmConfigEngine {
        pub mode: DsmConfigMode,
        pub menu_id: u16,
        pub parent_menu_id: u16,
        pub title: [u8; MAX_LINE_LEN],
        pub title_len: u8,
        pub lines: [FpMenuLine; MAX_FP_LINES],
        pub line_count: usize,
        pub textgen_rows: [[u8; MAX_LINE_LEN]; 8],
        pub tx_pending_cmd: Option<[u8; 8]>,
        pub rx_version: u8,
    }

    impl DsmConfigEngine {
        pub const fn new() -> Self {
            Self {
                mode: DsmConfigMode::Disconnected,
                menu_id: 0,
                parent_menu_id: 0,
                title: [b' '; MAX_LINE_LEN],
                title_len: 0,
                lines: [FpMenuLine::empty(); MAX_FP_LINES],
                line_count: 0,
                textgen_rows: [[b' '; MAX_LINE_LEN]; 8],
                tx_pending_cmd: None,
                rx_version: 0,
            }
        }

        /// Initiate a Forward Programming handshake (`req = 0x10` Version Request).
        pub fn start(&mut self) {
            self.mode = DsmConfigMode::Connecting;
            self.menu_id = 0;
            self.parent_menu_id = 0;
            self.line_count = 0;
            self.tx_pending_cmd = Some([0x09, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        }

        /// Parse an incoming 16-byte Spektrum telemetry payload (`0x09` Forward Prog or `0x0C` TextGen).
        pub fn process_telem_frame(&mut self, payload: &[u8; 16]) {
            let sensor_id = payload[0];
            if sensor_id == super::TELEM_ID_FWD_PROG {
                let pkt_type = payload[2];
                match pkt_type {
                    // 0x01: Receiver Version & Root Menu Ack
                    0x01 => {
                        self.rx_version = payload[3];
                        self.mode = DsmConfigMode::ForwardProgMenu;
                        self.menu_id = u16::from_be_bytes([payload[4], payload[5]]);
                        self.parent_menu_id = u16::from_be_bytes([payload[6], payload[7]]);
                    }
                    // 0x02: Menu Title String
                    0x02 => {
                        self.mode = DsmConfigMode::ForwardProgMenu;
                        let slice = &payload[4..16];
                        let len = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
                        self.title = [b' '; MAX_LINE_LEN];
                        self.title[..len].copy_from_slice(&slice[..len]);
                        self.title_len = len as u8;
                    }
                    // 0x03: Menu Line Definition
                    0x03 => {
                        let slot = (payload[3] as usize).min(MAX_FP_LINES - 1);
                        let mut line = FpMenuLine::empty();
                        line.id = payload[4];
                        line.editable = (payload[5] & 0x01) != 0;
                        line.cur_val = i16::from_be_bytes([payload[6], payload[7]]);
                        let slice = &payload[8..16];
                        let len = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
                        line.text[..len].copy_from_slice(&slice[..len]);
                        line.text_len = len as u8;
                        self.lines[slot] = line;
                        if slot + 1 > self.line_count {
                            self.line_count = slot + 1;
                        }
                    }
                    _ => {}
                }
            } else if sensor_id == super::TELEM_ID_TEXTGEN {
                self.mode = DsmConfigMode::TextGenScreen;
                let row_idx = (payload[2] as usize) & 0x07;
                let chars = &payload[3..16];
                let mut row = [b' '; MAX_LINE_LEN];
                for (i, &c) in chars.iter().enumerate() {
                    if c == 0 {
                        break;
                    }
                    row[i] = c;
                }
                self.textgen_rows[row_idx] = row;
            }
        }

        /// Queue a Forward Programming navigation or value update command.
        pub fn send_action(&mut self, line_id: u8, value: i16) {
            let [vh, vl] = value.to_be_bytes();
            self.tx_pending_cmd = Some([0x09, 0x20, line_id, vh, vl, 0, 0, 0]);
        }
    }
}

#[cfg(feature = "dsm-fwd-prog")]
static mut DSM_ENGINE: fwd_prog::DsmConfigEngine = fwd_prog::DsmConfigEngine::new();

#[cfg(feature = "dsm-fwd-prog")]
pub fn get_config_engine() -> &'static mut fwd_prog::DsmConfigEngine {
    unsafe { &mut *core::ptr::addr_of_mut!(DSM_ENGINE) }
}

#[cfg(feature = "dsm-fwd-prog")]
pub fn start_forward_prog() {
    get_config_engine().start();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_multi_dsm_frame() {
        let mut chs = [1500u16; NUM_CHANNELS];
        chs[0] = 1000;
        chs[1] = 2000;

        let mut out = [0u8; 26];
        pack_multi_dsm_frame(&chs, DsmSubProtocol::Dsmx11, true, false, &mut out);

        // Header and protocol checks
        assert_eq!(out[0], 0x55);
        assert_eq!(out[1], 0x80 | (MULTI_PROTO_DSM & 0x1F)); // bind bit set + proto 6
        assert_eq!(out[2], (0 << 4) | 12); // DSMX-11 sub-proto, 12 channels
        assert_eq!(out[3], 0);

        // Verify channel payload is populated
        let has_non_zero = out[4..].iter().any(|&b| b != 0);
        assert!(has_non_zero);
    }

    #[test]
    fn test_decode_satellite_frame_11bit() {
        let mut frame = [0xFFu8; 16];
        frame[0] = 0; // Fades
        frame[1] = 0x01; // System: 0x01 = 2048 / 11-bit

        // Slot 0: CH0 (Aileron): Channel ID 0, value 1024 (midpoint)
        // 11-bit word: [ID:4 bits][Val:11 bits]
        // ch_id = 0, val = 1024 (0x0400) => word = (0 << 11) | 0x0400 = 0x0400
        frame[2] = 0x04;
        frame[3] = 0x00;

        let mut chs = [0u16; 12];
        let ok = decode_satellite_frame(&frame, true, &mut chs);
        assert!(ok);
        assert!(chs[0] >= 1490 && chs[0] <= 1515, "CH0 us: {}", chs[0]);
    }

    #[cfg(feature = "dsm-fwd-prog")]
    #[test]
    fn test_fwd_prog_engine_lifecycle() {
        let engine = get_config_engine();
        engine.mode = fwd_prog::DsmConfigMode::Disconnected;
        assert_eq!(engine.mode, fwd_prog::DsmConfigMode::Disconnected);

        engine.start();
        assert_eq!(engine.mode, fwd_prog::DsmConfigMode::Connecting);
        assert!(engine.tx_pending_cmd.is_some());

        // Process title frame (sensor_id 0x09, pkt_type 0x02)
        // Title payload slice is &payload[4..16], so 12 bytes max per frame
        let mut title_frame = [0u8; 16];
        title_frame[0] = TELEM_ID_FWD_PROG;
        title_frame[2] = 0x02; // Title packet
        let title_bytes = b"RECEIVER SET";
        title_frame[4..4 + title_bytes.len()].copy_from_slice(title_bytes);
        engine.process_telem_frame(&title_frame);

        assert_eq!(engine.mode, fwd_prog::DsmConfigMode::ForwardProgMenu);
        assert_eq!(&engine.title[..12], b"RECEIVER SET");

        // Send an action
        engine.send_action(1, 42);
        assert_eq!(engine.tx_pending_cmd, Some([0x09, 0x20, 1, 0, 42, 0, 0, 0]));
    }
}
