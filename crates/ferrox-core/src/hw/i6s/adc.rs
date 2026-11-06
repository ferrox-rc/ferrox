//! Hardware ADC interface for FlySky FS-i6S.

pub use crate::adc::{
    init, is_ext_adc_enabled, read_raw, set_ext_adc_enabled, wait_first_conversion, ADC_CENTER,
    ADC_MAX, ADC_MIN, NUM_CHANNELS, NUM_STD_CHANNELS,
};

#[cfg(test)]
pub use crate::adc::set_test_adc_snapshot;

pub const NUM_EXT_CHANNELS: usize = NUM_CHANNELS;

#[inline(always)]
pub fn set_pc_adc_enabled(enabled: bool) {
    set_ext_adc_enabled(enabled);
}

#[inline(always)]
pub fn is_pc_adc_enabled() -> bool {
    is_ext_adc_enabled()
}
