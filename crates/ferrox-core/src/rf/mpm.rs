//! Multi-Protocol Module (MPM) and CC2500 Serial Protocol Driver.
//!
//! Controls internal and external 4-in-1 / CC2500 MULTI-Modules via 100,000 baud 8E2 serial stream.
//! Supports FrSky D16 (FCC & LBT), FrSky D8, Futaba SFHSS, Corona, and Hitec protocols
//! with frequency fine-tuning and bidirectional telemetry decoding.

use crate::mixer::NUM_CHANNELS;

/// MULTI-Module Protocol IDs for CC2500 transceivers:
pub const MPM_PROTO_FRSKY_D: u8 = 3; // FrSky D8
pub const MPM_PROTO_FRSKY_X: u8 = 15; // FrSky D16 / X (FCC & LBT)
pub const MPM_PROTO_SFHSS: u8 = 21; // Futaba SFHSS
pub const MPM_PROTO_FRSKY_V: u8 = 25; // FrSky V8
pub const MPM_PROTO_CORONA: u8 = 36; // Corona
pub const MPM_PROTO_HITEC: u8 = 39; // Hitec Optima/Minima

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Cc2500Protocol {
    FrSkyD16 = 0,
    FrSkyD8 = 1,
    Sfhss = 2,
    Corona = 3,
    Hitec = 4,
}

impl Cc2500Protocol {
    pub const fn from_u8(val: u8) -> Self {
        match val {
            0 => Self::FrSkyD16,
            1 => Self::FrSkyD8,
            2 => Self::Sfhss,
            3 => Self::Corona,
            4 => Self::Hitec,
            _ => Self::FrSkyD16,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::FrSkyD16 => "FRSKY D16",
            Self::FrSkyD8 => "FRSKY D8",
            Self::Sfhss => "FUTABA SFHSS",
            Self::Corona => "CORONA",
            Self::Hitec => "HITEC",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::FrSkyD16 => Self::FrSkyD8,
            Self::FrSkyD8 => Self::Sfhss,
            Self::Sfhss => Self::Corona,
            Self::Corona => Self::Hitec,
            Self::Hitec => Self::FrSkyD16,
        }
    }

    pub const fn prev(self) -> Self {
        match self {
            Self::FrSkyD16 => Self::Hitec,
            Self::FrSkyD8 => Self::FrSkyD16,
            Self::Sfhss => Self::FrSkyD8,
            Self::Corona => Self::Sfhss,
            Self::Hitec => Self::Corona,
        }
    }

    pub const fn mpm_proto_id(self) -> u8 {
        match self {
            Self::FrSkyD16 => MPM_PROTO_FRSKY_X,
            Self::FrSkyD8 => MPM_PROTO_FRSKY_D,
            Self::Sfhss => MPM_PROTO_SFHSS,
            Self::Corona => MPM_PROTO_CORONA,
            Self::Hitec => MPM_PROTO_HITEC,
        }
    }
}

/// Decoded Telemetry state from MPM / CC2500 S.Port or Hub stream.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct MpmTelemetry {
    pub connected: bool,
    pub rssi: u8,
    pub lq: u8,
    pub rx_batt_mv: u16,
    pub alt_m: i16,
    pub vspeed_cms: i16,
}

/// Format a 26-byte MULTI-Module CC2500 serial frame (100,000 baud 8E2):
/// - Byte 0: `0x55` (Stream synchronization header)
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
    }
}
