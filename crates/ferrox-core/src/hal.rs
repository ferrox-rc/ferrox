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
    fn read(&mut self, addr: u16, buf: &mut [u8]) -> Result<(), ()>;

    /// Write a slice of bytes to non-volatile storage.
    fn write(&mut self, addr: u16, buf: &[u8]) -> Result<(), ()>;
}
