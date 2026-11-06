//! Hardware ADC interface for FlySky FS-i6X.

pub use crate::adc::{
    init, is_ext_adc_enabled, read_raw, set_ext_adc_enabled, wait_first_conversion, ADC_CENTER,
    ADC_MAX, ADC_MIN, NUM_CHANNELS, NUM_STD_CHANNELS,
};

#[inline(always)]
pub fn set_p7_enabled(enabled: bool) {
    set_ext_adc_enabled(enabled);
}

#[inline(always)]
pub fn is_p7_enabled() -> bool {
    is_ext_adc_enabled()
}
