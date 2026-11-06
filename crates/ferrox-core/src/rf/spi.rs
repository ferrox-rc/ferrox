//! Hardware-agnostic SPI and RF frontend control interface.

#![allow(dead_code)]

use crate::hal::RfBus;

// RF Mode constants matching OpenI6X / A7105 hardware
pub const RF_MODE_RX_EN: u8 = 0b0000_0001;
pub const RF_MODE_TX_EN: u8 = 0b0000_0010;
pub const RF_MODE_OFF: u8 = 0b0000_0011;

static mut RF_BUS: Option<&'static mut dyn RfBus> = None;

/// Register platform-specific RF bus driver.
pub fn register_rf_bus(bus: &'static mut dyn RfBus) {
    unsafe { RF_BUS = Some(bus) };
}

/// Initialize SPI, frontend control GPIOs, GIO2 IRQ, and protocol timer.
pub fn init() {
    if let Some(ref mut bus) = unsafe { RF_BUS.as_mut() } {
        bus.init();
    }
}

/// Assert Chip Select (Active Low).
#[inline]
pub fn csn_low() {
    if let Some(ref mut bus) = unsafe { RF_BUS.as_mut() } {
        bus.csn_low();
    }
}

/// Deassert Chip Select (High).
#[inline]
pub fn csn_high() {
    if let Some(ref mut bus) = unsafe { RF_BUS.as_mut() } {
        bus.csn_high();
    }
}

/// Transfer a single byte over SPI (Full Duplex).
#[inline]
pub fn transfer_byte(data: u8) -> u8 {
    if let Some(ref mut bus) = unsafe { RF_BUS.as_mut() } {
        bus.transfer_byte(data)
    } else {
        0
    }
}

/// Write a single byte over SPI (discards read byte).
#[inline]
pub fn write_byte(data: u8) {
    let _ = transfer_byte(data);
}

/// Read a single byte over SPI (sends dummy 0x00).
#[inline]
pub fn read_byte() -> u8 {
    transfer_byte(0x00)
}

/// Set front-end PA/LNA mode (TX, RX, or OFF).
pub fn set_tx_rx_mode(mode: u8) {
    if let Some(ref mut bus) = unsafe { RF_BUS.as_mut() } {
        bus.set_tx_rx_mode(mode);
    }
}

/// Alternate antenna diversity switch between Antenna 0 and Antenna 1.
pub fn switch_antenna() {
    if let Some(ref mut bus) = unsafe { RF_BUS.as_mut() } {
        bus.switch_antenna();
    }
}

/// Enable transceiver GIO2/IRQ line interrupt.
#[inline(always)]
pub fn enable_gio2_irq() {
    if let Some(ref mut bus) = unsafe { RF_BUS.as_mut() } {
        bus.enable_gio2_irq();
    }
}

/// Disable transceiver GIO2/IRQ line interrupt.
#[inline(always)]
pub fn disable_gio2_irq() {
    if let Some(ref mut bus) = unsafe { RF_BUS.as_mut() } {
        bus.disable_gio2_irq();
    }
}

/// Initialize protocol timer for periodic packet generation (e.g. 3850 µs).
pub fn init_tim16(period_us: u16) {
    if let Some(ref mut bus) = unsafe { RF_BUS.as_mut() } {
        bus.init_timer(period_us);
    }
}

/// Clear protocol timer update interrupt flag.
#[inline(always)]
pub fn clear_tim16_flag() {
    if let Some(ref mut bus) = unsafe { RF_BUS.as_mut() } {
        bus.clear_timer_flag();
    }
}
