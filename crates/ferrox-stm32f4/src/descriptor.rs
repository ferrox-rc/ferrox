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
            let _ = rcc.ahb1enr.read().bits();
            let gpio = &*self.gpio_ptr();
            let shift = self.pin * 2;
            gpio.moder
                .modify(|r, w| w.bits(r.bits() & !(0x03 << shift)));
            gpio.pupdr
                .modify(|r, w| w.bits((r.bits() & !(0x03 << shift)) | (0x01 << shift)));
        }
    }

    pub fn enable_clock_and_output(&self, default_high: bool) {
        unsafe {
            let rcc = &*pac::RCC::ptr();
            rcc.ahb1enr
                .modify(|r, w| w.bits(r.bits() | self.rcc_ahb1_bit()));
            let _ = rcc.ahb1enr.read().bits();
            let gpio = &*self.gpio_ptr();
            if default_high {
                gpio.bsrr.write(|w| w.bits(1 << self.pin));
            } else {
                gpio.bsrr.write(|w| w.bits(1 << (self.pin + 16)));
            }
            let shift = self.pin * 2;
            gpio.moder
                .modify(|r, w| w.bits((r.bits() & !(0x03 << shift)) | (0x01 << shift)));
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

    /// MicroSD card SPI2 pins.
    const SD_CS: PinDef = PinDef::new(Port::B, 12);
    const SD_SCK: PinDef = PinDef::new(Port::B, 13);
    const SD_MISO: PinDef = PinDef::new(Port::B, 14);
    const SD_MOSI: PinDef = PinDef::new(Port::B, 15);
    const SD_DETECT: PinDef = PinDef::new(Port::D, 9);

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

    /// Soft-power latch output pin (`PD0` on Q X7/Pocket/Zorro, `PD1` on GX12).
    const PWR_ON_PIN: PinDef = PinDef::new(Port::D, 0);
    /// Soft-power button input pin (`PD1` on Q X7/Pocket/Zorro, `PD0` on GX12).
    const PWR_SWITCH_PIN: PinDef = PinDef::new(Port::D, 1);

    /// True if display controller is a 128x64 SSD1309 SPI OLED (GX12) instead of ST7567 LCD.
    const IS_OLED_SSD1309: bool = false;

    /// True if switches SA..SH are attached to PCA9555/TCA9539 I2C2 expanders on PB10/PB11 (GX12).
    const HAS_PCA9555_SWITCHES: bool = false;

    /// Status LED pin.
    const LED_STATUS: Option<PinDef> = Some(PinDef::new(Port::E, 2));

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
    const LED_STATUS: Option<PinDef> = Some(PinDef::new(Port::A, 7));
}

/// RadioMaster Zorro HAL Descriptor.
#[derive(Copy, Clone, Debug, Default)]
pub struct ZorroHal;

impl Qx7HalDescriptor for ZorroHal {
    const ROTENC_INVERT: bool = false;
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
    const LED_STATUS: Option<PinDef> = Some(PinDef::new(Port::A, 7));
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
    const LED_STATUS: Option<PinDef> = Some(PinDef::new(Port::B, 1));
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
    const LED_STATUS: Option<PinDef> = Some(PinDef::new(Port::B, 1));
    const INT_RF_PWR: PinDef = PinDef::new(Port::C, 6);
    const INT_RF_BOOTCMD: Option<PinDef> = None;
}

/// Jumper T12 HAL Descriptor.
#[derive(Copy, Clone, Debug, Default)]
pub struct T12Hal;

impl Qx7HalDescriptor for T12Hal {
    const STICK_INVERT: [bool; 4] = [false, true, false, true];
    const S1_INVERT: bool = false;
    const S2_INVERT: bool = false;
    const LCD_VERT_INVERT: bool = true;
    const LCD_HORIZ_INVERT: bool = false;
    const SW_SE: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::E, 14));
    const SW_SF: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::E, 1));
}

/// FrSky Taranis X9D+ HAL Descriptor.
#[derive(Copy, Clone, Debug, Default)]
pub struct X9dHal;

impl Qx7HalDescriptor for X9dHal {
    const STICK_INVERT: [bool; 4] = [true, false, false, true];
    const S1_INVERT: bool = false;
    const S2_INVERT: bool = false;
    const LCD_VERT_INVERT: bool = false;
    const LCD_HORIZ_INVERT: bool = false;
    const SW_SE: SwitchDef = SwitchDef::ThreePos {
        high: PinDef::new(Port::E, 14),
        low: PinDef::new(Port::E, 7),
    };
    const SW_SF: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::E, 1));
    const SW_SG: SwitchDef = SwitchDef::ThreePos {
        high: PinDef::new(Port::E, 2),
        low: PinDef::new(Port::E, 0),
    };
    const SW_SH: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::D, 14));
    const LED_STATUS: Option<PinDef> = None;
}

/// FrSky Taranis X9E Tray Radio HAL Descriptor.
#[derive(Copy, Clone, Debug, Default)]
pub struct X9eHal;

impl Qx7HalDescriptor for X9eHal {
    const STICK_INVERT: [bool; 4] = [true, false, false, true];
    const S1_INVERT: bool = false;
    const S2_INVERT: bool = false;
    const LCD_VERT_INVERT: bool = false;
    const LCD_HORIZ_INVERT: bool = false;
    const SW_SE: SwitchDef = SwitchDef::ThreePos {
        high: PinDef::new(Port::E, 14),
        low: PinDef::new(Port::E, 7),
    };
    const SW_SF: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::E, 1));
    const SW_SG: SwitchDef = SwitchDef::ThreePos {
        high: PinDef::new(Port::E, 2),
        low: PinDef::new(Port::E, 0),
    };
    const SW_SH: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::D, 14));
    const LED_STATUS: Option<PinDef> = None;
}

/// RadioLink AT9S Pro HAL Descriptor.
#[derive(Copy, Clone, Debug, Default)]
pub struct At9sProHal;

impl Qx7HalDescriptor for At9sProHal {
    const STICK_INVERT: [bool; 4] = [false, true, false, true];
    const S1_INVERT: bool = false;
    const S2_INVERT: bool = false;
    const LCD_VERT_INVERT: bool = false;
    const LCD_HORIZ_INVERT: bool = false;
    const ROTENC_PIN_B: u8 = 11;
    const ROTENC_INVERT: bool = false;
    const KEY_MDL: Option<PinDef> = None;
    const SW_SE: SwitchDef = SwitchDef::ThreePos {
        high: PinDef::new(Port::E, 14),
        low: PinDef::new(Port::E, 7),
    };
    const SW_SF: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::E, 1));
    const SW_SG: SwitchDef = SwitchDef::ThreePos {
        high: PinDef::new(Port::E, 2),
        low: PinDef::new(Port::E, 0),
    };
    const SW_SH: SwitchDef = SwitchDef::TwoPos(PinDef::new(Port::D, 14));
    const LED_STATUS: Option<PinDef> = None;
}

/// RadioMaster GX12 HAL Descriptor.
#[derive(Copy, Clone, Debug, Default)]
pub struct Gx12Hal;

impl Qx7HalDescriptor for Gx12Hal {
    const STICK_INVERT: [bool; 4] = [false, true, true, false];
    const S1_INVERT: bool = false;
    const S2_INVERT: bool = true;
    const LCD_VERT_INVERT: bool = false;
    const LCD_HORIZ_INVERT: bool = false;
    const IS_OLED_SSD1309: bool = true;
    const HAS_PCA9555_SWITCHES: bool = true;
    const PWR_ON_PIN: PinDef = PinDef::new(Port::D, 1);
    const PWR_SWITCH_PIN: PinDef = PinDef::new(Port::D, 0);
    const INT_RF_PWR: PinDef = PinDef::new(Port::E, 0);
    const KEY_ENTER: PinDef = PinDef::new(Port::A, 10);
    const SW_SA: SwitchDef = SwitchDef::None;
    const SW_SB: SwitchDef = SwitchDef::None;
    const SW_SC: SwitchDef = SwitchDef::None;
    const SW_SD: SwitchDef = SwitchDef::None;
    const SW_SE: SwitchDef = SwitchDef::None;
    const SW_SF: SwitchDef = SwitchDef::None;
    const SW_SG: SwitchDef = SwitchDef::None;
    const SW_SH: SwitchDef = SwitchDef::None;
    const LED_STATUS: Option<PinDef> = Some(PinDef::new(Port::A, 7));
}

/// PCA9555 / TCA9539 I2C2 (PB10 SCL, PB11 SDA) 16-bit GPIO switch expander driver for GX12.
pub mod pca9555 {
    use super::*;

    const PCA9555_ADDR_PRIMARY: u8 = 0x74;
    const PCA9555_ADDR_ALT: u8 = 0x75;
    const I2C_TIMEOUT: u32 = 15_000;

    static mut CACHED_PORTS: u16 = 0xFFFF;
    static mut DETECTED_ADDR: u8 = 0;

    /// Initialize I2C2 on PB10 (SCL, AF4) and PB11 (SDA, AF4) at 400 kHz Fast Mode (APB1 = 42 MHz).
    pub fn init() {
        unsafe {
            let rcc = &*pac::RCC::ptr();
            let gpiob = &*pac::GPIOB::ptr();
            let i2c2 = &*pac::I2C2::ptr();

            // 1. Enable GPIOB (bit 1) and I2C2 (APB1 bit 22) clocks with read-back (ES0182 2.1.13)
            rcc.ahb1enr.modify(|r, w| w.bits(r.bits() | (1 << 1)));
            let _ = rcc.ahb1enr.read().bits();
            rcc.apb1enr.modify(|r, w| w.bits(r.bits() | (1 << 22)));
            let _ = rcc.apb1enr.read().bits();

            // 2. Configure PB10 (SCL) and PB11 (SDA): AF4, Open-Drain, Pull-Up, Very High Speed
            gpiob
                .otyper
                .modify(|r, w| w.bits(r.bits() | (1 << 10) | (1 << 11)));
            gpiob
                .ospeedr
                .modify(|r, w| w.bits(r.bits() | (0x03 << 20) | (0x03 << 22)));
            gpiob.pupdr.modify(|r, w| {
                w.bits((r.bits() & !((0x03 << 20) | (0x03 << 22))) | (0x01 << 20) | (0x01 << 22))
            });
            gpiob.afrh.modify(|r, w| {
                let mask = (0x0F << 8) | (0x0F << 12);
                let af4 = (0x04 << 8) | (0x04 << 12);
                w.bits((r.bits() & !mask) | af4)
            });
            gpiob.moder.modify(|r, w| {
                w.bits((r.bits() & !((0x03 << 20) | (0x03 << 22))) | (0x02 << 20) | (0x02 << 22))
            });

            // 3. Reset and configure I2C2 for 400 kHz Fast Mode (42 MHz APB1)
            i2c2.cr1.write(|w| w.bits(1 << 15)); // SWRST = 1
            i2c2.cr1.write(|w| w.bits(0));
            i2c2.cr2.write(|w| w.bits(42)); // FREQ = 42 MHz
            i2c2.ccr.write(|w| w.bits((1 << 15) | 35)); // F/S = 1, CCR = 35 (400 kHz)
            i2c2.trise.write(|w| w.bits(14));
            i2c2.cr1.write(|w| w.bits(1 << 0)); // PE = 1

            // Probe primary (0x74) or secondary (0x75) expander
            if let Some(val) = read_ports_raw(PCA9555_ADDR_PRIMARY) {
                DETECTED_ADDR = PCA9555_ADDR_PRIMARY;
                CACHED_PORTS = val;
            } else if let Some(val) = read_ports_raw(PCA9555_ADDR_ALT) {
                DETECTED_ADDR = PCA9555_ADDR_ALT;
                CACHED_PORTS = val;
            }
        }
    }

    #[inline]
    unsafe fn abort_i2c2(i2c2: &pac::i2c1::RegisterBlock) {
        i2c2.sr1.write(|w| w.bits(0));
        i2c2.cr1
            .modify(|r, w| w.bits((r.bits() & !(1 << 11)) | (1 << 9))); // POS = 0, STOP = 1
    }

    /// Perform RM0090 27.3.3 2-byte POS=1 Master Receiver read of Input Port 0 & 1 (`cmd = 0x00`).
    fn read_ports_raw(addr7: u8) -> Option<u16> {
        let i2c2 = unsafe { &*pac::I2C2::ptr() };
        unsafe {
            // Clear any sticky error flags (BERR, ARLO, AF, OVR)
            i2c2.sr1.write(|w| w.bits(0));

            // 1. Generate START
            i2c2.cr1.modify(|r, w| w.bits(r.bits() | (1 << 8)));
            let mut t = I2C_TIMEOUT;
            while (i2c2.sr1.read().bits() & (1 << 0)) == 0 {
                t -= 1;
                if t == 0 {
                    abort_i2c2(i2c2);
                    return None;
                }
            }

            // 2. Send slave address + Write
            i2c2.dr.write(|w| w.bits((addr7 as u32) << 1));
            t = I2C_TIMEOUT;
            while (i2c2.sr1.read().bits() & (1 << 1)) == 0 {
                if (i2c2.sr1.read().bits() & (1 << 10)) != 0 {
                    abort_i2c2(i2c2);
                    return None;
                }
                t -= 1;
                if t == 0 {
                    abort_i2c2(i2c2);
                    return None;
                }
            }
            let _ = i2c2.sr1.read().bits();
            let _ = i2c2.sr2.read().bits();

            // 3. Write command byte 0x00 (Input Port 0)
            i2c2.dr.write(|w| w.bits(0x00));
            t = I2C_TIMEOUT;
            while (i2c2.sr1.read().bits() & (1 << 2)) == 0 {
                if (i2c2.sr1.read().bits() & (1 << 10)) != 0 {
                    abort_i2c2(i2c2);
                    return None;
                }
                t -= 1;
                if t == 0 {
                    abort_i2c2(i2c2);
                    return None;
                }
            }

            // 4. Generate Repeated START
            i2c2.cr1.modify(|r, w| w.bits(r.bits() | (1 << 8)));
            t = I2C_TIMEOUT;
            while (i2c2.sr1.read().bits() & (1 << 0)) == 0 {
                t -= 1;
                if t == 0 {
                    abort_i2c2(i2c2);
                    return None;
                }
            }

            // 5. Send slave address + Read
            i2c2.dr.write(|w| w.bits(((addr7 as u32) << 1) | 1));
            t = I2C_TIMEOUT;
            while (i2c2.sr1.read().bits() & (1 << 1)) == 0 {
                if (i2c2.sr1.read().bits() & (1 << 10)) != 0 {
                    abort_i2c2(i2c2);
                    return None;
                }
                t -= 1;
                if t == 0 {
                    abort_i2c2(i2c2);
                    return None;
                }
            }

            // 6. RM0090 27.3.3 2-byte reception: POS=1, clear ADDR, clear ACK, wait BTF, STOP, read 2 bytes
            i2c2.cr1
                .modify(|r, w| w.bits(r.bits() | (1 << 11) | (1 << 10)));
            cortex_m::interrupt::free(|_| {
                let _ = i2c2.sr1.read().bits();
                let _ = i2c2.sr2.read().bits();
                i2c2.cr1.modify(|r, w| w.bits(r.bits() & !(1 << 10))); // ACK = 0
            });

            t = I2C_TIMEOUT;
            while (i2c2.sr1.read().bits() & (1 << 2)) == 0 {
                t -= 1;
                if t == 0 {
                    abort_i2c2(i2c2);
                    return None;
                }
            }

            i2c2.cr1.modify(|r, w| w.bits(r.bits() | (1 << 9))); // STOP = 1
            let port0 = (i2c2.dr.read().bits() & 0xFF) as u16;
            let port1 = (i2c2.dr.read().bits() & 0xFF) as u16;
            i2c2.cr1.modify(|r, w| w.bits(r.bits() & !(1 << 11))); // POS = 0

            Some(port0 | (port1 << 8))
        }
    }

    /// Refresh cached 16-bit switch port state and return `(SA_adc, SB_adc, SC_adc, SD_adc)`.
    pub fn poll_primary_virtual_adc() -> (u16, u16, u16, u16) {
        let ports = unsafe {
            let addr = if DETECTED_ADDR != 0 {
                DETECTED_ADDR
            } else {
                PCA9555_ADDR_PRIMARY
            };
            if let Some(val) = read_ports_raw(addr) {
                CACHED_PORTS = val;
            }
            CACHED_PORTS
        };

        let is_low = |bit: u8| ((ports >> bit) & 1) == 0;
        let two_pos_adc = |bit: u8| if is_low(bit) { 4000 } else { 100 };
        let three_pos_adc = |high_bit: u8, low_bit: u8| {
            if is_low(high_bit) {
                100
            } else if is_low(low_bit) {
                4000
            } else {
                2048
            }
        };

        (
            two_pos_adc(0),      // SA (Port 0 bit 0)
            three_pos_adc(1, 2), // SB (Port 0 bits 1, 2)
            three_pos_adc(3, 4), // SC (Port 0 bits 3, 4)
            two_pos_adc(5),      // SD (Port 0 bit 5)
        )
    }

    /// Return decoded `(SE, SF, SG, SH)` from cached PCA9555 Port 1 state.
    pub fn read_ext_switches() -> (SwitchPos, SwitchPos, SwitchPos, SwitchPos) {
        let ports = unsafe { CACHED_PORTS };
        let is_low = |bit: u8| ((ports >> bit) & 1) == 0;
        let two_pos = |bit: u8| {
            if is_low(bit) {
                SwitchPos::Down
            } else {
                SwitchPos::Up
            }
        };
        let three_pos = |high_bit: u8, low_bit: u8| {
            if is_low(high_bit) {
                SwitchPos::Up
            } else if is_low(low_bit) {
                SwitchPos::Down
            } else {
                SwitchPos::Mid
            }
        };

        (
            three_pos(8, 9),   // SE (Port 1 bits 0, 1)
            three_pos(10, 11), // SF (Port 1 bits 2, 3)
            two_pos(12),       // SG (Port 1 bit 4)
            two_pos(13),       // SH (Port 1 bit 5)
        )
    }
}
