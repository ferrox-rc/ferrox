//! USB subsystem: HID descriptors, CDC-ACM serial protocol, and USB mode handling.

#![allow(static_mut_refs, dead_code)]

pub mod descriptors;
pub mod hid;
pub mod serial;

use crate::hal::UsbDriver;
use crate::input::Switches;
use crate::rf::afhds2a::TelemetryData;

pub use serial::ResetAction;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum UsbMode {
    Off = 0,       // USB disconnected / charging only (Default)
    Joystick = 1,  // Native USB Gamepad (RF disabled for simulator play)
    Serial = 2,    // Virtual COM Port (RF active with live telemetry streaming)
    Composite = 3, // Both Joystick and Serial active simultaneously
}

impl UsbMode {
    pub fn from_u8(val: u8) -> Self {
        match val {
            1 => Self::Joystick,
            2 => Self::Serial,
            3 => Self::Composite,
            _ => Self::Off,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::Joystick => "JOYSTICK",
            Self::Serial => "SERIAL",
            Self::Composite => "COMPOSITE",
        }
    }
}

static mut USB_DRIVER: Option<&'static mut dyn UsbDriver> = None;
static mut PENDING_INIT_MODE: Option<u8> = None;

/// Register platform-specific USB hardware driver.
pub fn register_usb_driver(driver: &'static mut dyn UsbDriver) {
    unsafe { USB_DRIVER = Some(driver) };
}

/// Request deferred USB re-initialization.
pub fn request_init(mode: u8) {
    cortex_m::interrupt::free(|_| unsafe {
        PENDING_INIT_MODE = Some(mode);
    });
}

/// Retrieve and clear any pending USB re-initialization request.
pub fn take_pending_init() -> Option<u8> {
    cortex_m::interrupt::free(|_| unsafe { PENDING_INIT_MODE.take() })
}

/// Initialize USB peripheral in the requested mode.
pub fn init(mode_u8: u8) {
    if let Some(ref mut driver) = unsafe { USB_DRIVER.as_mut() } {
        driver.init(mode_u8);
    }
}

/// Returns true if USB is in active flight simulator mode (Joystick configured and attached).
pub fn is_sim_mode() -> bool {
    if let Some(driver) = unsafe { USB_DRIVER.as_ref() } {
        driver.is_sim_mode()
    } else {
        false
    }
}

/// Periodic USB task (called from the main loop).
pub fn poll(
    now_ms: u32,
    rf_chs: &[u16; crate::mixer::NUM_CHANNELS],
    switches: &Switches,
    telem: &TelemetryData,
    battery_mv: u16,
) {
    if let Some(ref mut driver) = unsafe { USB_DRIVER.as_mut() } {
        driver.poll(now_ms, rf_chs, switches, telem, battery_mv);
    }
}
