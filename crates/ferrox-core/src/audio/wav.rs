//! Zero-heap WAV/PCM header parser and 12-bit DAC sample converter.
//!
//! Supports uncompressed PCM (`audio_format == 1`), 8-bit unsigned or 16-bit signed LE,
//! mono or stereo (downmixed to mono), at standard EdgeTX/OpenTX sample rates (8-48 kHz).

/// WAV parsing error.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum WavError {
    InvalidHeader,
    UnsupportedFormat,
    Truncated,
}

/// Parsed WAV PCM stream parameters.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct WavFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    /// Byte offset from start of file where `data` chunk payload begins.
    pub data_offset: usize,
    /// Total byte length of PCM payload in the `data` chunk.
    pub data_size: u32,
}

impl WavFormat {
    /// Bytes per complete frame (all channels for one sample tick).
    #[inline]
    pub const fn bytes_per_frame(&self) -> usize {
        (self.channels as usize) * ((self.bits_per_sample as usize) / 8)
    }
}

/// Parse a WAV RIFF header from the first sector (or slice) of a WAV file.
pub fn parse_wav_header(buf: &[u8]) -> Result<WavFormat, WavError> {
    if buf.len() < 44 {
        return Err(WavError::Truncated);
    }
    if &buf[0..4] != b"RIFF" || &buf[8..12] != b"WAVE" {
        return Err(WavError::InvalidHeader);
    }

    let mut pos = 12usize;
    let mut found_fmt = false;
    let mut sample_rate = 0u32;
    let mut channels = 0u16;
    let mut bits_per_sample = 0u16;

    while pos + 8 <= buf.len() {
        let chunk_id = &buf[pos..pos + 4];
        let chunk_size =
            u32::from_le_bytes([buf[pos + 4], buf[pos + 5], buf[pos + 6], buf[pos + 7]]);
        let payload_start = pos + 8;

        if chunk_id == b"fmt " {
            if chunk_size < 16 || payload_start + 16 > buf.len() {
                return Err(WavError::InvalidHeader);
            }
            let audio_format = u16::from_le_bytes([buf[payload_start], buf[payload_start + 1]]);
            channels = u16::from_le_bytes([buf[payload_start + 2], buf[payload_start + 3]]);
            sample_rate = u32::from_le_bytes([
                buf[payload_start + 4],
                buf[payload_start + 5],
                buf[payload_start + 6],
                buf[payload_start + 7],
            ]);
            bits_per_sample =
                u16::from_le_bytes([buf[payload_start + 14], buf[payload_start + 15]]);

            if audio_format != 1
                || !matches!(channels, 1 | 2)
                || !matches!(bits_per_sample, 8 | 16)
                || !(4000..=48000).contains(&sample_rate)
            {
                return Err(WavError::UnsupportedFormat);
            }
            found_fmt = true;
        } else if chunk_id == b"data" {
            if !found_fmt {
                return Err(WavError::InvalidHeader);
            }
            return Ok(WavFormat {
                sample_rate,
                channels,
                bits_per_sample,
                data_offset: payload_start,
                data_size: chunk_size,
            });
        }

        // RIFF chunks are word-aligned (padded to even byte count).
        let padded = (chunk_size as usize).saturating_add(chunk_size as usize & 1);
        pos = payload_start.saturating_add(padded);
    }

    Err(WavError::Truncated)
}

/// Convert raw PCM bytes into 12-bit unsigned DAC samples (`0..=4095`, midpoint `2048`)
/// scaled by `volume_pct` (`0..=100`).
///
/// Returns `(samples_written, bytes_consumed)`.
pub fn decode_pcm_to_dac12(
    src: &[u8],
    fmt: &WavFormat,
    volume_pct: u8,
    out: &mut [u16],
) -> (usize, usize) {
    let bpf = fmt.bytes_per_frame();
    if bpf == 0 || out.is_empty() || src.len() < bpf {
        return (0, 0);
    }

    let vol = volume_pct.min(100) as i32;
    let max_samples = (src.len() / bpf).min(out.len());
    let mut s_idx = 0usize;
    let mut b_idx = 0usize;

    match (fmt.bits_per_sample, fmt.channels) {
        (16, 1) => {
            while s_idx < max_samples {
                let s16 = i16::from_le_bytes([src[b_idx], src[b_idx + 1]]) as i32;
                let scaled = (s16 * vol) / 100;
                // Map -32768..=32767 -> 0..=4095 (midpoint 2048)
                out[s_idx] = ((scaled + 32768) >> 4).clamp(0, 4095) as u16;
                s_idx += 1;
                b_idx += 2;
            }
        }
        (16, 2) => {
            while s_idx < max_samples {
                let l = i16::from_le_bytes([src[b_idx], src[b_idx + 1]]) as i32;
                let r = i16::from_le_bytes([src[b_idx + 2], src[b_idx + 3]]) as i32;
                let mono = (l + r) / 2;
                let scaled = (mono * vol) / 100;
                out[s_idx] = ((scaled + 32768) >> 4).clamp(0, 4095) as u16;
                s_idx += 1;
                b_idx += 4;
            }
        }
        (8, 1) => {
            while s_idx < max_samples {
                let s8 = (src[b_idx] as i32) - 128;
                let scaled = (s8 * vol) / 100;
                out[s_idx] = ((scaled + 128) << 4).clamp(0, 4095) as u16;
                s_idx += 1;
                b_idx += 1;
            }
        }
        (8, 2) => {
            while s_idx < max_samples {
                let l = (src[b_idx] as i32) - 128;
                let r = (src[b_idx + 1] as i32) - 128;
                let mono = (l + r) / 2;
                let scaled = (mono * vol) / 100;
                out[s_idx] = ((scaled + 128) << 4).clamp(0, 4095) as u16;
                s_idx += 1;
                b_idx += 2;
            }
        }
        _ => return (0, 0),
    }

    (s_idx, b_idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_wav_header(sr: u32, ch: u16, bps: u16, data_len: u32) -> [u8; 44] {
        let mut h = [0u8; 44];
        h[0..4].copy_from_slice(b"RIFF");
        h[4..8].copy_from_slice(&(36 + data_len).to_le_bytes());
        h[8..12].copy_from_slice(b"WAVE");
        h[12..16].copy_from_slice(b"fmt ");
        h[16..20].copy_from_slice(&16u32.to_le_bytes());
        h[20..22].copy_from_slice(&1u16.to_le_bytes()); // PCM
        h[22..24].copy_from_slice(&ch.to_le_bytes());
        h[24..28].copy_from_slice(&sr.to_le_bytes());
        let byte_rate = sr * ch as u32 * (bps as u32 / 8);
        h[28..32].copy_from_slice(&byte_rate.to_le_bytes());
        let block_align = ch * (bps / 8);
        h[32..34].copy_from_slice(&block_align.to_le_bytes());
        h[34..36].copy_from_slice(&bps.to_le_bytes());
        h[36..40].copy_from_slice(b"data");
        h[40..44].copy_from_slice(&data_len.to_le_bytes());
        h
    }

    #[test]
    fn parse_valid_16bit_mono_wav() {
        let hdr = make_wav_header(16000, 1, 16, 3200);
        let fmt = parse_wav_header(&hdr).unwrap();
        assert_eq!(fmt.sample_rate, 16000);
        assert_eq!(fmt.channels, 1);
        assert_eq!(fmt.bits_per_sample, 16);
        assert_eq!(fmt.data_offset, 44);
        assert_eq!(fmt.data_size, 3200);
    }

    #[test]
    fn decode_16bit_mono_silence_and_peaks() {
        let fmt = WavFormat {
            sample_rate: 16000,
            channels: 1,
            bits_per_sample: 16,
            data_offset: 44,
            data_size: 6,
        };
        let mut pcm = [0u8; 6];
        pcm[0..2].copy_from_slice(&0i16.to_le_bytes());
        pcm[2..4].copy_from_slice(&32767i16.to_le_bytes());
        pcm[4..6].copy_from_slice(&(-32768i16).to_le_bytes());

        let mut out = [0u16; 3];
        let (n, used) = decode_pcm_to_dac12(&pcm, &fmt, 100, &mut out);
        assert_eq!((n, used), (3, 6));
        assert_eq!(out[0], 2048);
        assert_eq!(out[1], 4095);
        assert_eq!(out[2], 0);
    }

    #[test]
    fn decode_volume_half_attenuates_toward_midpoint() {
        let fmt = WavFormat {
            sample_rate: 16000,
            channels: 1,
            bits_per_sample: 8,
            data_offset: 44,
            data_size: 2,
        };
        let pcm = [255u8, 0u8];
        let mut out = [0u16; 2];
        let (n, _) = decode_pcm_to_dac12(&pcm, &fmt, 50, &mut out);
        assert_eq!(n, 2);
        // 50% volume: 127*50/100 = 63 -> (63+128)*16 = 3056; -128*50/100 = -64 -> (64)*16 = 1024
        assert_eq!(out[0], 3056);
        assert_eq!(out[1], 1024);
    }
}
