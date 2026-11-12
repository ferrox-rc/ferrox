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

/// MULTI-Module telemetry stream parser (zero-allocation state machine).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum MultiParseState {
    WaitSync1,
    WaitSync2,
    HeaderType,
    HeaderLen,
    Payload,
}

pub struct MultiRxParser {
    state: MultiParseState,
    pkt_type: u8,
    pkt_len: u8,
    idx: usize,
    buf: [u8; 32],
}

impl MultiRxParser {
    pub const fn new() -> Self {
        Self {
            state: MultiParseState::WaitSync1,
            pkt_type: 0,
            pkt_len: 0,
            idx: 0,
            buf: [0; 32],
        }
    }

    /// Feed a byte from UART RX. Returns `Some((pkt_type, payload_len))` if a valid packet arrived.
    pub fn feed_byte(&mut self, b: u8) -> Option<(u8, usize)> {
        match self.state {
            MultiParseState::WaitSync1 => {
                if b == 0x4D { // 'M'
                    self.state = MultiParseState::WaitSync2;
                }
            }
            MultiParseState::WaitSync2 => {
                if b == 0x50 { // 'P'
                    self.state = MultiParseState::HeaderType;
                } else if b == 0x4D {
                    self.state = MultiParseState::WaitSync2;
                } else {
                    self.state = MultiParseState::WaitSync1;
                }
            }
            MultiParseState::HeaderType => {
                self.pkt_type = b;
                self.state = MultiParseState::HeaderLen;
            }
            MultiParseState::HeaderLen => {
                let len = (b as usize).min(self.buf.len());
                self.pkt_len = len as u8;
                self.idx = 0;
                if len == 0 {
                    self.state = MultiParseState::WaitSync1;
                    return Some((self.pkt_type, 0));
                } else {
                    self.state = MultiParseState::Payload;
                }
            }
            MultiParseState::Payload => {
                if self.idx < self.buf.len() {
                    self.buf[self.idx] = b;
                }
                self.idx += 1;
                if self.idx >= (self.pkt_len as usize) {
                    self.state = MultiParseState::WaitSync1;
                    return Some((self.pkt_type, self.pkt_len as usize));
                }
            }
        }
        None
    }

    pub fn payload(&self) -> &[u8] {
        &self.buf[..self.pkt_len as usize]
    }
}

impl Default for MultiRxParser {
    fn default() -> Self {
        Self::new()
    }
}

static mut MULTI_PARSER: MultiRxParser = MultiRxParser::new();
static mut DSM_TELEMETRY: DsmTelemetry = DsmTelemetry {
    connected: false,
    fades_a: 0,
    fades_b: 0,
    frame_losses: 0,
    holds: 0,
    rx_voltage_mv: 0,
    rssi_pct: 0,
};
static mut DSM_LINK_ENABLED: bool = false;
static mut LAST_DSM_TX_MS: u32 = 0;
static mut DSM_BINDING: bool = false;
static mut DSM_RANGE_CHECK: bool = false;

pub fn set_enabled(enabled: bool) {
    unsafe {
        if enabled != DSM_LINK_ENABLED {
            DSM_LINK_ENABLED = enabled;
            crate::uart::set_uart_enabled(enabled, crate::uart::BAUD_IDX_MULTI_100K_8E2, false);
            crate::uart::set_module_power(enabled);
            if !enabled {
                DSM_TELEMETRY = DsmTelemetry::default();
                MULTI_PARSER = MultiRxParser::new();
            }
        }
    }
}

pub fn is_enabled() -> bool {
    unsafe { DSM_LINK_ENABLED }
}

pub fn set_bind_mode(binding: bool) {
    unsafe { DSM_BINDING = binding; }
}

pub fn set_range_check(range_check: bool) {
    unsafe { DSM_RANGE_CHECK = range_check; }
}

pub fn get_telemetry() -> DsmTelemetry {
    unsafe { DSM_TELEMETRY }
}

/// Decode a 16-byte Spektrum Flight Log payload (`0x7F` or `0xFE`).
pub fn decode_flight_log(payload: &[u8; 16], telem: &mut DsmTelemetry) {
    // Spektrum 0x7F Flight Log structure:
    // [0]: 0x7F, [1]: SubID/Type, [2..3]: Fades A, [4..5]: Fades B,
    // [6..7]: Fades L / R, [8..9]: Frame Loss, [10..11]: Holds, [12..13]: Rx Volts (0.01V)
    telem.connected = true;
    telem.fades_a = u16::from_be_bytes([payload[2], payload[3]]);
    telem.fades_b = u16::from_be_bytes([payload[4], payload[5]]);
    telem.frame_losses = u16::from_be_bytes([payload[8], payload[9]]);
    telem.holds = u16::from_be_bytes([payload[10], payload[11]]);

    let volts_raw = u16::from_be_bytes([payload[12], payload[13]]);
    if volts_raw != 0xFFFF {
        telem.rx_voltage_mv = volts_raw * 10;
    }

    // Compute synthetic link quality percentage based on frame losses and holds
    if telem.holds > 0 {
        telem.rssi_pct = 0;
    } else if telem.frame_losses > 50 {
        telem.rssi_pct = 20;
    } else if telem.frame_losses > 10 {
        telem.rssi_pct = 50;
    } else {
        telem.rssi_pct = 100u8.saturating_sub((telem.frame_losses as u8) * 2).max(60);
    }
}

/// Transmit periodic MULTI-Module DSM frames and forward pending configuration commands.
pub fn update_link(now_ms: u32, channels: &[u16; NUM_CHANNELS], sub_proto: DsmSubProtocol) {
    unsafe {
        if !DSM_LINK_ENABLED {
            return;
        }

        // Frame interval: 11ms for DSMX11/DSM2_11, 22ms for DSMX22/DSM2_22
        let interval = match sub_proto {
            DsmSubProtocol::Dsmx11 | DsmSubProtocol::Dsm2_11 => 11,
            DsmSubProtocol::Dsmx22 | DsmSubProtocol::Dsm2_22 => 22,
        };

        if now_ms.wrapping_sub(LAST_DSM_TX_MS) >= interval {
            LAST_DSM_TX_MS = now_ms;
            let mut frame = [0u8; 26];
            pack_multi_dsm_frame(channels, sub_proto, DSM_BINDING, DSM_RANGE_CHECK, &mut frame);

            #[cfg(feature = "dsm-fwd-prog")]
            {
                let engine = get_config_engine();
                engine.poll_tick(now_ms);
                if let Some(cmd) = engine.tx_pending_cmd.take() {
                    // MULTI-Module DSM Telemetry Pass-Through extension frame (option = 0x70)
                    let mut ext_frame = [0u8; 10];
                    ext_frame[0] = 0x54; // Extended stream sync
                    ext_frame[1] = 0x70; // FwdProg / Spektrum Telemetry pass-through command
                    ext_frame[2..10].copy_from_slice(&cmd);
                    crate::uart::write_bytes(&frame);
                    crate::uart::write_bytes(&ext_frame);
                    return;
                }
            }

            crate::uart::write_bytes(&frame);
        }
    }
}

/// Poll incoming bytes from UART and parse `MULTI_TELEMETRY` frames.
pub fn poll_telemetry(_now_ms: u32) {
    unsafe {
        if !DSM_LINK_ENABLED {
            return;
        }

        while let Some(b) = crate::uart::read_byte() {
            if let Some((pkt_type, len)) = MULTI_PARSER.feed_byte(b) {
                // pkt_type == 0x04: Spektrum 16-byte telemetry frame
                if pkt_type == 0x04 && len >= 16 {
                    let mut telem_buf = [0u8; 16];
                    telem_buf.copy_from_slice(&MULTI_PARSER.payload()[..16]);
                    let sensor_id = telem_buf[0];

                    if sensor_id == TELEM_ID_FLIGHT_LOG || sensor_id == 0xFE {
                        decode_flight_log(&telem_buf, &mut DSM_TELEMETRY);
                    }
                    #[cfg(feature = "dsm-fwd-prog")]
                    {
                        if sensor_id == TELEM_ID_FWD_PROG || sensor_id == TELEM_ID_TEXTGEN {
                            DSM_TELEMETRY.connected = true;
                            get_config_engine().process_telem_frame(&telem_buf);
                        }
                    }
                }
            }
        }
    }
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
        pub last_activity_ms: u32,
        pub last_req_ms: u32,
        pub retry_count: u8,
        pub next_line_req: u8,
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
                last_activity_ms: 0,
                last_req_ms: 0,
                retry_count: 0,
                next_line_req: 0,
            }
        }

        /// Initiate a Forward Programming handshake (`req = 0x10` Version Request).
        pub fn start(&mut self) {
            self.mode = DsmConfigMode::Connecting;
            self.menu_id = 0;
            self.parent_menu_id = 0;
            self.line_count = 0;
            self.retry_count = 0;
            self.next_line_req = 0;
            self.tx_pending_cmd = Some([0x09, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        }

        /// Request menu line by slot index.
        pub fn request_line(&mut self, slot: u8) {
            self.tx_pending_cmd = Some([0x09, 0x13, slot, 0x00, 0x00, 0x00, 0x00, 0x00]);
        }

        /// Request navigation back to parent menu.
        pub fn navigate_back(&mut self) {
            let [mh, ml] = self.parent_menu_id.to_be_bytes();
            self.tx_pending_cmd = Some([0x09, 0x11, mh, ml, 0x00, 0x00, 0x00, 0x00]);
            self.line_count = 0;
            self.next_line_req = 0;
        }

        /// Exit Forward Programming mode.
        pub fn exit(&mut self) {
            self.mode = DsmConfigMode::Disconnected;
            self.tx_pending_cmd = Some([0x09, 0x1F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        }

        /// Periodic tick for retry timeouts and keep-alives.
        pub fn poll_tick(&mut self, now_ms: u32) {
            match self.mode {
                DsmConfigMode::Connecting => {
                    // Retry version request if no reply within 400ms
                    if now_ms.wrapping_sub(self.last_req_ms) >= 400 {
                        self.last_req_ms = now_ms;
                        if self.retry_count < 10 {
                            self.retry_count += 1;
                            self.tx_pending_cmd = Some([0x09, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
                        }
                    }
                }
                DsmConfigMode::ForwardProgMenu => {
                    // Request remaining lines sequentially
                    if (self.next_line_req as usize) < self.line_count {
                        if now_ms.wrapping_sub(self.last_req_ms) >= 150 {
                            self.last_req_ms = now_ms;
                            let slot = self.next_line_req;
                            self.request_line(slot);
                            self.next_line_req += 1;
                        }
                    } else if now_ms.wrapping_sub(self.last_activity_ms) >= 1000 {
                        // Periodic keep-alive frame every 1000ms so receiver doesn't exit programming
                        self.last_activity_ms = now_ms;
                        let [mh, ml] = self.menu_id.to_be_bytes();
                        self.tx_pending_cmd = Some([0x09, 0x00, mh, ml, 0x00, 0x00, 0x00, 0x00]);
                    }
                }
                _ => {}
            }
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
                        self.line_count = (payload[8] as usize).min(MAX_FP_LINES);
                        self.next_line_req = 0;
                        // Request root menu header
                        let [mh, ml] = self.menu_id.to_be_bytes();
                        self.tx_pending_cmd = Some([0x09, 0x11, mh, ml, 0x00, 0x00, 0x00, 0x00]);
                    }
                    // 0x02: Menu Title String
                    0x02 => {
                        self.mode = DsmConfigMode::ForwardProgMenu;
                        let slice = &payload[4..16];
                        let len = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
                        self.title = [b' '; MAX_LINE_LEN];
                        self.title[..len].copy_from_slice(&slice[..len]);
                        self.title_len = len as u8;
                        // Trigger retrieval of first menu line
                        if self.line_count > 0 {
                            self.next_line_req = 1;
                            self.request_line(0);
                        }
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
                    // 0x04: Menu Value / Range Update
                    0x04 => {
                        let line_id = payload[3];
                        for line in self.lines.iter_mut() {
                            if line.id == line_id {
                                line.min_val = i16::from_be_bytes([payload[4], payload[5]]);
                                line.max_val = i16::from_be_bytes([payload[6], payload[7]]);
                                line.cur_val = i16::from_be_bytes([payload[8], payload[9]]);
                                let slice = &payload[10..16];
                                let len = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
                                line.val_text = [b' '; 10];
                                line.val_text[..len].copy_from_slice(&slice[..len]);
                                line.val_len = len as u8;
                                break;
                            }
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

    impl Default for DsmConfigEngine {
        fn default() -> Self {
            Self::new()
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
        assert_eq!(out[2], 12); // DSMX-11 sub-proto, 12 channels
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

    #[test]
    fn test_multi_rx_parser_stream() {
        let mut parser = MultiRxParser::new();

        // Feed prefix noise
        assert_eq!(parser.feed_byte(0x00), None);
        assert_eq!(parser.feed_byte(0xAA), None);

        // Feed MULTI header: 'M' (0x4D), 'P' (0x50), Type: 0x04, Len: 16
        assert_eq!(parser.feed_byte(0x4D), None);
        assert_eq!(parser.feed_byte(0x50), None);
        assert_eq!(parser.feed_byte(0x04), None);
        assert_eq!(parser.feed_byte(16), None);

        // Build 16-byte Flight Log payload
        let mut payload = [0u8; 16];
        payload[0] = TELEM_ID_FLIGHT_LOG;
        payload[1] = 0x00;
        payload[2] = 0x00; payload[3] = 0x05; // Fades A = 5
        payload[4] = 0x00; payload[5] = 0x02; // Fades B = 2
        payload[8] = 0x00; payload[9] = 0x01; // Frame Losses = 1
        payload[10] = 0x00; payload[11] = 0x00; // Holds = 0
        payload[12] = 0x01; payload[13] = 0xF4; // Rx Volts = 500 (5.00V -> 5000 mV)

        let mut res = None;
        for &b in &payload {
            res = parser.feed_byte(b);
        }

        assert_eq!(res, Some((0x04, 16)));
        let mut telem = DsmTelemetry::default();
        decode_flight_log(&payload, &mut telem);
        assert!(telem.connected);
        assert_eq!(telem.fades_a, 5);
        assert_eq!(telem.fades_b, 2);
        assert_eq!(telem.frame_losses, 1);
        assert_eq!(telem.holds, 0);
        assert_eq!(telem.rx_voltage_mv, 5000);
        assert_eq!(telem.rssi_pct, 98);
    }

    #[cfg(feature = "dsm-fwd-prog")]
    #[test]
    fn test_fwd_prog_line_and_value_updates() {
        let engine = get_config_engine();
        engine.start();

        // 1. Version Ack (0x01): rx_version=2, menu_id=10, parent=5, 3 lines
        let mut ack_frame = [0u8; 16];
        ack_frame[0] = TELEM_ID_FWD_PROG;
        ack_frame[2] = 0x01;
        ack_frame[3] = 2; // version
        ack_frame[4] = 0; ack_frame[5] = 10; // menu_id
        ack_frame[6] = 0; ack_frame[7] = 5;  // parent_menu_id
        ack_frame[8] = 3; // line_count
        engine.process_telem_frame(&ack_frame);

        assert_eq!(engine.rx_version, 2);
        assert_eq!(engine.menu_id, 10);
        assert_eq!(engine.parent_menu_id, 5);
        assert_eq!(engine.line_count, 3);
        assert_eq!(engine.tx_pending_cmd, Some([0x09, 0x11, 0, 10, 0, 0, 0, 0]));

        // 2. Menu Line (0x03): slot 0, line_id 1, editable=true, cur_val=50, text="AS3X Gain"
        let mut line_frame = [0u8; 16];
        line_frame[0] = TELEM_ID_FWD_PROG;
        line_frame[2] = 0x03;
        line_frame[3] = 0; // slot
        line_frame[4] = 1; // line_id
        line_frame[5] = 1; // editable
        line_frame[6] = 0; line_frame[7] = 50; // cur_val
        let text = b"AS3XGain";
        line_frame[8..8 + text.len()].copy_from_slice(text);
        engine.process_telem_frame(&line_frame);

        assert_eq!(engine.lines[0].id, 1);
        assert!(engine.lines[0].editable);
        assert_eq!(engine.lines[0].cur_val, 50);
        assert_eq!(&engine.lines[0].text[..text.len()], text);

        // 3. Value Update (0x04): line_id 1, min=0, max=100, cur=75, val_text="75%"
        let mut val_frame = [0u8; 16];
        val_frame[0] = TELEM_ID_FWD_PROG;
        val_frame[2] = 0x04;
        val_frame[3] = 1; // line_id
        val_frame[4] = 0; val_frame[5] = 0; // min = 0
        val_frame[6] = 0; val_frame[7] = 100; // max = 100
        val_frame[8] = 0; val_frame[9] = 75; // cur = 75
        let vtext = b"75%";
        val_frame[10..10 + vtext.len()].copy_from_slice(vtext);
        engine.process_telem_frame(&val_frame);

        assert_eq!(engine.lines[0].min_val, 0);
        assert_eq!(engine.lines[0].max_val, 100);
        assert_eq!(engine.lines[0].cur_val, 75);
        assert_eq!(&engine.lines[0].val_text[..vtext.len()], vtext);

        // 4. Back navigation test
        engine.navigate_back();
        assert_eq!(engine.tx_pending_cmd, Some([0x09, 0x11, 0, 5, 0, 0, 0, 0]));
        assert_eq!(engine.line_count, 0);
    }
}
