//! Dedicated TI CC2500 RF Protocol Catalog & Controller.
//!
//! Strictly contains genuine TI CC2500 2.4 GHz chip protocols (`FrSkyX`, `FrSkyX2`, `FrSkyD`,
//! `FrSkyV`, `SFHSS`, `Redpine`, `Corona`, `Hitec`, `HoTT`, `Skyartec`, `RLINK`), completely
//! separated from the 4-in-1 Multi-Protocol Module catalog in [`crate::rf::mpm`].
//!
//! Uses a flat `O(1)` static descriptor table (`MultiProtoSpec`) and delegates 26-byte MULTI
//! serial framing and 100k `8E2` telemetry decoding to the shared engine in `rf::mpm`.

/// Compact static metadata for a MULTI-compatible RF protocol (`O(1)` table lookup).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MultiProtoSpec {
    pub wire_id: u8,
    pub default_sub: u8,
    pub name: &'static str,
}

// Genuine TI CC2500 MULTI wire protocol IDs
pub const WIRE_FRSKY_D: u8 = 3;
pub const WIRE_FRSKY_X: u8 = 15;
pub const WIRE_SFHSS: u8 = 21;
pub const WIRE_FRSKY_V: u8 = 25;
pub const WIRE_CORONA: u8 = 36;
pub const WIRE_HITEC: u8 = 39;
pub const WIRE_REDPINE: u8 = 50;
pub const WIRE_HOTT: u8 = 57;
pub const WIRE_FRSKY_X2: u8 = 64;
pub const WIRE_RLINK: u8 = 67;
pub const WIRE_SKYARTEC: u8 = 70;

/// Authoritative TI CC2500 protocol table (13 entries, indexed by [`Cc2500Protocol`]).
pub const CC2500_PROTOCOLS: &[MultiProtoSpec] = &[
    MultiProtoSpec { wire_id: WIRE_FRSKY_X, default_sub: 0, name: "FrSkyX" },
    MultiProtoSpec { wire_id: WIRE_FRSKY_D, default_sub: 0, name: "FrSkyD" },
    MultiProtoSpec { wire_id: WIRE_SFHSS, default_sub: 0, name: "SFHSS" },
    MultiProtoSpec { wire_id: WIRE_CORONA, default_sub: 0, name: "Corona" },
    MultiProtoSpec { wire_id: WIRE_HITEC, default_sub: 0, name: "Hitec" },
    MultiProtoSpec { wire_id: WIRE_FRSKY_X, default_sub: 2, name: "FrSkyX LBT" },
    MultiProtoSpec { wire_id: WIRE_HOTT, default_sub: 0, name: "HoTT" },
    MultiProtoSpec { wire_id: WIRE_FRSKY_X2, default_sub: 0, name: "FrSkyX2" },
    MultiProtoSpec { wire_id: WIRE_FRSKY_X2, default_sub: 2, name: "FrSkyX2 LBT" },
    MultiProtoSpec { wire_id: WIRE_FRSKY_V, default_sub: 0, name: "FrSkyV" },
    MultiProtoSpec { wire_id: WIRE_REDPINE, default_sub: 0, name: "Redpine" },
    MultiProtoSpec { wire_id: WIRE_SKYARTEC, default_sub: 0, name: "Skyartec" },
    MultiProtoSpec { wire_id: WIRE_RLINK, default_sub: 0, name: "RLINK" },
];

/// Genuine TI CC2500 protocol selector (stored in `ModelConfig::rx_serial_proto` when `RfProtocol::Cc2500`).
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Cc2500Protocol {
    FrSkyX = 0,
    FrSkyD = 1,
    Sfhss = 2,
    Corona = 3,
    Hitec = 4,
    FrSkyXLbt = 5,
    Hott = 6,
    FrSkyX2 = 7,
    FrSkyX2Lbt = 8,
    FrSkyV = 9,
    Redpine = 10,
    Skyartec = 11,
    Rlink = 12,
}

impl Cc2500Protocol {
    pub const COUNT: u8 = CC2500_PROTOCOLS.len() as u8;

    #[inline(always)]
    pub const fn from_u8(val: u8) -> Self {
        match val {
            0 => Self::FrSkyX,
            1 => Self::FrSkyD,
            2 => Self::Sfhss,
            3 => Self::Corona,
            4 => Self::Hitec,
            5 => Self::FrSkyXLbt,
            6 => Self::Hott,
            7 => Self::FrSkyX2,
            8 => Self::FrSkyX2Lbt,
            9 => Self::FrSkyV,
            10 => Self::Redpine,
            11 => Self::Skyartec,
            12 => Self::Rlink,
            _ => Self::FrSkyX,
        }
    }

    #[inline(always)]
    pub const fn spec(self) -> &'static MultiProtoSpec {
        &CC2500_PROTOCOLS[self as usize]
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
        Self::from_u8((self as u8 + 1) % Self::COUNT)
    }

    #[inline(always)]
    pub const fn prev(self) -> Self {
        Self::from_u8((self as u8 + Self::COUNT - 1) % Self::COUNT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cc2500_table_bounds_and_cycle() {
        assert_eq!(Cc2500Protocol::COUNT, 13);
        let mut p = Cc2500Protocol::FrSkyX;
        for _ in 0..Cc2500Protocol::COUNT {
            assert!(!p.name().is_empty());
            assert!(p.mpm_proto_id() > 0);
            p = p.next();
        }
        assert_eq!(p, Cc2500Protocol::FrSkyX);
        assert_eq!(p.prev(), Cc2500Protocol::Rlink);
        // Clamping out-of-range stored byte safely falls back to FrSkyX
        assert_eq!(Cc2500Protocol::from_u8(99), Cc2500Protocol::FrSkyX);
    }
}
