//! Chip identification, Silicon Unique ID (UID) extraction, and DFU bootloader jump.

use crate::hal::SystemDriver;

static mut SYSTEM_DRIVER: Option<&'static mut dyn SystemDriver> = None;

/// Register platform-specific system driver.
pub fn register_system_driver(driver: &'static mut dyn SystemDriver) {
    unsafe {
        SYSTEM_DRIVER = Some(driver);
    }
}

/// Read the 96-bit (12-byte) Unique Device ID from MCU silicon.
pub fn read_uid() -> [u8; 12] {
    if let Some(ref mut driver) = unsafe { SYSTEM_DRIVER.as_mut() } {
        driver.read_uid()
    } else {
        [0u8; 12]
    }
}

/// Retrieve the active MCU silicon model name.
pub fn mcu_name() -> &'static str {
    if let Some(ref mut driver) = unsafe { SYSTEM_DRIVER.as_mut() } {
        driver.mcu_name()
    } else {
        "Unknown MCU"
    }
}

/// Run early platform-specific pre-initialization (e.g., VTOR relocation under bootloader).
pub fn pre_init() {
    if let Some(ref mut driver) = unsafe { SYSTEM_DRIVER.as_mut() } {
        driver.pre_init();
    }
}

/// Initialize system clock using registered SystemDriver.
pub fn init_system_clock() {
    if let Some(ref mut driver) = unsafe { SYSTEM_DRIVER.as_mut() } {
        driver.init_system_clock();
    }
}

/// Initialize SysTick monotonic 1.000 ms timekeeper using registered SystemDriver.
pub fn init_time() {
    if let Some(ref mut driver) = unsafe { SYSTEM_DRIVER.as_mut() } {
        driver.init_time();
    }
}

/// Spin-delay for a given number of processor cycles.
pub fn delay_cycles(n: u32) {
    if let Some(ref mut driver) = unsafe { SYSTEM_DRIVER.as_mut() } {
        driver.delay_cycles(n);
    }
}

/// Check if the MCU was rebooted warm by the hardware watchdog or software reset,
/// and clear the reset status flags.
pub fn check_and_clear_reset_flags() -> bool {
    if let Some(ref mut driver) = unsafe { SYSTEM_DRIVER.as_mut() } {
        driver.check_and_clear_reset_flags()
    } else {
        false
    }
}

/// Check if retained memory contains a pending DFU boot request.
pub fn has_retained_dfu_flag() -> bool {
    if let Some(ref mut driver) = unsafe { SYSTEM_DRIVER.as_mut() } {
        driver.has_retained_dfu_flag()
    } else {
        false
    }
}

/// Clear retained DFU boot request.
pub fn clear_retained_dfu_flag() {
    if let Some(ref mut driver) = unsafe { SYSTEM_DRIVER.as_mut() } {
        driver.clear_retained_dfu_flag();
    }
}

/// Reboot into DFU bootloader via platform driver.
pub fn reboot_to_dfu() -> ! {
    if let Some(ref mut driver) = unsafe { SYSTEM_DRIVER.as_mut() } {
        driver.reboot_to_dfu();
    } else {
        loop {
            core::hint::spin_loop();
        }
    }
}

/// Perform a software jump to the MCU's built-in factory DFU bootloader.
pub fn enter_dfu_bootloader() -> ! {
    if let Some(ref mut driver) = unsafe { SYSTEM_DRIVER.as_mut() } {
        driver.enter_dfu_bootloader();
    } else {
        loop {
            core::hint::spin_loop();
        }
    }
}
