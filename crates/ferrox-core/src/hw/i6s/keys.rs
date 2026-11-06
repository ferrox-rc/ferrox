//! Boot manager and key scanning for FlySky FS-i6S.

pub use crate::boot::{
    check_dfu_entry, check_sram_dfu_entry, init_j4_keys, init_keys, is_dfu_requested,
    read_ext_switches, read_front_buttons, read_j4_buttons, reboot_to_dfu, scan_keys,
    set_touch_keys, DFU_FLAG_ADDR, DFU_MAGIC_FLAG,
};

#[cfg(test)]
pub use crate::boot::{test_set_front_buttons, test_set_j4_buttons, test_set_keys};

#[inline(always)]
pub fn read_rear_buttons() -> (bool, bool) {
    read_front_buttons()
}

#[inline]
pub fn inject_touch_keys(keys: u16) {
    let cur = crate::boot::get_touch_keys();
    crate::boot::set_touch_keys(cur | keys);
}

#[inline]
pub fn clear_touch_keys() {
    crate::boot::set_touch_keys(0);
}

