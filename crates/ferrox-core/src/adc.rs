//! Platform-agnostic analog-to-digital conversion interface.

use crate::hal::{AdcDriver, MAX_ADC_CHANNELS};

pub const NUM_STD_CHANNELS: usize = 11;
pub const NUM_CHANNELS: usize = MAX_ADC_CHANNELS;

pub const ADC_MIN: u16 = 0;
pub const ADC_CENTER: u16 = 2048;
pub const ADC_MAX: u16 = 4095;

static mut ADC_DRIVER: Option<&'static mut dyn AdcDriver> = None;

#[cfg(test)]
static mut TEST_SNAPSHOT: [u16; NUM_CHANNELS] = [ADC_CENTER; NUM_CHANNELS];

/// Register platform-specific ADC scanner driver.
pub fn register_adc_driver(driver: &'static mut dyn AdcDriver) {
    unsafe { ADC_DRIVER = Some(driver) };
}

/// Initialize ADC peripheral and autonomous DMA scanning.
pub fn init(ext_adc: bool) {
    if let Some(ref mut d) = unsafe { ADC_DRIVER.as_mut() } {
        d.init(ext_adc);
    }
}

/// Wait for at least one complete conversion cycle.
pub fn wait_first_conversion() {
    if let Some(ref mut d) = unsafe { ADC_DRIVER.as_mut() } {
        d.wait_first_conversion();
    }
}

/// Read latest raw ADC channel values.
pub fn read_raw() -> [u16; NUM_CHANNELS] {
    #[cfg(test)]
    unsafe {
        TEST_SNAPSHOT
    }
    #[cfg(not(test))]
    if let Some(ref mut d) = unsafe { ADC_DRIVER.as_mut() } {
        d.read_raw()
    } else {
        [ADC_CENTER; NUM_CHANNELS]
    }
}

#[cfg(test)]
pub fn set_test_adc_snapshot(snapshot: [u16; NUM_CHANNELS]) {
    unsafe {
        TEST_SNAPSHOT = snapshot;
    }
}

#[inline(always)]
pub fn set_ext_adc_enabled(enabled: bool) {
    init(enabled);
}

#[inline(always)]
pub fn is_ext_adc_enabled() -> bool {
    false
}
