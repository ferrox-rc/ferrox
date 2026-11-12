//! Boot manager, key scanning hook, and DFU bootloader detection.

use crate::chip;
use crate::hal::KeyDriver;

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

/// Read external digital switches (SE/SF/SG/SH).
pub fn read_ext_switches() -> (
    crate::input::SwitchPos,
    crate::input::SwitchPos,
    crate::input::SwitchPos,
    crate::input::SwitchPos,
) {
    if let Some(ref mut d) = unsafe { KEY_DRIVER.as_mut() } {
        d.read_ext_switches()
    } else {
        (
            crate::input::SwitchPos::Up,
            crate::input::SwitchPos::Up,
            crate::input::SwitchPos::Up,
            crate::input::SwitchPos::Up,
        )
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

fn enter_dfu_with_splash() -> ! {
    let mut lcd = crate::display::st7567::St7567::new();
    crate::ui::splash::draw_bootloader(&mut lcd);
    lcd.flush();
    chip::enter_dfu_bootloader();
}

/// Check if retained `.uninit` SRAM flag requests a DFU jump across soft reboot.
pub fn check_sram_dfu_entry() {
    if chip::has_retained_dfu_flag() {
        chip::clear_retained_dfu_flag();
        enter_dfu_with_splash();
    }
}

/// Check if DFU boot keys are pressed and jump to bootloader if requested.
pub fn check_dfu_entry() {
    init_keys();

    // Wait ~20ms for power rails and switch contacts to settle at power-on
    chip::delay_cycles(40_000);

    let mut match_count = 0;
    for _ in 0..5 {
        let keys = scan_keys();
        if is_dfu_requested(keys) {
            match_count += 1;
        }
        chip::delay_cycles(2_000);
    }

    if match_count >= 3 {
        enter_dfu_with_splash();
    }
}

/// Trigger an immediate reboot into DFU bootloader mode.
pub fn reboot_to_dfu() -> ! {
    chip::reboot_to_dfu();
}
