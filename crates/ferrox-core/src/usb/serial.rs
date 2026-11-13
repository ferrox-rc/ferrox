//! USB CDC-ACM Serial communications and telemetry streaming driver.
//!
//! Provides virtual COM port communication with standard JSON-lines telemetry streaming,
//! model inspection, and interactive serial CLI without third-party crate dependencies.

#![allow(dead_code)]

use crate::board::BoardProfile;
use crate::rf::afhds2a::TelemetryData;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ResetAction {
    None,
    Reboot(u32),
    Dfu(u32),
}

pub struct SerialHandler {
    rx_queue: [u8; 128],
    rx_head: usize,
    rx_tail: usize,
    tx_queue: [u8; 512],
    tx_head: usize,
    tx_tail: usize,
    cmd_buf: [u8; 64],
    cmd_len: usize,
    last_was_cr: bool,
    stream_enabled: bool,
    reset_action: ResetAction,
}

impl Default for SerialHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl SerialHandler {
    pub const fn new() -> Self {
        Self {
            rx_queue: [0u8; 128],
            rx_head: 0,
            rx_tail: 0,
            tx_queue: [0u8; 512],
            tx_head: 0,
            tx_tail: 0,
            cmd_buf: [0u8; 64],
            cmd_len: 0,
            last_was_cr: false,
            stream_enabled: false,
            reset_action: ResetAction::None,
        }
    }

    pub fn is_streaming(&self) -> bool {
        self.stream_enabled
    }

    pub fn pending_reset(&self) -> ResetAction {
        self.reset_action
    }

    /// Push incoming bytes received from the USB CDC OUT endpoint into the internal RX FIFO.
    pub fn push_rx(&mut self, bytes: &[u8]) {
        for &b in bytes {
            let next = (self.rx_head + 1) % self.rx_queue.len();
            if next != self.rx_tail {
                self.rx_queue[self.rx_head] = b;
                self.rx_head = next;
            }
        }
    }

    /// Queue bytes to transmit over the USB CDC IN endpoint.
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            let next = (self.tx_head + 1) % self.tx_queue.len();
            if next != self.tx_tail {
                self.tx_queue[self.tx_head] = b;
                self.tx_head = next;
            } else {
                break; // TX buffer full, drop excess
            }
        }
    }

    pub fn write_str(&mut self, s: &str) {
        self.write_bytes(s.as_bytes());
    }

    /// Pop up to `dst.len()` bytes from the TX queue to transmit over hardware PMA.
    pub fn pop_tx(&mut self, dst: &mut [u8]) -> usize {
        let mut count = 0;
        while count < dst.len() && self.tx_tail != self.tx_head {
            dst[count] = self.tx_queue[self.tx_tail];
            self.tx_tail = (self.tx_tail + 1) % self.tx_queue.len();
            count += 1;
        }
        count
    }

    pub fn has_pending_tx(&self) -> bool {
        self.tx_head != self.tx_tail
    }

    /// Process queued incoming data from the main loop and write pending telemetry frames.
    pub fn update(
        &mut self,
        now_ms: u32,
        rf_chs: &[u16; crate::mixer::NUM_CHANNELS],
        telem: &TelemetryData,
        battery_mv: u16,
    ) {
        while self.rx_tail != self.rx_head {
            let b = self.rx_queue[self.rx_tail];
            self.rx_tail = (self.rx_tail + 1) % self.rx_queue.len();

            // While actively streaming telemetry, any key press pauses streaming and returns to CLI prompt
            if self.stream_enabled {
                self.stream_enabled = false;
                self.cmd_len = 0;
                self.last_was_cr = false;
                self.write_bytes(crate::board::ActiveBoard::CLI_PAUSE_PROMPT);
                continue;
            }

            if b == b'\r' || b == b'\n' {
                let is_crlf_second = b == b'\n' && self.last_was_cr;
                self.last_was_cr = b == b'\r';
                if !is_crlf_second {
                    self.write_bytes(b"\r\n");
                    if self.cmd_len > 0 {
                        self.handle_command(now_ms, rf_chs, telem, battery_mv);
                        self.cmd_len = 0;
                    }
                    if !self.stream_enabled {
                        self.write_bytes(crate::board::ActiveBoard::CLI_PROMPT);
                    }
                }
            } else if b == 0x08 || b == 0x7F {
                self.last_was_cr = false;
                // Backspace
                if self.cmd_len > 0 {
                    self.cmd_len -= 1;
                    self.write_bytes(b"\x08 \x08");
                }
            } else if self.cmd_len < self.cmd_buf.len() {
                self.last_was_cr = false;
                self.cmd_buf[self.cmd_len] = b;
                self.cmd_len += 1;
                self.write_bytes(&[b]); // Local echo
            }
        }
    }

    /// Dispatch interactive CLI command responses.
    fn handle_command(
        &mut self,
        now_ms: u32,
        rf_chs: &[u16; crate::mixer::NUM_CHANNELS],
        telem: &TelemetryData,
        battery_mv: u16,
    ) {
        let cmd = core::str::from_utf8(&self.cmd_buf[..self.cmd_len])
            .unwrap_or("")
            .trim();

        if cmd.eq_ignore_ascii_case("help") {
            self.write_bytes(
                b"Commands:\r\n  help      - Show this help\r\n  status    - Firmware & battery info\r\n  channels  - Dump RF channel values (CH1..CH18)\r\n  storage   - Dump EEPROM bus, address & I/O diagnostics\r\n  telem     - Dump single telemetry frame\r\n  stream    - Start continuous telemetry streaming (any key to stop)\r\n  dfu       - Reboot into STM32 factory ROM DFU bootloader\r\n  reboot    - Reboot transmitter\r\n",
            );
        } else if cmd.eq_ignore_ascii_case("storage") || cmd.eq_ignore_ascii_case("eeprom") {
            let status = crate::storage::get_eeprom_status();
            self.write_bytes(b"Storage Diagnostics:\r\n");
            if status.detected {
                let bus = match status.bus {
                    2 => "I2C2 (PB10/PB11)",
                    1 => "I2C1 (PB8/PB9)",
                    _ => "Unknown I2C",
                };
                self.write_bytes(b"  Detected: YES\r\n  Bus:      ");
                self.write_bytes(bus.as_bytes());
                self.write_bytes(b"\r\n  Address:  0x");
                let hex = [
                    b"0123456789ABCDEF"[(status.addr >> 4) as usize],
                    b"0123456789ABCDEF"[(status.addr & 0x0F) as usize],
                ];
                self.write_bytes(&hex);
                self.write_bytes(b"\r\n  Read:     ");
                self.write_bytes(if status.read_ok {
                    b"OK\r\n"
                } else {
                    b"FAIL\r\n"
                });
                self.write_bytes(b"  Write:    ");
                self.write_bytes(if status.write_err {
                    b"FAIL\r\n"
                } else if status.write_ok {
                    b"OK\r\n"
                } else {
                    b"OK (READY)\r\n"
                });
            } else {
                self.write_bytes(b"  Detected: NO (No external I2C EEPROM found)\r\n");
            }
        } else if cmd.eq_ignore_ascii_case("status") {
            self.write_bytes(crate::board::ActiveBoard::STATUS_BANNER);
            if crate::crsf::is_enabled() {
                self.write_bytes(crate::board::ActiveBoard::CRSF_UART_STATUS);
            } else {
                self.write_bytes(b"Protocol: AFHDS 2A (A7105 SPI active)\r\n");
            }
            self.send_telemetry_line(rf_chs, telem, battery_mv);
        } else if cmd.eq_ignore_ascii_case("channels") {
            let mut cbuf = [0u8; 128];
            let mut pos = 0;
            macro_rules! append_ch {
                ($bytes:expr) => {
                    let s = $bytes;
                    if pos + s.len() <= cbuf.len() {
                        cbuf[pos..pos + s.len()].copy_from_slice(s);
                        pos += s.len();
                    }
                };
            }
            append_ch!(b"{\"ch\":[");
            for (i, &ch) in rf_chs.iter().enumerate() {
                if i > 0 {
                    append_ch!(b",");
                }
                let mut num = [b'0'; 4];
                num[0] = b'0' + ((ch / 1000) % 10) as u8;
                num[1] = b'0' + ((ch / 100) % 10) as u8;
                num[2] = b'0' + ((ch / 10) % 10) as u8;
                num[3] = b'0' + (ch % 10) as u8;
                append_ch!(&num);
            }
            append_ch!(b"]}\r\n");
            self.write_bytes(&cbuf[..pos]);
        } else if cmd.eq_ignore_ascii_case("telem") {
            self.send_telemetry_line(rf_chs, telem, battery_mv);
        } else if cmd.eq_ignore_ascii_case("stream")
            || cmd.eq_ignore_ascii_case("telem on")
            || cmd.eq_ignore_ascii_case("telem stream")
        {
            self.stream_enabled = true;
            self.write_bytes(b"[Streaming telemetry @ 10Hz. Press any key to stop.]\r\n");
        } else if cmd.eq_ignore_ascii_case("telem off") {
            self.stream_enabled = false;
            self.write_bytes(b"Telemetry streaming disabled\r\n");
        } else if cmd.eq_ignore_ascii_case("dfu")
            || cmd.eq_ignore_ascii_case("reboot bootloader")
            || cmd.eq_ignore_ascii_case("reboot-bootloader")
        {
            self.write_bytes(b"Rebooting to STM32 DFU bootloader...\r\n");
            // Set deferred DFU grace timer (200 ms) so buffer flushes to host
            self.reset_action = ResetAction::Dfu(now_ms.wrapping_add(200));
        } else if cmd.eq_ignore_ascii_case("reboot") {
            self.write_bytes(b"Rebooting...\r\n");
            // Set deferred reboot grace timer (200 ms)
            self.reset_action = ResetAction::Reboot(now_ms.wrapping_add(200));
        } else if !cmd.is_empty() {
            self.write_bytes(b"Unknown command. Type 'help'\r\n");
        }
    }

    /// Stream a single formatted JSON telemetry line over USB CDC.
    pub fn send_telemetry_line(
        &mut self,
        rf_chs: &[u16; crate::mixer::NUM_CHANNELS],
        telem: &TelemetryData,
        battery_mv: u16,
    ) {
        let mut buf = [0u8; 320];
        let mut pos = 0;

        append_bytes(&mut buf, &mut pos, b"{\"vbat\":");
        append_volt(&mut buf, &mut pos, battery_mv);
        append_bytes(&mut buf, &mut pos, b",\"rssi\":");
        append_u32(&mut buf, &mut pos, telem.rssi as u32);
        append_bytes(&mut buf, &mut pos, b",\"rx_v\":");
        append_volt(&mut buf, &mut pos, telem.rx_voltage_mv);
        append_bytes(&mut buf, &mut pos, b",\"tx\":");
        append_u32(&mut buf, &mut pos, telem.packets_sent);
        append_bytes(&mut buf, &mut pos, b",\"rx\":");
        append_u32(&mut buf, &mut pos, telem.packets_received);
        append_bytes(&mut buf, &mut pos, b",\"err\":");
        append_u32(
            &mut buf,
            &mut pos,
            telem.packets_sent.saturating_sub(telem.packets_received),
        );
        if crate::crsf::is_enabled() {
            let crsf = crate::crsf::get_telemetry();
            append_bytes(&mut buf, &mut pos, b",\"crsf\":{\"conn\":");
            append_bytes(
                &mut buf,
                &mut pos,
                if crsf.connected { b"true" } else { b"false" },
            );
            append_bytes(&mut buf, &mut pos, b",\"lq\":");
            append_u32(&mut buf, &mut pos, crsf.uplink_link_quality as u32);
            append_bytes(&mut buf, &mut pos, b",\"rssi\":");
            append_i32(&mut buf, &mut pos, crsf.uplink_rssi_1 as i32);
            append_bytes(&mut buf, &mut pos, b",\"snr\":");
            append_i32(&mut buf, &mut pos, crsf.uplink_snr as i32);
            append_bytes(&mut buf, &mut pos, b",\"pwr_mw\":");
            append_u32(&mut buf, &mut pos, crsf.tx_power_mw as u32);
            append_bytes(&mut buf, &mut pos, b",\"rf_mode\":");
            append_u32(&mut buf, &mut pos, crsf.rf_mode as u32);
            append_bytes(&mut buf, &mut pos, b"}");
        }
        append_bytes(&mut buf, &mut pos, b",\"ch\":[");
        for (i, &ch) in rf_chs.iter().enumerate() {
            if i > 0 {
                append_bytes(&mut buf, &mut pos, b",");
            }
            append_u32(&mut buf, &mut pos, ch as u32);
        }
        append_bytes(&mut buf, &mut pos, b"]}\r\n");

        self.write_bytes(&buf[..pos]);
    }
}

fn append_bytes(buf: &mut [u8], pos: &mut usize, bytes: &[u8]) {
    let p = *pos;
    if p + bytes.len() <= buf.len() {
        buf[p..p + bytes.len()].copy_from_slice(bytes);
        *pos += bytes.len();
    }
}

fn append_u32(buf: &mut [u8], pos: &mut usize, mut val: u32) {
    if val == 0 {
        append_bytes(buf, pos, b"0");
        return;
    }
    let mut temp = [0u8; 10];
    let mut len = 0;
    while val > 0 {
        temp[len] = b'0' + (val % 10) as u8;
        val /= 10;
        len += 1;
    }
    for i in (0..len).rev() {
        append_bytes(buf, pos, &[temp[i]]);
    }
}

fn append_i32(buf: &mut [u8], pos: &mut usize, val: i32) {
    if val < 0 {
        append_bytes(buf, pos, b"-");
        append_u32(buf, pos, val.unsigned_abs());
    } else {
        append_u32(buf, pos, val as u32);
    }
}

fn append_volt(buf: &mut [u8], pos: &mut usize, mv: u16) {
    let v = (mv / 1000) as u32;
    let rem = mv % 1000;
    append_u32(buf, pos, v);
    let frac = [
        b'.',
        b'0' + ((rem / 100) % 10) as u8,
        b'0' + ((rem / 10) % 10) as u8,
    ];
    append_bytes(buf, pos, &frac);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_i32_min() {
        let mut buf = [0u8; 32];
        let mut pos = 0;
        append_i32(&mut buf, &mut pos, i32::MIN);
        assert_eq!(&buf[..pos], b"-2147483648");
    }

    #[test]
    fn test_append_volt_two_digits() {
        let mut buf = [0u8; 32];
        let mut pos = 0;
        append_volt(&mut buf, &mut pos, 12340);
        append_bytes(&mut buf, &mut pos, b" ");
        append_volt(&mut buf, &mut pos, 4150);
        assert_eq!(&buf[..pos], b"12.34 4.15");
    }
}
