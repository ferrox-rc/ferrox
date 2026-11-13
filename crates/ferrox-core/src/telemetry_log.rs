//! Zero-heap 512-byte sector flight telemetry CSV logger for Ferrox.

use crate::hal::StorageBus;

pub const SECTOR_SIZE: usize = 512;

/// Telemetry log frame captured at periodic intervals during flight.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct FlightLogFrame {
    pub time_ms: u32,
    pub rqly: u8,
    pub rssi_dbm: i8,
    pub snr_db: i8,
    pub tx_pwr: u8,
    pub rx_vbat_mv: u16,
    pub curr_ma: u16,
    pub mah: u16,
    pub lat: i32,
    pub lon: i32,
    pub alt_m: i16,
    pub spd_kt: u16,
    pub sats: u8,
    pub vspd_cms: i16,
    pub ch1: u16,
    pub ch2: u16,
    pub ch3: u16,
    pub ch4: u16,
}

pub struct TelemetryLogger {
    sector_buf: [u8; SECTOR_SIZE],
    len: usize,
    header_written: bool,
    total_sectors_written: u32,
}

impl Default for TelemetryLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl TelemetryLogger {
    pub const fn new() -> Self {
        Self {
            sector_buf: [0u8; SECTOR_SIZE],
            len: 0,
            header_written: false,
            total_sectors_written: 0,
        }
    }

    pub fn reset(&mut self) {
        self.len = 0;
        self.header_written = false;
        self.total_sectors_written = 0;
    }

    #[inline(always)]
    pub fn sectors_written(&self) -> u32 {
        self.total_sectors_written
    }

    fn push_byte(&mut self, b: u8, bus: &mut dyn StorageBus) {
        self.sector_buf[self.len] = b;
        self.len += 1;
        if self.len >= SECTOR_SIZE {
            let _ = bus.append_telemetry_sector(&self.sector_buf);
            self.total_sectors_written = self.total_sectors_written.wrapping_add(1);
            self.len = 0;
        }
    }

    fn push_bytes(&mut self, bytes: &[u8], bus: &mut dyn StorageBus) {
        for &b in bytes {
            self.push_byte(b, bus);
        }
    }

    fn push_u32(&mut self, mut val: u32, bus: &mut dyn StorageBus) {
        if val == 0 {
            self.push_byte(b'0', bus);
            return;
        }
        let mut buf = [0u8; 10];
        let mut idx = 10;
        while val > 0 {
            idx -= 1;
            buf[idx] = b'0' + (val % 10) as u8;
            val /= 10;
        }
        self.push_bytes(&buf[idx..10], bus);
    }

    fn push_i32(&mut self, val: i32, bus: &mut dyn StorageBus) {
        if val < 0 {
            self.push_byte(b'-', bus);
            self.push_u32((-val) as u32, bus);
        } else {
            self.push_u32(val as u32, bus);
        }
    }

    /// Record a single flight telemetry frame into the 512-byte sector buffer.
    pub fn log_frame(&mut self, frame: &FlightLogFrame, bus: &mut dyn StorageBus) {
        if !self.header_written {
            self.push_bytes(
                b"time_ms,rqly,rssi,snr,pwr,vbat,curr,mah,lat,lon,alt,spd,sat,vspd,c1,c2,c3,c4\n",
                bus,
            );
            self.header_written = true;
        }

        self.push_u32(frame.time_ms, bus);
        self.push_byte(b',', bus);
        self.push_u32(frame.rqly as u32, bus);
        self.push_byte(b',', bus);
        self.push_i32(frame.rssi_dbm as i32, bus);
        self.push_byte(b',', bus);
        self.push_i32(frame.snr_db as i32, bus);
        self.push_byte(b',', bus);
        self.push_u32(frame.tx_pwr as u32, bus);
        self.push_byte(b',', bus);
        self.push_u32(frame.rx_vbat_mv as u32, bus);
        self.push_byte(b',', bus);
        self.push_u32(frame.curr_ma as u32, bus);
        self.push_byte(b',', bus);
        self.push_u32(frame.mah as u32, bus);
        self.push_byte(b',', bus);
        self.push_i32(frame.lat, bus);
        self.push_byte(b',', bus);
        self.push_i32(frame.lon, bus);
        self.push_byte(b',', bus);
        self.push_i32(frame.alt_m as i32, bus);
        self.push_byte(b',', bus);
        self.push_u32(frame.spd_kt as u32, bus);
        self.push_byte(b',', bus);
        self.push_u32(frame.sats as u32, bus);
        self.push_byte(b',', bus);
        self.push_i32(frame.vspd_cms as i32, bus);
        self.push_byte(b',', bus);
        self.push_u32(frame.ch1 as u32, bus);
        self.push_byte(b',', bus);
        self.push_u32(frame.ch2 as u32, bus);
        self.push_byte(b',', bus);
        self.push_u32(frame.ch3 as u32, bus);
        self.push_byte(b',', bus);
        self.push_u32(frame.ch4 as u32, bus);
        self.push_byte(b'\n', bus);
    }

    /// Flush any remaining buffered bytes in the sector at end of flight.
    pub fn flush(&mut self, bus: &mut dyn StorageBus) {
        if self.len > 0 {
            // Zero-fill remainder of sector
            for i in self.len..SECTOR_SIZE {
                self.sector_buf[i] = 0;
            }
            let _ = bus.append_telemetry_sector(&self.sector_buf);
            self.total_sectors_written = self.total_sectors_written.wrapping_add(1);
            self.len = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockStorageBus {
        sectors_written: usize,
        first_sector: [u8; 512],
        last_sector: [u8; 512],
    }

    impl StorageBus for MockStorageBus {
        fn init(&mut self) -> bool {
            true
        }
        fn read(&mut self, _addr: u16, _buf: &mut [u8]) -> Result<(), ()> {
            Ok(())
        }
        fn write(&mut self, _addr: u16, _buf: &[u8]) -> Result<(), ()> {
            Ok(())
        }
        fn append_telemetry_sector(&mut self, sector: &[u8; 512]) -> Result<(), ()> {
            if self.sectors_written == 0 {
                self.first_sector.copy_from_slice(sector);
            }
            self.sectors_written += 1;
            self.last_sector.copy_from_slice(sector);
            Ok(())
        }
    }

    #[test]
    fn test_telemetry_logger_header_and_rollover() {
        let mut bus = MockStorageBus {
            sectors_written: 0,
            first_sector: [0u8; 512],
            last_sector: [0u8; 512],
        };

        let mut logger = TelemetryLogger::new();
        assert_eq!(logger.sectors_written(), 0);

        let frame = FlightLogFrame {
            time_ms: 12500,
            rqly: 100,
            rssi_dbm: -75,
            snr_db: 12,
            tx_pwr: 3,
            rx_vbat_mv: 15800,
            curr_ma: 24500,
            mah: 450,
            lat: 37774900,
            lon: -122419400,
            alt_m: 85,
            spd_kt: 32,
            sats: 14,
            vspd_cms: 120,
            ch1: 1500,
            ch2: 1500,
            ch3: 1100,
            ch4: 1500,
        };

        // Log multiple frames until sector rolls over
        for _ in 0..15 {
            logger.log_frame(&frame, &mut bus);
        }

        assert!(
            bus.sectors_written >= 1,
            "At least one 512-byte sector should have flushed"
        );
        assert_eq!(logger.sectors_written(), bus.sectors_written as u32);

        // Verify CSV header in initial sector
        let header_str = core::str::from_utf8(&bus.first_sector).unwrap_or("");
        assert!(header_str.contains("time_ms,rqly,rssi"));

        // Flush remainder
        logger.flush(&mut bus);
        assert!(bus.sectors_written >= 2);
    }
}
