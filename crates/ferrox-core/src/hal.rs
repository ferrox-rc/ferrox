//! Hardware Abstraction Layer Traits for Ferrox.
//!
//! Decouples core logic from microcontrollers (STM32F0, STM32F4, etc.)
//! and allows testable, modular driver substitution.

/// Hardware audio / tone generator output.
pub trait ToneOutput {
    /// Initialize audio hardware timer / GPIO.
    fn init(&mut self);

    /// Turn on hardware audio generation at `freq_hz`.
    fn on(&mut self, freq_hz: u16);

    /// Turn off hardware audio generation.
    fn off(&mut self);
}

/// Dummy no-op tone output for tests and quiet operation.
#[derive(Copy, Clone, Debug, Default)]
pub struct NullToneOutput;

impl ToneOutput for NullToneOutput {
    fn init(&mut self) {}
    fn on(&mut self, _freq_hz: u16) {}
    fn off(&mut self) {}
}

/// Hardware LCD display bus flush and backlight driver.
pub trait DisplayBus {
    /// Initialize GPIOs and controller registers.
    fn init(&mut self);

    /// Flush the 1024-byte framebuffer to the screen.
    fn flush(&mut self, framebuffer: &[u8; 1024]);

    /// Set display contrast value (0..63).
    fn set_contrast(&mut self, contrast: u8);

    /// Set backlight brightness level (0..100%).
    fn set_backlight_level(&mut self, pct: u8);
}

/// Dummy display bus for tests.
#[derive(Copy, Clone, Debug, Default)]
pub struct NullDisplayBus;

impl DisplayBus for NullDisplayBus {
    fn init(&mut self) {}
    fn flush(&mut self, _framebuffer: &[u8; 1024]) {}
    fn set_contrast(&mut self, _contrast: u8) {}
    fn set_backlight_level(&mut self, _pct: u8) {}
}

/// Block-level non-volatile storage driver.
pub trait StorageBus {
    /// Initialize bus interface and verify storage chip presence.
    fn init(&mut self) -> bool;

    /// Read a slice of bytes from non-volatile storage.
    #[allow(clippy::result_unit_err)]
    fn read(&mut self, addr: u16, buf: &mut [u8]) -> Result<(), ()>;

    /// Write a slice of bytes to non-volatile storage.
    #[allow(clippy::result_unit_err)]
    fn write(&mut self, addr: u16, buf: &[u8]) -> Result<(), ()>;
}

/// Dummy no-op storage bus for tests and simulated environments.
#[derive(Copy, Clone, Debug, Default)]
pub struct NullStorageBus;

impl StorageBus for NullStorageBus {
    fn init(&mut self) -> bool {
        false
    }
    fn read(&mut self, _addr: u16, _buf: &mut [u8]) -> Result<(), ()> {
        Err(())
    }
    fn write(&mut self, _addr: u16, _buf: &[u8]) -> Result<(), ()> {
        Err(())
    }
}

/// Asynchronous serial communication port driver (e.g., CRSF UART).
pub trait SerialPort {
    /// Initialize UART peripheral and power supply pin.
    fn init(&mut self, active_high: bool);

    /// Set power control pin polarity.
    fn set_power_polarity(&mut self, active_high: bool);

    /// Enable or disable UART reception/transmission at `baud_idx`.
    fn set_uart_enabled(&mut self, enabled: bool, baud_idx: u8, half_duplex: bool);

    /// Control external RF module power rail.
    fn set_module_power(&mut self, enabled: bool);

    /// Write a slice of bytes to the transmit buffer/DMA.
    fn write_bytes(&mut self, bytes: &[u8]);

    /// Read a single byte from the receive buffer, if available.
    fn read_byte(&mut self) -> Option<u8>;
}

/// Dummy serial port for tests.
#[derive(Copy, Clone, Debug, Default)]
pub struct NullSerialPort;

impl SerialPort for NullSerialPort {
    fn init(&mut self, _active_high: bool) {}
    fn set_power_polarity(&mut self, _active_high: bool) {}
    fn set_uart_enabled(&mut self, _enabled: bool, _baud_idx: u8, _half_duplex: bool) {}
    fn set_module_power(&mut self, _enabled: bool) {}
    fn write_bytes(&mut self, _bytes: &[u8]) {}
    fn read_byte(&mut self) -> Option<u8> {
        None
    }
}

pub const MAX_ADC_CHANNELS: usize = 16;

/// Analog-to-digital converter scanner driver.
pub trait AdcDriver {
    /// Initialize ADC peripheral and autonomous DMA scanning.
    fn init(&mut self, ext_adc: bool);

    /// Read the latest raw 12-bit ADC snapshot across all scanned channels.
    fn read_raw(&mut self) -> [u16; MAX_ADC_CHANNELS];

    /// Block until the first full ADC conversion sequence finishes.
    fn wait_first_conversion(&mut self);
}

/// Dummy ADC driver returning centered sticks (2048).
#[derive(Copy, Clone, Debug, Default)]
pub struct NullAdcDriver;

impl AdcDriver for NullAdcDriver {
    fn init(&mut self, _ext_adc: bool) {}
    fn read_raw(&mut self) -> [u16; MAX_ADC_CHANNELS] {
        [2048; MAX_ADC_CHANNELS]
    }
    fn wait_first_conversion(&mut self) {}
}

/// Physical button and switch matrix scanner driver.
pub trait KeyDriver {
    /// Initialize GPIO inputs and matrix rows/cols.
    fn init(&mut self);

    /// Scan all physical buttons and return 16-bit key bitmask.
    fn scan_keys(&mut self) -> u16;

    /// Read auxiliary external digital switches (e.g. SE/SF on PC12/PC15 or SG/SH).
    fn read_ext_switches(&mut self) -> (crate::input::SwitchPos, crate::input::SwitchPos) {
        (crate::input::SwitchPos::Up, crate::input::SwitchPos::Up)
    }

    /// Read front left and right buttons (FS-i6S).
    fn read_front_buttons(&mut self) -> (bool, bool) {
        (false, false)
    }

    /// Read J4 tactile buttons (FS-i6S: PA13/PA14).
    fn read_j4_buttons(&mut self) -> (bool, bool) {
        (false, false)
    }

    /// Configure J4 pins as GPIO inputs with pull-ups.
    fn init_j4_keys(&mut self, _enabled: bool) {}
}

/// Dummy key driver returning no keys pressed (0).
#[derive(Copy, Clone, Debug, Default)]
pub struct NullKeyDriver;

impl KeyDriver for NullKeyDriver {
    fn init(&mut self) {}
    fn scan_keys(&mut self) -> u16 {
        0
    }
}

/// Electronic power latch and power button driver.
pub trait PowerDriver {
    /// Initialize power latch and power button GPIOs.
    fn init(&mut self);

    /// Assert power latch to keep board powered on.
    fn latch_on(&mut self);

    /// Release power latch to power down board.
    fn latch_off(&mut self);

    /// Check if physical power button is currently pressed.
    fn is_power_key_pressed(&mut self) -> bool;

    /// Control power button LED backlight.
    fn set_leds(&mut self, on: bool);
}

/// Dummy power driver.
#[derive(Copy, Clone, Debug, Default)]
pub struct NullPowerDriver;

impl PowerDriver for NullPowerDriver {
    fn init(&mut self) {}
    fn latch_on(&mut self) {}
    fn latch_off(&mut self) {}
    fn is_power_key_pressed(&mut self) -> bool {
        false
    }
    fn set_leds(&mut self, _on: bool) {}
}

static mut POWER_DRIVER: Option<&'static mut dyn PowerDriver> = None;

/// Register platform-specific power driver.
pub fn register_power_driver(driver: &'static mut dyn PowerDriver) {
    unsafe { POWER_DRIVER = Some(driver) };
}

pub fn get_power_driver<'a>() -> Option<&'a mut dyn PowerDriver> {
    unsafe {
        if let Some(ref mut d) = POWER_DRIVER {
            Some(&mut **d)
        } else {
            None
        }
    }
}

/// Capacitive touchscreen driver.
pub trait TouchDriver {
    /// Initialize touch controller communication and interrupt pin.
    fn init(&mut self);

    /// Check if touch interrupt line is asserted.
    fn is_touch_int_asserted(&mut self) -> bool;

    /// Read latest touch coordinates if available.
    fn read_touch(&mut self) -> Option<(u16, u16)>;
}

/// Dummy touch driver.
#[derive(Copy, Clone, Debug, Default)]
pub struct NullTouchDriver;

impl TouchDriver for NullTouchDriver {
    fn init(&mut self) {}
    fn is_touch_int_asserted(&mut self) -> bool {
        false
    }
    fn read_touch(&mut self) -> Option<(u16, u16)> {
        None
    }
}

static mut TOUCH_DRIVER: Option<&'static mut dyn TouchDriver> = None;

/// Register platform-specific touch controller driver.
pub fn register_touch_driver(driver: &'static mut dyn TouchDriver) {
    unsafe { TOUCH_DRIVER = Some(driver) };
}

pub fn get_touch_driver<'a>() -> Option<&'a mut dyn TouchDriver> {
    unsafe {
        if let Some(ref mut d) = TOUCH_DRIVER {
            Some(&mut **d)
        } else {
            None
        }
    }
}

/// Hardware Watchdog (IWDG) driver.
pub trait WatchdogDriver {
    /// Initialize and start hardware watchdog timer.
    fn init(&mut self);

    /// Refresh / feed the watchdog counter to prevent reset.
    fn feed(&mut self);
}

/// Dummy watchdog for tests.
#[derive(Copy, Clone, Debug, Default)]
pub struct NullWatchdog;

impl WatchdogDriver for NullWatchdog {
    fn init(&mut self) {}
    fn feed(&mut self) {}
}

/// Low-level MCU system clock, reset flag query, and silicon UID reader.
pub trait SystemDriver {
    /// Early board pre-initialization (e.g. electronic power latch).
    fn pre_init(&mut self);

    /// Check if boot was caused by watchdog reset and clear RCC flags.
    fn check_and_clear_reset_flags(&mut self) -> bool;

    /// Configure PLL system clock to target frequency (e.g. 48 MHz).
    fn init_system_clock(&mut self);

    /// Read 96-bit (12-byte) silicon Unique Device ID.
    fn read_uid(&mut self) -> [u8; 12];

    /// Initialize SysTick monotonic 1.000 ms timekeeper.
    fn init_time(&mut self);
}

/// Dummy system driver for tests.
#[derive(Copy, Clone, Debug, Default)]
pub struct NullSystemDriver;

impl SystemDriver for NullSystemDriver {
    fn pre_init(&mut self) {}
    fn check_and_clear_reset_flags(&mut self) -> bool {
        false
    }
    fn init_system_clock(&mut self) {}
    fn read_uid(&mut self) -> [u8; 12] {
        [0u8; 12]
    }
    fn init_time(&mut self) {}
}

/// Trainer and PPM input/output hardware driver.
pub trait TrainerDriver {
    /// Initialize timer and GPIO pins for PPM out/in.
    fn init(&mut self, ppm_out_en: bool, ppm_in_en: bool);

    /// Enable or disable PPM output pin and timer interrupt.
    fn set_ppm_out(&mut self, enabled: bool);

    /// Enable or disable PPM input capture pin and timer interrupt.
    fn set_ppm_in(&mut self, enabled: bool);
}

/// Dummy trainer driver for tests.
#[derive(Copy, Clone, Debug, Default)]
pub struct NullTrainerDriver;

impl TrainerDriver for NullTrainerDriver {
    fn init(&mut self, _ppm_out_en: bool, _ppm_in_en: bool) {}
    fn set_ppm_out(&mut self, _enabled: bool) {}
    fn set_ppm_in(&mut self, _enabled: bool) {}
}

/// RF transceiver SPI bus and hardware control lines (A7105 / CC2500).
pub trait RfBus {
    /// Initialize SPI peripheral, CSN, TX/RX enable pins, GIO2/IRQ pin, and protocol timer.
    fn init(&mut self);

    /// Initialize periodic protocol frame timer (e.g. 3850 µs for AFHDS 2A).
    fn init_timer(&mut self, period_us: u16);

    /// Clear timer update interrupt flag.
    fn clear_timer_flag(&mut self);

    /// Assert Chip Select (Active Low).
    fn csn_low(&mut self);

    /// Deassert Chip Select (High).
    fn csn_high(&mut self);

    /// Full duplex single-byte transfer over SPI.
    fn transfer_byte(&mut self, data: u8) -> u8;

    /// Set frontend PA/LNA mode (RX, TX, or OFF).
    fn set_tx_rx_mode(&mut self, mode: u8);

    /// Toggle antenna diversity switch between primary and secondary antenna.
    fn switch_antenna(&mut self);

    /// Enable transceiver GIO2/IRQ line interrupt.
    fn enable_gio2_irq(&mut self);

    /// Disable transceiver GIO2/IRQ line interrupt.
    fn disable_gio2_irq(&mut self);
}

/// Dummy RF bus for tests.
#[derive(Copy, Clone, Debug, Default)]
pub struct NullRfBus;

impl RfBus for NullRfBus {
    fn init(&mut self) {}
    fn init_timer(&mut self, _period_us: u16) {}
    fn clear_timer_flag(&mut self) {}
    fn csn_low(&mut self) {}
    fn csn_high(&mut self) {}
    fn transfer_byte(&mut self, _data: u8) -> u8 {
        0
    }
    fn set_tx_rx_mode(&mut self, _mode: u8) {}
    fn switch_antenna(&mut self) {}
    fn enable_gio2_irq(&mut self) {}
    fn disable_gio2_irq(&mut self) {}
}

/// USB hardware peripheral driver (transceiver, PMA endpoints, and enumeration).
pub trait UsbDriver {
    /// Initialize USB peripheral hardware in the specified mode (0=Off, 1=Joystick, 2=Serial, 3=Composite).
    fn init(&mut self, mode: u8);

    /// Poll and transmit pending HID report or telemetry packets.
    fn poll(
        &mut self,
        now_ms: u32,
        rf_chs: &[u16; crate::mixer::NUM_CHANNELS],
        switches: &crate::input::Switches,
        telem: &crate::rf::afhds2a::TelemetryData,
        battery_mv: u16,
    );

    /// Check if USB is configured and attached in Joystick simulation mode.
    fn is_sim_mode(&self) -> bool;
}

/// Dummy USB driver for tests.
#[derive(Copy, Clone, Debug, Default)]
pub struct NullUsbDriver;

impl UsbDriver for NullUsbDriver {
    fn init(&mut self, _mode: u8) {}
    fn poll(
        &mut self,
        _now_ms: u32,
        _rf_chs: &[u16; crate::mixer::NUM_CHANNELS],
        _switches: &crate::input::Switches,
        _telem: &crate::rf::afhds2a::TelemetryData,
        _battery_mv: u16,
    ) {
    }
    fn is_sim_mode(&self) -> bool {
        false
    }
}

