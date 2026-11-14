//! Non-blocking WAV/PCM voice audio playback and telemetry speech callouts.
//!
//! Reads EdgeTX-compatible `/SOUNDS/en/` WAV files from microSD via [`crate::fat`]
//! and streams 12-bit unsigned PCM samples to [`crate::hal::AudioOutput`] from the
//! background idle loop without blocking the 4 ms RF flight loop.

pub mod wav;

use crate::fat::{BlockDevice, FatVolume, FileCursor};
use crate::hal::AudioOutput;
use wav::{decode_pcm_to_dac12, parse_wav_header, WavFormat};

static mut AUDIO_OUTPUT: Option<&'static mut dyn AudioOutput> = None;

/// Register the target MCU's PCM/DAC audio driver.
pub fn register_audio_output(driver: &'static mut dyn AudioOutput) {
    unsafe {
        AUDIO_OUTPUT = Some(driver);
    }
}

/// Returns `true` if a hardware PCM audio driver is registered.
pub fn has_audio_output() -> bool {
    unsafe { AUDIO_OUTPUT.is_some() }
}

/// Spoken unit suffix for numeric telemetry callouts (EdgeTX `en` system IDs).
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum VoiceUnit {
    Meters,
    Volts,
    Decibels,
    Percent,
    Minutes,
    Seconds,
}

impl VoiceUnit {
    const fn system_id(self) -> u16 {
        match self {
            Self::Volts => 116,    // 0116.wav ("volts")
            Self::Meters => 123,   // 0123.wav ("meters")
            Self::Minutes => 128,  // 0128.wav ("minutes")
            Self::Seconds => 129,  // 0129.wav ("seconds")
            Self::Percent => 134,  // 0134.wav ("percent")
            Self::Decibels => 135, // 0135.wav ("decibels")
        }
    }
}

/// Telemetry parameter kind for spoken callouts.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TelemetryCallout {
    Altitude,
    Distance,
    Rssi,
    LinkQuality,
    RxBat,
    TxBat,
}

impl TelemetryCallout {
    const fn filename(self) -> &'static str {
        match self {
            Self::Altitude => "altitude.wav",
            Self::Distance => "distance.wav",
            Self::Rssi | Self::LinkQuality => "rssi.wav",
            Self::RxBat => "rxbat.wav",
            Self::TxBat => "txbat.wav",
        }
    }
}

/// Fixed-capacity clip path entry (`no_std`, zero-heap).
#[derive(Copy, Clone)]
enum QueuedClip {
    Path { buf: [u8; 40], len: u8 },
    Label(TelemetryCallout),
    SystemId(u16),
}

impl QueuedClip {
    fn from_path(path: &str) -> Option<Self> {
        let bytes = path.as_bytes();
        if bytes.is_empty() || bytes.len() > 40 {
            return None;
        }
        let mut buf = [0u8; 40];
        buf[..bytes.len()].copy_from_slice(bytes);
        Some(Self::Path {
            buf,
            len: bytes.len() as u8,
        })
    }
}

const QUEUE_CAP: usize = 24;

/// Non-blocking audio clip queue and WAV sector streamer.
pub struct AudioPlayer {
    queue: [Option<QueuedClip>; QUEUE_CAP],
    q_head: usize,
    q_tail: usize,
    q_count: usize,
    lang: [u8; 2],
    volume: Option<FatVolume>,
    sd_probed: bool,
    sd_available: bool,
    cursor: Option<FileCursor>,
    fmt: Option<WavFormat>,
    sector_buf: [u8; 512],
    sector_pos: usize,
    sector_len: usize,
    pcm_bytes_left: u32,
    carry: [u8; 4],
    carry_len: usize,
}

impl AudioPlayer {
    pub const fn new() -> Self {
        Self {
            queue: [None; QUEUE_CAP],
            q_head: 0,
            q_tail: 0,
            q_count: 0,
            lang: *b"en",
            volume: None,
            sd_probed: false,
            sd_available: false,
            cursor: None,
            fmt: None,
            sector_buf: [0u8; 512],
            sector_pos: 0,
            sector_len: 0,
            pcm_bytes_left: 0,
            carry: [0u8; 4],
            carry_len: 0,
        }
    }

    pub fn set_lang(&mut self, lang: &str) {
        let bytes = lang.as_bytes();
        if bytes.len() >= 2 {
            self.lang = [bytes[0].to_ascii_lowercase(), bytes[1].to_ascii_lowercase()];
        }
    }

    pub fn is_busy(&self) -> bool {
        self.cursor.is_some() || self.q_count > 0
    }

    pub fn clear_queue(&mut self) {
        self.q_head = 0;
        self.q_tail = 0;
        self.q_count = 0;
    }

    pub fn push_clip(&mut self, path: &str) -> bool {
        let Some(clip) = QueuedClip::from_path(path) else {
            return false;
        };
        self.push_item(clip)
    }

    pub fn push_system_id(&mut self, id: u16) -> bool {
        self.push_item(QueuedClip::SystemId(id))
    }

    fn push_item(&mut self, item: QueuedClip) -> bool {
        if self.q_count >= QUEUE_CAP {
            return false;
        }
        self.queue[self.q_tail] = Some(item);
        self.q_tail = (self.q_tail + 1) % QUEUE_CAP;
        self.q_count += 1;
        true
    }

    fn pop_item(&mut self) -> Option<QueuedClip> {
        if self.q_count == 0 {
            return None;
        }
        let item = self.queue[self.q_head].take();
        self.q_head = (self.q_head + 1) % QUEUE_CAP;
        self.q_count -= 1;
        item
    }

    /// Decompose a signed integer (`-9999..=9999`) into EdgeTX `SYSTEM/XXXX.wav` clips.
    pub fn speak_number(&mut self, value: i32, unit: Option<VoiceUnit>) {
        let mut n = value;
        if n < 0 {
            self.push_system_id(111); // "minus"
            n = n.saturating_abs();
        }
        if n > 9999 {
            n = 9999;
        }
        if n == 0 {
            self.push_system_id(0);
        } else {
            let thousands = (n / 1000) as u16;
            let rem_th = n % 1000;
            if thousands > 0 {
                self.push_system_id(thousands);
                self.push_system_id(109); // "thousand"
            }
            let hundreds = (rem_th / 100) as u16;
            let rem_hu = rem_th % 100;
            if hundreds > 0 {
                // EdgeTX `en`: 0100="one hundred", 0101="two hundred", ..., 0108="nine hundred"
                self.push_system_id(100 + (hundreds - 1));
            }
            if rem_hu > 0 || (thousands == 0 && hundreds == 0) {
                self.push_system_id(rem_hu as u16);
            }
        }
        if let Some(u) = unit {
            self.push_system_id(u.system_id());
        }
    }

    /// Queue a telemetry label + value + unit sequence.
    pub fn announce_telemetry(&mut self, kind: TelemetryCallout, value: i32) {
        self.push_item(QueuedClip::Label(kind));
        let unit = match kind {
            TelemetryCallout::Altitude | TelemetryCallout::Distance => Some(VoiceUnit::Meters),
            TelemetryCallout::Rssi => Some(VoiceUnit::Decibels),
            TelemetryCallout::LinkQuality => Some(VoiceUnit::Percent),
            TelemetryCallout::RxBat | TelemetryCallout::TxBat => Some(VoiceUnit::Volts),
        };
        self.speak_number(value, unit);
    }

    /// Advance playback state machine by reading/decoding up to 4 sectors per tick.
    pub fn tick_with<D: BlockDevice>(
        &mut self,
        dev: &mut D,
        out: &mut dyn AudioOutput,
        volume_pct: u8,
        ensure_sd: impl FnOnce() -> bool,
    ) {
        if self.cursor.is_none() {
            if self.q_count == 0 {
                if out.is_playing() {
                    // Let DMA drain naturally before muting and releasing PA4.
                    return;
                }
                out.stop();
                return;
            }
            if !self.sd_probed {
                self.sd_probed = true;
                self.sd_available = ensure_sd();
                if self.sd_available {
                    self.volume = FatVolume::mount(dev).ok();
                }
            }
            if self.volume.is_none() {
                self.clear_queue();
                out.stop();
                return;
            }
            while let Some(item) = self.pop_item() {
                if self.open_clip(dev, out, item) {
                    break;
                }
            }
            if self.cursor.is_none() && self.q_count == 0 && !out.is_playing() {
                out.stop();
                return;
            }
        }

        // Stream up to 4 sectors per background tick so we never stall the 4ms RF loop.
        let mut sectors_this_tick = 0usize;
        let mut dac_buf = [0u16; 256];
        while self.cursor.is_some() && out.free_space() >= 64 && sectors_this_tick < 4 {
            let Some(fmt) = self.fmt else {
                self.cursor = None;
                break;
            };
            if self.sector_pos >= self.sector_len {
                if self.pcm_bytes_left == 0 {
                    self.cursor = None;
                    self.fmt = None;
                    break;
                }
                let Some(ref mut vol) = self.volume else {
                    self.cursor = None;
                    break;
                };
                let Some(ref mut cursor) = self.cursor else {
                    break;
                };
                match cursor.read_next_sector(vol, dev, &mut self.sector_buf) {
                    Ok(0) | Err(_) => {
                        self.cursor = None;
                        self.fmt = None;
                        break;
                    }
                    Ok(n) => {
                        let valid = n.min(self.pcm_bytes_left as usize);
                        self.sector_pos = 0;
                        self.sector_len = valid;
                        self.pcm_bytes_left = self.pcm_bytes_left.saturating_sub(valid as u32);
                        sectors_this_tick += 1;
                    }
                }
            }

            let bpf = fmt.bytes_per_frame();
            if self.carry_len > 0 && self.sector_pos < self.sector_len {
                let need = bpf.saturating_sub(self.carry_len);
                let avail = self.sector_len - self.sector_pos;
                let take = need.min(avail);
                let start = self.carry_len;
                self.carry[start..start + take]
                    .copy_from_slice(&self.sector_buf[self.sector_pos..self.sector_pos + take]);
                self.carry_len += take;
                self.sector_pos += take;
                if self.carry_len >= bpf {
                    let (written, _) = decode_pcm_to_dac12(
                        &self.carry[..bpf],
                        &fmt,
                        volume_pct,
                        &mut dac_buf[..1],
                    );
                    if written > 0 {
                        let accepted = out.write_samples(&dac_buf[..written]);
                        if accepted == 0 {
                            // Rewind not needed if free_space >= 64, but guard anyway
                            break;
                        }
                    }
                    self.carry_len = 0;
                }
            }

            let free = out.free_space().min(dac_buf.len());
            if free == 0 || self.sector_pos >= self.sector_len {
                continue;
            }
            let avail_bytes = self.sector_len - self.sector_pos;
            if avail_bytes < bpf {
                self.carry[..avail_bytes]
                    .copy_from_slice(&self.sector_buf[self.sector_pos..self.sector_len]);
                self.carry_len = avail_bytes;
                self.sector_pos = self.sector_len;
                continue;
            }
            let (written, consumed) = decode_pcm_to_dac12(
                &self.sector_buf[self.sector_pos..self.sector_len],
                &fmt,
                volume_pct,
                &mut dac_buf[..free],
            );
            if written == 0 {
                break;
            }
            let accepted = out.write_samples(&dac_buf[..written]);
            if accepted == 0 {
                break;
            }
            let bytes_used = if accepted == written {
                consumed
            } else {
                accepted * bpf
            };
            self.sector_pos += bytes_used;
        }
    }

    fn open_clip<D: BlockDevice>(
        &mut self,
        dev: &mut D,
        out: &mut dyn AudioOutput,
        item: QueuedClip,
    ) -> bool {
        let Some(ref mut vol) = self.volume else {
            return false;
        };

        let mut cursor = match item {
            QueuedClip::Path { buf, len } => {
                let Ok(path) = core::str::from_utf8(&buf[..len as usize]) else {
                    return false;
                };
                match vol.open_file(dev, path) {
                    Ok(c) => c,
                    Err(_) => return false,
                }
            }
            QueuedClip::Label(kind) => {
                let mut p = [0u8; 32];
                let s = format_label_path(&mut p, self.lang, kind.filename());
                match vol.open_file(dev, s) {
                    Ok(c) => c,
                    Err(_) => return false,
                }
            }
            QueuedClip::SystemId(id) => {
                let mut p1 = [0u8; 32];
                let s1 = format_system_path(&mut p1, self.lang, true, id);
                if let Ok(c) = vol.open_file(dev, s1) {
                    c
                } else {
                    let mut p2 = [0u8; 32];
                    let s2 = format_system_path(&mut p2, self.lang, false, id);
                    match vol.open_file(dev, s2) {
                        Ok(c) => c,
                        Err(_) => return false,
                    }
                }
            }
        };

        let Ok(n) = cursor.read_next_sector(vol, dev, &mut self.sector_buf) else {
            return false;
        };
        let Ok(fmt) = parse_wav_header(&self.sector_buf[..n]) else {
            return false;
        };
        if fmt.data_offset > n || !out.start(fmt.sample_rate) {
            return false;
        }

        let first_pcm_bytes = (n - fmt.data_offset).min(fmt.data_size as usize);
        self.cursor = Some(cursor);
        self.fmt = Some(fmt);
        self.sector_pos = fmt.data_offset;
        self.sector_len = fmt.data_offset + first_pcm_bytes;
        self.pcm_bytes_left = fmt.data_size.saturating_sub(first_pcm_bytes as u32);
        self.carry_len = 0;
        true
    }
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}

fn write_lang_prefix(buf: &mut [u8; 32], lang: [u8; 2]) -> usize {
    buf[..8].copy_from_slice(b"/SOUNDS/");
    buf[8] = lang[0];
    buf[9] = lang[1];
    buf[10] = b'/';
    11
}

fn format_label_path<'a>(buf: &'a mut [u8; 32], lang: [u8; 2], filename: &str) -> &'a str {
    let mut n = write_lang_prefix(buf, lang);
    let fb = filename.as_bytes();
    let take = fb.len().min(buf.len() - n);
    buf[n..n + take].copy_from_slice(&fb[..take]);
    n += take;
    core::str::from_utf8(&buf[..n]).unwrap_or("")
}

fn format_system_path(buf: &mut [u8; 32], lang: [u8; 2], with_system_dir: bool, id: u16) -> &str {
    let mut n = write_lang_prefix(buf, lang);
    if with_system_dir {
        buf[n..n + 7].copy_from_slice(b"SYSTEM/");
        n += 7;
    }
    let d0 = ((id / 1000) % 10) as u8 + b'0';
    let d1 = ((id / 100) % 10) as u8 + b'0';
    let d2 = ((id / 10) % 10) as u8 + b'0';
    let d3 = (id % 10) as u8 + b'0';
    buf[n] = d0;
    buf[n + 1] = d1;
    buf[n + 2] = d2;
    buf[n + 3] = d3;
    n += 4;
    buf[n..n + 4].copy_from_slice(b".wav");
    n += 4;
    core::str::from_utf8(&buf[..n]).unwrap_or("")
}

struct StorageBusBlockDevice;

impl BlockDevice for StorageBusBlockDevice {
    fn read_sector(&mut self, lba: u32, buf: &mut [u8; 512]) -> Result<(), ()> {
        crate::storage::read_raw_sd_sector(lba, buf)
    }
}

static mut PLAYER: AudioPlayer = AudioPlayer::new();

/// Initialize the registered hardware PCM audio driver (if present).
pub fn init() {
    unsafe {
        if let Some(ref mut out) = AUDIO_OUTPUT {
            out.init();
        }
    }
}

/// Set the active EdgeTX voice pack 2-letter language folder (e.g. `"en"`, `"de"`, `"nl"`).
pub fn set_lang(lang: &str) {
    unsafe {
        PLAYER.set_lang(lang);
    }
}

/// Queue a WAV file path (e.g. `"/SOUNDS/en/altitude.wav"`) for playback.
pub fn play_file(path: &str) -> bool {
    if !has_audio_output() {
        return false;
    }
    unsafe { PLAYER.push_clip(path) }
}

/// Speak an integer value with optional EdgeTX unit suffix.
pub fn speak_number(value: i32, unit: Option<VoiceUnit>) {
    if !has_audio_output() {
        return;
    }
    unsafe {
        PLAYER.speak_number(value, unit);
    }
}

/// Speak a telemetry callout (label + number + unit).
pub fn announce_telemetry(kind: TelemetryCallout, value: i32) {
    if !has_audio_output() {
        return;
    }
    unsafe {
        PLAYER.announce_telemetry(kind, value);
    }
}

/// Advance background WAV/PCM streaming (call from idle loop, never from 4 ms RF loop).
pub fn tick(volume_pct: u8) {
    unsafe {
        let Some(ref mut out) = AUDIO_OUTPUT else {
            return;
        };
        let mut dev = StorageBusBlockDevice;
        PLAYER.tick_with(&mut dev, *out, volume_pct, crate::storage::init_sd_readonly);
    }
}

/// Periodic telemetry voice callout scheduler.
pub struct CalloutScheduler {
    last_callout_ms: u32,
}

impl CalloutScheduler {
    pub const fn new() -> Self {
        Self { last_callout_ms: 0 }
    }

    /// Trigger periodic altitude/RSSI/LQ callouts when `interval_s > 0` and link is active.
    pub fn maybe_announce(
        &mut self,
        now_ms: u32,
        interval_s: u8,
        link_active: bool,
        altitude_m: Option<i16>,
        rssi_dbm: i8,
        lq_pct: u8,
    ) {
        if interval_s == 0 || !link_active || !has_audio_output() {
            return;
        }
        let period_ms = (interval_s as u32) * 1000;
        if now_ms.wrapping_sub(self.last_callout_ms) < period_ms {
            return;
        }
        unsafe {
            if PLAYER.is_busy() {
                return;
            }
        }
        self.last_callout_ms = now_ms;
        if let Some(alt) = altitude_m {
            announce_telemetry(TelemetryCallout::Altitude, alt as i32);
        } else if rssi_dbm < 0 {
            announce_telemetry(TelemetryCallout::Rssi, rssi_dbm as i32);
        } else if lq_pct > 0 {
            announce_telemetry(TelemetryCallout::LinkQuality, lq_pct as i32);
        }
    }
}

impl Default for CalloutScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockAudio {
        started_rate: Option<u32>,
        stopped: bool,
        samples: [u16; 2048],
        count: usize,
    }

    impl MockAudio {
        fn new() -> Self {
            Self {
                started_rate: None,
                stopped: false,
                samples: [0u16; 2048],
                count: 0,
            }
        }
    }

    impl AudioOutput for MockAudio {
        fn init(&mut self) {}
        fn start(&mut self, sample_rate_hz: u32) -> bool {
            self.started_rate = Some(sample_rate_hz);
            self.stopped = false;
            true
        }
        fn stop(&mut self) {
            self.stopped = true;
        }
        fn write_samples(&mut self, samples: &[u16]) -> usize {
            let room = self.samples.len() - self.count;
            let take = room.min(samples.len());
            self.samples[self.count..self.count + take].copy_from_slice(&samples[..take]);
            self.count += take;
            take
        }
        fn free_space(&self) -> usize {
            self.samples.len() - self.count
        }
        fn is_playing(&self) -> bool {
            false
        }
    }

    #[test]
    fn speak_number_decomposes_edgetx_ids() {
        let mut p = AudioPlayer::new();
        p.speak_number(235, Some(VoiceUnit::Meters));
        // 235 -> 0101 (200), 0035 (35), 0123 (meters)
        assert!(matches!(p.pop_item(), Some(QueuedClip::SystemId(101))));
        assert!(matches!(p.pop_item(), Some(QueuedClip::SystemId(35))));
        assert!(matches!(p.pop_item(), Some(QueuedClip::SystemId(123))));
        assert!(p.pop_item().is_none());
    }

    #[test]
    fn speak_negative_zero_and_thousands() {
        let mut p = AudioPlayer::new();
        p.speak_number(-1042, None);
        // -1042 -> 0111 (minus), 0001 (1), 0109 (thousand), 0042 (42)
        assert!(matches!(p.pop_item(), Some(QueuedClip::SystemId(111))));
        assert!(matches!(p.pop_item(), Some(QueuedClip::SystemId(1))));
        assert!(matches!(p.pop_item(), Some(QueuedClip::SystemId(109))));
        assert!(matches!(p.pop_item(), Some(QueuedClip::SystemId(42))));
        assert!(p.pop_item().is_none());
    }

    #[test]
    fn system_path_formatting() {
        let mut b = [0u8; 32];
        assert_eq!(
            format_system_path(&mut b, *b"en", true, 42),
            "/SOUNDS/en/SYSTEM/0042.wav"
        );
        let mut b2 = [0u8; 32];
        assert_eq!(
            format_system_path(&mut b2, *b"de", false, 123),
            "/SOUNDS/de/0123.wav"
        );
        let mut b3 = [0u8; 32];
        assert_eq!(
            format_label_path(&mut b3, *b"nl", TelemetryCallout::Altitude.filename()),
            "/SOUNDS/nl/altitude.wav"
        );
    }

    #[test]
    fn end_to_end_wav_stream_from_fat32_to_mock_audio() {
        struct Disk {
            sectors: [[u8; 512]; 32],
        }
        impl BlockDevice for Disk {
            fn read_sector(&mut self, lba: u32, buf: &mut [u8; 512]) -> Result<(), ()> {
                buf.copy_from_slice(&self.sectors[lba as usize]);
                Ok(())
            }
        }
        let mut disk = Disk {
            sectors: [[0u8; 512]; 32],
        };
        // Superfloppy FAT32 at LBA 0
        let vbr = &mut disk.sectors[0];
        vbr[0] = 0xEB;
        vbr[11..13].copy_from_slice(&512u16.to_le_bytes());
        vbr[13] = 1;
        vbr[14..16].copy_from_slice(&1u16.to_le_bytes());
        vbr[16] = 1;
        vbr[32..36].copy_from_slice(&70000u32.to_le_bytes());
        vbr[36..40].copy_from_slice(&1u32.to_le_bytes()); // FAT at LBA 1, data at LBA 2
        vbr[44..48].copy_from_slice(&2u32.to_le_bytes());
        vbr[510] = 0x55;
        vbr[511] = 0xAA;

        // FAT at LBA 1: cl2,3,4,5 = EOC
        for cl in 2..=5 {
            let off = cl * 4;
            disk.sectors[1][off..off + 4].copy_from_slice(&0x0FFF_FFFFu32.to_le_bytes());
        }
        // Root (cl2 -> LBA 2): SOUNDS dir (cl3)
        disk.sectors[2][0..11].copy_from_slice(b"SOUNDS     ");
        disk.sectors[2][11] = 0x10;
        disk.sectors[2][26..28].copy_from_slice(&3u16.to_le_bytes());
        // SOUNDS (cl3 -> LBA 3): EN dir (cl4)
        disk.sectors[3][0..11].copy_from_slice(b"EN         ");
        disk.sectors[3][11] = 0x10;
        disk.sectors[3][26..28].copy_from_slice(&4u16.to_le_bytes());
        // EN (cl4 -> LBA 4): 0001.WAV (cl5, size 48 = 44 hdr + 4 bytes = 2 samples)
        disk.sectors[4][0..11].copy_from_slice(b"0001    WAV");
        disk.sectors[4][11] = 0x20;
        disk.sectors[4][26..28].copy_from_slice(&5u16.to_le_bytes());
        disk.sectors[4][28..32].copy_from_slice(&48u32.to_le_bytes());

        // 0001.WAV at cl5 (LBA 5)
        let w = &mut disk.sectors[5];
        w[0..4].copy_from_slice(b"RIFF");
        w[4..8].copy_from_slice(&40u32.to_le_bytes());
        w[8..12].copy_from_slice(b"WAVE");
        w[12..16].copy_from_slice(b"fmt ");
        w[16..20].copy_from_slice(&16u32.to_le_bytes());
        w[20..22].copy_from_slice(&1u16.to_le_bytes());
        w[22..24].copy_from_slice(&1u16.to_le_bytes());
        w[24..28].copy_from_slice(&16000u32.to_le_bytes());
        w[28..32].copy_from_slice(&32000u32.to_le_bytes());
        w[32..34].copy_from_slice(&2u16.to_le_bytes());
        w[34..36].copy_from_slice(&16u16.to_le_bytes());
        w[36..40].copy_from_slice(b"data");
        w[40..44].copy_from_slice(&4u32.to_le_bytes());
        w[44..46].copy_from_slice(&0i16.to_le_bytes());
        w[46..48].copy_from_slice(&32767i16.to_le_bytes());

        let mut player = AudioPlayer::new();
        let mut out = MockAudio::new();
        player.push_system_id(1);
        player.tick_with(&mut disk, &mut out, 100, || true);
        assert_eq!(out.started_rate, Some(16000));
        assert_eq!(out.count, 2);
        assert_eq!(out.samples[0], 2048);
        assert_eq!(out.samples[1], 4095);

        // Next tick drains & stops
        player.tick_with(&mut disk, &mut out, 100, || true);
        assert!(out.stopped);
    }
}
