//! Chip identification, Silicon Unique ID (UID) extraction, and DFU bootloader jump.

use core::mem::MaybeUninit;
use crate::hal::SystemDriver;

pub const DFU_MAGIC_FLAG: u32 = 0xDEAD_BEEF;
pub const PANIC_MAGIC: u32 = 0x5041_4E00; // 'PAN\0'
pub const PANIC_MAX_CONSECUTIVE: u32 = 3;

#[cfg_attr(not(test), link_section = ".uninit")]
static mut DFU_RETAINED_FLAG: MaybeUninit<u32> = MaybeUninit::uninit();

#[cfg_attr(not(test), link_section = ".uninit")]
static mut PANIC_COUNT: MaybeUninit<u32> = MaybeUninit::uninit();

static mut SYSTEM_DRIVER: Option<&'static mut dyn SystemDriver> = None;

/// Register platform-specific system driver.
pub fn register_system_driver(driver: &'static mut dyn SystemDriver) {
    unsafe {
        SYSTEM_DRIVER = Some(driver);
    }
}

/// Read a 96-bit (12-byte) Unique Device ID from the given silicon base address.
///
/// # Safety
/// `base_addr` must point to a valid 12-byte readable memory region on the target MCU.
#[inline]
pub unsafe fn read_uid_from_addr(base_addr: usize) -> [u8; 12] {
    let mut uid = [0u8; 12];
    let src = base_addr as *const u8;
    for (i, byte) in uid.iter_mut().enumerate() {
        *byte = core::ptr::read_volatile(src.add(i));
    }
    uid
}

/// Check the shared `.uninit` SRAM retained DFU magic flag.
#[inline]
pub fn has_sram_dfu_magic() -> bool {
    unsafe {
        core::ptr::read_volatile(core::ptr::addr_of!(DFU_RETAINED_FLAG).cast::<u32>())
            == DFU_MAGIC_FLAG
    }
}

/// Write `DFU_MAGIC_FLAG` to the shared `.uninit` SRAM retained DFU flag.
#[inline]
pub fn set_sram_dfu_magic() {
    unsafe {
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(DFU_RETAINED_FLAG).cast::<u32>(),
            DFU_MAGIC_FLAG,
        );
    }
}

/// Clear the shared `.uninit` SRAM retained DFU flag.
#[inline]
pub fn clear_sram_dfu_magic() {
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!(DFU_RETAINED_FLAG).cast::<u32>(), 0);
    }
}

/// Shared Cortex-M bootloader handoff sequence after MCU-specific `rcc_deinit()`:
/// disables SysTick & NVIC interrupts, invokes `remap_syscfg()`, executes `DSB`/`ISB`
/// memory barriers, loads MSP & Reset Handler from `bootloader_addr`, re-enables
/// interrupts (required by ST ROM USB DFU), and bootstraps into System Memory.
///
/// # Safety
/// Must only be called when ready to abandon the current stack and jump to ROM bootloader.
pub unsafe fn jump_to_system_bootloader(
    bootloader_addr: usize,
    nvic_icer_count: usize,
    remap_syscfg: impl FnOnce(),
) -> ! {
    let mut cp = cortex_m::peripheral::Peripherals::steal();
    cp.SYST.disable_counter();
    cp.SYST.disable_interrupt();
    cp.SYST.clear_current();
    cp.SCB.icsr.write(1 << 25); // PENDSTCLR

    for i in 0..nvic_icer_count {
        cp.NVIC.icer[i].write(0xFFFF_FFFF);
        cp.NVIC.icpr[i].write(0xFFFF_FFFF);
    }

    cortex_m::interrupt::disable();
    cortex_m::asm::dsb();

    remap_syscfg();

    cortex_m::asm::dsb();
    cortex_m::asm::isb();

    let initial_msp = core::ptr::read_volatile(bootloader_addr as *const u32);
    let reset_handler_addr = core::ptr::read_volatile((bootloader_addr + 4) as *const u32);

    cortex_m::interrupt::enable();
    cortex_m::asm::bootstrap(initial_msp as *const u32, reset_handler_addr as *const u32);
}

/// Shared `.uninit` crash-loop guard for `#[panic_handler]` across all board binaries.
/// Performs up to `PANIC_MAX_CONSECUTIVE` rapid warm resets (`SCB::sys_reset()`) to recover
/// flight control in `<20 ms`, then halts in `wfi()` if a deterministic crash loop is detected.
pub fn handle_panic() -> ! {
    cortex_m::interrupt::disable();

    unsafe {
        let val = core::ptr::read_volatile(core::ptr::addr_of!(PANIC_COUNT).cast::<u32>());
        let count = if (val & 0xFFFF_FF00) == PANIC_MAGIC {
            val & 0xFF
        } else {
            0
        };

        if count < PANIC_MAX_CONSECUTIVE {
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!(PANIC_COUNT).cast::<u32>(),
                PANIC_MAGIC | (count + 1),
            );
            cortex_m::peripheral::SCB::sys_reset();
        }

        loop {
            cortex_m::asm::wfi();
        }
    }
}

/// Clear the `.uninit` consecutive panic counter once the flight loop is running stably.
#[inline]
pub fn clear_panic_count() {
    #[cfg(not(test))]
    unsafe {
        let val = core::ptr::read_volatile(core::ptr::addr_of!(PANIC_COUNT).cast::<u32>());
        if val != 0 {
            core::ptr::write_volatile(core::ptr::addr_of_mut!(PANIC_COUNT).cast::<u32>(), 0);
        }
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
