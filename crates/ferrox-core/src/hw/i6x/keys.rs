//! Key scanning and DFU detection for FlySky FS-i6X.

pub use crate::boot::{
    check_dfu_entry, check_sram_dfu_entry, init_keys, is_dfu_requested, read_ext_switches,
    reboot_to_dfu, scan_keys, DFU_FLAG_ADDR, DFU_MAGIC_FLAG,
};
