//! Hardware descriptors and pin configuration traits for Q X7 STM32F407 derivative targets.

use ferrox_core::input::SwitchPos;
use stm32f4::stm32f407 as pac;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Port {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PinDef {
    pub port: Port,
    pub pin: u8,
}

impl PinDef {
    pub const fn new(port: Port, pin: u8) -> Self {
        Self { port, pin }
    }

    #[inline(always)]
    pub fn gpio_ptr(&self) -> *const pac::gpioa::RegisterBlock {
        match self.port {
            Port::A => pac::GPIOA::ptr(),
            Port::B => pac::GPIOB::ptr() as *const _,
            Port::C => pac::GPIOC::ptr() as *const _,
            Port::D => pac::GPIOD::ptr() as *const _,
            Port::E => pac::GPIOE::ptr() as *const _,
            Port::F => pac::GPIOF::ptr() as *const _,
            Port::G => pac::GPIOG::ptr() as *const _,
        }
    }

    #[inline(always)]
    pub fn rcc_ahb1_bit(&self) -> u32 {
        match self.port {
            Port::A => 1 << 0,
            Port::B => 1 << 1,
            Port::C => 1 << 2,
            Port::D => 1 << 3,
            Port::E => 1 << 4,
            Port::F => 1 << 5,
            Port::G => 1 << 6,
        }
    }

    pub fn enable_clock_and_input_pullup(&self) {
        unsafe {
            let rcc = &*pac::RCC::ptr();
            rcc.ahb1enr
                .modify(|r, w| w.bits(r.bits() | self.rcc_ahb1_bit()));
            let gpio = &*self.gpio_ptr();
            let shift = self.pin * 2;
            gpio.moder.modify(|r, w| w.bits(r.bits() & !(0x03 << shift)));
            gpio.pupdr
                .modify(|r, w| w.bits((r.bits() & !(0x03 << shift)) | (0x01 << shift)));
        }
    }

    pub fn enable_clock_and_output(&self, default_high: bool) {
        unsafe {
            let rcc = &*pac::RCC::ptr();
            rcc.ahb1enr
                .modify(|r, w| w.bits(r.bits() | self.rcc_ahb1_bit()));
            let gpio = &*self.gpio_ptr();
            let shift = self.pin * 2;
            gpio.moder
                .modify(|r, w| w.bits((r.bits() & !(0x03 << shift)) | (0x01 << shift)));
            if default_high {
                gpio.bsrr.write(|w| w.bits(1 << self.pin));
            } else {
                gpio.bsrr.write(|w| w.bits(1 << (self.pin + 16)));
            }
        }
    }

    #[inline(always)]
    pub fn is_low(&self) -> bool {
        unsafe {
            let gpio = &*self.gpio_ptr();
            (gpio.idr.read().bits() & (1 << self.pin)) == 0
        }
    }

    #[inline(always)]
    pub fn set_high(&self) {
        unsafe {
            let gpio = &*self.gpio_ptr();
            gpio.bsrr.write(|w| w.bits(1 << self.pin));
        }
    }

    #[inline(always)]
    pub fn set_low(&self) {
        unsafe {
            let gpio = &*self.gpio_ptr();
            gpio.bsrr.write(|w| w.bits(1 << (self.pin + 16)));
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SwitchDef {
    None,
    TwoPos(PinDef),
    ThreePos { high: PinDef, low: PinDef },
}

impl SwitchDef {
    pub fn init(&self) {
        match self {
            SwitchDef::None => {}
            SwitchDef::TwoPos(pin) => pin.enable_clock_and_input_pullup(),
            SwitchDef::ThreePos { high, low } => {
                high.enable_clock_and_input_pullup();
                low.enable_clock_and_input_pullup();
            }
        }
    }

    #[inline]
    pub fn read_virtual_adc(&self) -> u16 {
        match self {
            SwitchDef::None => 2048,
            SwitchDef::TwoPos(pin) => {
                if pin.is_low() {
                    4000
                } else {
                    100
                }
            }
            SwitchDef::ThreePos { high, low } => {
                if high.is_low() {
                    100
                } else if low.is_low() {
                    4000
                } else {
                    2048
                }
            }
        }
    }

    #[inline]
    pub fn read_switch_pos(&self) -> SwitchPos {
        match self {
            SwitchDef::None => SwitchPos::Up,
            SwitchDef::TwoPos(pin) => {
                if pin.is_low() {
                    SwitchPos::Down
                } else {
                    SwitchPos::Up
                }
            }
            SwitchDef::ThreePos { high, low } => {
                if high.is_low() {
                    SwitchPos::Up
                } else if low.is_low() {
                    SwitchPos::Down
                } else {
                    SwitchPos::Mid
                }
            }
        }
    }
}

/// Shared trait defining hardware pinouts and inversions for STM32F407 Q X7 family boards.
pub trait Qx7HalDescriptor: 'static {
    /// Invert stick axes: `[RV, RH, LV, LH]`.
    const STICK_INVERT: [bool; 4] = [false, true, false, true];

    /// Invert S1 (P1) potentiometer.
    const S1_INVERT: bool = true;

    /// Whether S2 (P2) potentiometer is physically present.
    const HAS_S2: bool = true;

    /// Invert S2 (P2) potentiometer.
    const S2_INVERT: bool = false;

    /// Swap S1 and S2 ADC channels (e.g. Jumper T-Pro V2).
    const SWAP_S1_S2_ADC: bool = false;

    /// LCD scan inversions.
    const LCD_VERT_INVERT: bool = true;
    const LCD_HORIZ_INVERT: bool = false;
    const LCD_CONTRAST_OFFSET: i8 = 0;

    /// EEPROM pins.
    const EEPROM_SCL: PinDef = PinDef::new(Port::B, 8);
    const EEPROM_SDA: PinDef = PinDef::new(Port::B, 9);
    const EEPROM_WP: PinDef = PinDef::new(Port::D, 10);

    /// Rotary encoder secondary pin (PE10 on RM, PE11 on QX7/Jumper).
    const ROTENC_PIN_B: u8 = 10;
    const ROTENC_INVERT: bool = true;

    /// Navigation keys.
    const KEY_ENTER: PinDef = PinDef::new(Port::A, 13);
    const KEY_EXIT: PinDef = PinDef::new(Port::C, 5);
    const KEY_PAGE_PREV: PinDef = PinDef::new(Port::D, 3);
    const KEY_PAGE_NEXT: PinDef = PinDef::new(Port::D, 7);

    /// 6-button navigation keys (None on 3-button QX7/T-Pro V2).
    const KEY_SYS: Option<PinDef> = Some(PinDef::new(Port::B, 4));
    const KEY_MDL: Option<PinDef> = Some(PinDef::new(Port::E, 11));
    const KEY_TELE: Option<PinDef> = Some(PinDef::new(Port::D, 2));

    /// Trim switches `(Dec, Inc)`.
    const TRIM_T1: (PinDef, PinDef) = (PinDef::new(Port::D, 15), PinDef::new(Port::C, 1));
    const TRIM_T2: (PinDef, PinDef) = (PinDef::new(Port::E, 6), PinDef::new(Port::E, 5));
    const TRIM_T3: (PinDef, PinDef) = (PinDef::new(Port::C, 3), PinDef::new(Port::C, 2));
    const TRIM_T4: (PinDef, PinDef) = (PinDef::new(Port::E, 3), PinDef::new(Port::E, 4));

    /// Primary switches SA..SD.
    const SW_SA: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::C, 13));
    const SW_SB: SwitchDef = SwitchDef::ThreePos {
        high: PinDef::new(Port::A, 5),
        low: PinDef::new(Port::E, 15),
    };
    const SW_SC: SwitchDef = SwitchDef::ThreePos {
        high: PinDef::new(Port::D, 11),
        low: PinDef::new(Port::E, 0),
    };
    const SW_SD: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::E, 8));

    /// Extended switches SE..SH.
    const SW_SE: SwitchDef = SwitchDef::None;
    const SW_SF: SwitchDef = SwitchDef::None;
    const SW_SG: SwitchDef = SwitchDef::None;
    const SW_SH: SwitchDef = SwitchDef::None;

    /// Status LED pin.
    const LED_STATUS: PinDef = PinDef::new(Port::E, 2);

    /// Internal RF power and bootloader control pins.
    const INT_RF_PWR: PinDef = PinDef::new(Port::C, 4);
    const INT_RF_BOOTCMD: Option<PinDef> = Some(PinDef::new(Port::B, 1));
}

// ============================================================================
// Derivative HAL Descriptors
// ============================================================================

/// RadioMaster Pocket HAL Descriptor.
#[derive(Copy, Clone, Debug, Default)]
pub struct PocketHal;

impl Qx7HalDescriptor for PocketHal {
    const HAS_S2: bool = false;
    const SW_SE: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::E, 7));
}

/// RadioMaster Boxer HAL Descriptor.
#[derive(Copy, Clone, Debug, Default)]
pub struct BoxerHal;

impl Qx7HalDescriptor for BoxerHal {
    const STICK_INVERT: [bool; 4] = [false, true, true, false];
    const S2_INVERT: bool = true;
    const KEY_ENTER: PinDef = PinDef::new(Port::A, 10);
    const SW_SB: SwitchDef = SwitchDef::ThreePos {
        high: PinDef::new(Port::E, 7),
        low: PinDef::new(Port::E, 15),
    };
    const SW_SE: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::E, 14));
    const SW_SF: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::E, 1));
    const LED_STATUS: PinDef = PinDef::new(Port::A, 7);
}

/// RadioMaster Zorro HAL Descriptor.
#[derive(Copy, Clone, Debug, Default)]
pub struct ZorroHal;

impl Qx7HalDescriptor for ZorroHal {
    const SW_SE: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::E, 7));
    const SW_SF: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::E, 1));
    const SW_SG: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::E, 14));
    const SW_SH: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::D, 14));
}

/// RadioMaster TX12 MKII HAL Descriptor.
#[derive(Copy, Clone, Debug, Default)]
pub struct Tx12Hal;

impl Qx7HalDescriptor for Tx12Hal {
    const STICK_INVERT: [bool; 4] = [true, false, false, true];
    const S1_INVERT: bool = false;
    const S2_INVERT: bool = true;
    const KEY_ENTER: PinDef = PinDef::new(Port::A, 10);
    const SW_SE: SwitchDef = SwitchDef::ThreePos {
        high: PinDef::new(Port::E, 14),
        low: PinDef::new(Port::E, 7),
    };
    const SW_SF: SwitchDef = SwitchDef::ThreePos {
        high: PinDef::new(Port::E, 1),
        low: PinDef::new(Port::D, 14),
    };
    const LED_STATUS: PinDef = PinDef::new(Port::A, 7);
}

/// FrSky Taranis Q X7 / X7S ACCESS HAL Descriptor.
#[derive(Copy, Clone, Debug, Default)]
pub struct Qx7Hal;

impl Qx7HalDescriptor for Qx7Hal {
    const STICK_INVERT: [bool; 4] = [true, false, false, true];
    const S1_INVERT: bool = false;
    const S2_INVERT: bool = false;
    const LCD_VERT_INVERT: bool = false;
    const LCD_HORIZ_INVERT: bool = false;
    const EEPROM_WP: PinDef = PinDef::new(Port::B, 5);
    const ROTENC_PIN_B: u8 = 11;
    const ROTENC_INVERT: bool = false;
    const KEY_ENTER: PinDef = PinDef::new(Port::E, 10);
    const KEY_EXIT: PinDef = PinDef::new(Port::D, 2);
    const KEY_PAGE_PREV: PinDef = PinDef::new(Port::D, 7); // MENU
    const KEY_PAGE_NEXT: PinDef = PinDef::new(Port::D, 3); // PAGE
    const KEY_SYS: Option<PinDef> = None;
    const KEY_MDL: Option<PinDef> = None;
    const KEY_TELE: Option<PinDef> = None;
    const TRIM_T1: (PinDef, PinDef) = (PinDef::new(Port::C, 1), PinDef::new(Port::D, 15));
    const TRIM_T4: (PinDef, PinDef) = (PinDef::new(Port::E, 4), PinDef::new(Port::E, 3));
    const SW_SA: SwitchDef = SwitchDef::ThreePos {
        high: PinDef::new(Port::E, 13),
        low: PinDef::new(Port::E, 7),
    };
    const SW_SC: SwitchDef = SwitchDef::ThreePos {
        high: PinDef::new(Port::D, 10),
        low: PinDef::new(Port::D, 11),
    };
    const SW_SD: SwitchDef = SwitchDef::ThreePos {
        high: PinDef::new(Port::E, 2),
        low: PinDef::new(Port::E, 1),
    };
    const SW_SF: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::E, 14));
    const SW_SH: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::D, 14));
    const LED_STATUS: PinDef = PinDef::new(Port::B, 1);
    const INT_RF_PWR: PinDef = PinDef::new(Port::C, 5);
    const INT_RF_BOOTCMD: Option<PinDef> = None;
}

/// Jumper T-Pro V2 HAL Descriptor.
#[derive(Copy, Clone, Debug, Default)]
pub struct TProV2Hal;

impl Qx7HalDescriptor for TProV2Hal {
    const STICK_INVERT: [bool; 4] = [false, true, true, false];
    const SWAP_S1_S2_ADC: bool = true;
    const LCD_VERT_INVERT: bool = true;
    const LCD_HORIZ_INVERT: bool = true;
    const EEPROM_SCL: PinDef = PinDef::new(Port::B, 6);
    const EEPROM_SDA: PinDef = PinDef::new(Port::B, 7);
    const EEPROM_WP: PinDef = PinDef::new(Port::B, 9);
    const ROTENC_PIN_B: u8 = 11;
    const ROTENC_INVERT: bool = false;
    const KEY_ENTER: PinDef = PinDef::new(Port::E, 10);
    const KEY_EXIT: PinDef = PinDef::new(Port::D, 2);
    const KEY_PAGE_PREV: PinDef = PinDef::new(Port::D, 7);
    const KEY_PAGE_NEXT: PinDef = PinDef::new(Port::D, 3);
    const KEY_SYS: Option<PinDef> = None;
    const KEY_MDL: Option<PinDef> = None;
    const KEY_TELE: Option<PinDef> = None;
    const SW_SA: SwitchDef = SwitchDef::ThreePos {
        high: PinDef::new(Port::E, 7),
        low: PinDef::new(Port::E, 13),
    };
    const SW_SB: SwitchDef = SwitchDef::ThreePos {
        high: PinDef::new(Port::E, 2),
        low: PinDef::new(Port::E, 1),
    };
    const SW_SC: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::E, 14));
    const SW_SD: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::D, 14));
    const SW_SE: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::G, 0));
    const SW_SF: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::G, 1));
    const SW_SG: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::G, 10));
    const SW_SH: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::G, 9));
    const LED_STATUS: PinDef = PinDef::new(Port::B, 1);
    const INT_RF_PWR: PinDef = PinDef::new(Port::C, 6);
    const INT_RF_BOOTCMD: Option<PinDef> = None;
}
