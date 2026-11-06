//! Boot manager, key scanning hook, and DFU bootloader detection.

use crate::chip::{self, McuProfile};
use crate::hal::KeyDriver;

/// Magic token in high SRAM used to signal a warm reboot directly into the DFU bootloader.
pub const DFU_MAGIC_FLAG: u32 = 0xDEAD_BEEF;

/// Address in top 16 bytes of STM32F072 16KB SRAM (0x2000_0000 .. 0x2000_4000).
pub const DFU_FLAG_ADDR: *mut u32 = 0x2000_3FF0 as *mut u32;

static mut KEY_DRIVER: Option<&'static mut dyn KeyDriver> = None;
static TOUCH_KEYS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

#[cfg(test)]
static TEST_KEYS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(test)]
static TEST_FRONT_BUTTONS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(test)]
static TEST_J4_BUTTONS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// Register platform-specific key driver.
pub fn register_key_driver(driver: &'static mut dyn KeyDriver) {
    unsafe { KEY_DRIVER = Some(driver) };
}

/// Inject or update touch virtual navigation keys.
pub fn set_touch_keys(keys: u16) {
    TOUCH_KEYS.store(keys as u32, core::sync::atomic::Ordering::Relaxed);
}

/// Retrieve current touch virtual navigation keys.
pub fn get_touch_keys() -> u16 {
    TOUCH_KEYS.load(core::sync::atomic::Ordering::Relaxed) as u16
}

/// Initialize keys and button inputs.
pub fn init_keys() {
    if let Some(ref mut d) = unsafe { KEY_DRIVER.as_mut() } {
        d.init();
    }
}

/// Scan all physical and matrix keys merged with virtual touch keys.
pub fn scan_keys() -> u16 {
    #[cfg(test)]
    let hw = TEST_KEYS.load(core::sync::atomic::Ordering::Relaxed) as u16;
    #[cfg(not(test))]
    let hw = if let Some(ref mut d) = unsafe { KEY_DRIVER.as_mut() } {
        d.scan_keys()
    } else {
        0
    };
    hw | get_touch_keys()
}

/// Read external digital switches (SE/SF on PC12/PC15 or SG/SH on J4).
pub fn read_ext_switches() -> (crate::input::SwitchPos, crate::input::SwitchPos) {
    if let Some(ref mut d) = unsafe { KEY_DRIVER.as_mut() } {
        d.read_ext_switches()
    } else {
        (crate::input::SwitchPos::Up, crate::input::SwitchPos::Up)
    }
}

/// Read front left and right buttons (FS-i6S).
pub fn read_front_buttons() -> (bool, bool) {
    #[cfg(test)]
    {
        let val = TEST_FRONT_BUTTONS.load(core::sync::atomic::Ordering::Relaxed);
        ((val & 1) != 0, (val & 2) != 0)
    }
    #[cfg(not(test))]
    if let Some(ref mut d) = unsafe { KEY_DRIVER.as_mut() } {
        d.read_front_buttons()
    } else {
        (false, false)
    }
}

/// Read J4 tactile buttons (FS-i6S: PA13/PA14).
pub fn read_j4_buttons() -> (bool, bool) {
    #[cfg(test)]
    {
        let val = TEST_J4_BUTTONS.load(core::sync::atomic::Ordering::Relaxed);
        ((val & 1) != 0, (val & 2) != 0)
    }
    #[cfg(not(test))]
    if let Some(ref mut d) = unsafe { KEY_DRIVER.as_mut() } {
        d.read_j4_buttons()
    } else {
        (false, false)
    }
}

/// Configure J4 pins as GPIO inputs with pull-ups.
pub fn init_j4_keys(enabled: bool) {
    if let Some(ref mut d) = unsafe { KEY_DRIVER.as_mut() } {
        d.init_j4_keys(enabled);
    }
}

#[cfg(test)]
pub fn test_set_keys(keys: u16) {
    TEST_KEYS.store(keys as u32, core::sync::atomic::Ordering::Relaxed);
}

#[cfg(test)]
pub fn test_set_front_buttons(left: bool, right: bool) {
    let val = (left as u32) | ((right as u32) << 1);
    TEST_FRONT_BUTTONS.store(val, core::sync::atomic::Ordering::Relaxed);
}

#[cfg(test)]
pub fn test_set_j4_buttons(sg: bool, sh: bool) {
    let val = (sg as u32) | ((sh as u32) << 1);
    TEST_J4_BUTTONS.store(val, core::sync::atomic::Ordering::Relaxed);
}

#[inline(always)]
fn delay_cycles(n: u32) {
    #[cfg(not(test))]
    cortex_m::asm::delay(n);
    #[cfg(test)]
    let _ = n;
}

/// Check if the DFU bootloader key combination is held.
pub fn is_dfu_requested(keys: u16) -> bool {
    #[cfg(feature = "board-i6s")]
    {
        // On FS-i6S, hold Left + Right front buttons during power-on (bits 10 and 11)
        (keys & ((1 << 10) | (1 << 11))) == ((1 << 10) | (1 << 11))
    }
    #[cfg(not(feature = "board-i6s"))]
    {
        // On FS-i6X, hold inward horizontal trims (Roll L + Yaw R)
        let rh_inward = (keys & (1 << 1)) != 0;
        let lh_inward = (keys & (1 << 6)) != 0;
        rh_inward && lh_inward
    }
}

/// Check if high SRAM flag requests a DFU jump across soft reboot.
pub fn check_sram_dfu_entry(profile: &McuProfile) {
    unsafe {
        let flag = core::ptr::read_volatile(DFU_FLAG_ADDR);
        if flag == DFU_MAGIC_FLAG {
            core::ptr::write_volatile(DFU_FLAG_ADDR, 0);
            chip::enter_dfu_bootloader(profile);
        }
    }
}

/// Check if DFU boot keys are pressed and jump to bootloader if requested.
pub fn check_dfu_entry(profile: &McuProfile) {
    init_keys();

    // Wait ~20ms for power rails and switch contacts to settle at power-on
    delay_cycles(40_000);

    let mut match_count = 0;
    for _ in 0..5 {
        let keys = scan_keys();
        if is_dfu_requested(keys) {
            match_count += 1;
        }
        delay_cycles(2_000);
    }

    if match_count >= 3 {
        chip::enter_dfu_bootloader(profile);
    }
}

/// Trigger an immediate reboot into DFU bootloader mode.
pub fn reboot_to_dfu() -> ! {
    unsafe {
        core::ptr::write_volatile(DFU_FLAG_ADDR, DFU_MAGIC_FLAG);
        cortex_m::peripheral::SCB::sys_reset();
    }
}
