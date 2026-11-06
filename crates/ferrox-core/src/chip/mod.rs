//! Chip identification, Silicon Unique ID (UID) extraction, and DFU bootloader jump.

use core::ptr;
use crate::hal::SystemDriver;

/// Supported MCU types.
#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum McuType {
    Stm32f072,
    Apm32f072,
    Stm32f407,
}

/// MCU-specific memory map profile.
#[allow(dead_code)]
pub struct McuProfile {
    pub mcu_type: McuType,
    pub name: &'static str,
    pub uid_addr: usize,
    pub dfu_bootloader_addr: usize,
}

/// STM32F072 memory layout definitions.
pub const STM32_PROFILE: McuProfile = McuProfile {
    mcu_type: McuType::Stm32f072,
    name: "STM32F072VB",
    uid_addr: 0x1FFF_F7AC,
    dfu_bootloader_addr: 0x1FFF_C800,
};

/// APM32F072 (Geehy) memory layout definitions.
#[allow(dead_code)]
pub const APM32_PROFILE: McuProfile = McuProfile {
    mcu_type: McuType::Apm32f072,
    name: "APM32F072VB",
    uid_addr: 0x1FFF_F7E8,
    dfu_bootloader_addr: 0x1FFF_F000,
};

/// Detect the active MCU profile.
pub fn get_mcu_profile() -> McuProfile {
    #[cfg(feature = "apm32")]
    {
        APM32_PROFILE
    }
    #[cfg(not(feature = "apm32"))]
    {
        STM32_PROFILE
    }
}

static mut SYSTEM_DRIVER: Option<&'static mut dyn SystemDriver> = None;
static mut DFU_JUMP_FN: Option<fn(&McuProfile) -> !> = None;

/// Register platform-specific system driver and DFU bootloader jump function.
pub fn register_system_driver(
    driver: &'static mut dyn SystemDriver,
    dfu_jump: Option<fn(&McuProfile) -> !>,
) {
    unsafe {
        SYSTEM_DRIVER = Some(driver);
        DFU_JUMP_FN = dfu_jump;
    }
}

/// Read the 96-bit (12-byte) Unique Device ID from MCU silicon.
#[allow(dead_code)]
pub fn read_uid(profile: &McuProfile) -> [u8; 12] {
    if let Some(ref mut driver) = unsafe { SYSTEM_DRIVER.as_mut() } {
        let uid = driver.read_uid();
        if uid != [0u8; 12] {
            return uid;
        }
    }
    let mut uid = [0u8; 12];
    unsafe {
        let src = profile.uid_addr as *const u8;
        for (i, byte) in uid.iter_mut().enumerate() {
            *byte = ptr::read_volatile(src.add(i));
        }
    }
    uid
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

/// Check if the MCU was rebooted warm by the hardware watchdog or software reset,
/// and clear the reset status flags.
pub fn check_and_clear_reset_flags() -> bool {
    if let Some(ref mut driver) = unsafe { SYSTEM_DRIVER.as_mut() } {
        driver.check_and_clear_reset_flags()
    } else {
        false
    }
}

/// Perform a software jump to the MCU's built-in factory DFU bootloader.
pub fn enter_dfu_bootloader(profile: &McuProfile) -> ! {
    if let Some(dfu_fn) = unsafe { DFU_JUMP_FN } {
        dfu_fn(profile);
    } else {
        loop {
            core::hint::spin_loop();
        }
    }
}
