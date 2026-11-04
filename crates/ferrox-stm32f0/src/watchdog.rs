//! Hardware Independent Watchdog (IWDG) driver for STM32F072.
//!
//! Clocked by the internal low-speed RC oscillator (LSI ~40 kHz).
//! Configured with Prescaler /64 and Reload = 1250 for a deterministic 2.0-second timeout.
//! Includes debug halt freeze support via DBGMCU so probe-rs / GDB breakpoints do not trip resets.

use stm32f0xx_hal::pac;

pub struct WatchdogManager;

impl WatchdogManager {
    /// Initialize and start the hardware watchdog with a 2.0-second timeout.
    pub fn init() {
        let rcc = unsafe { &*pac::RCC::ptr() };
        let dbgmcu = unsafe { &*pac::DBGMCU::ptr() };
        let iwdg = unsafe { &*pac::IWDG::ptr() };

        unsafe {
            // 1. Enable DBGMCU clock in RCC_APB2ENR (bit 22: DBGMCUEN)
            rcc.apb2enr.modify(|r, w| w.bits(r.bits() | (1 << 22)));

            // 2. Freeze IWDG counter when CPU core is halted in debug mode
            dbgmcu.apb1_fz.modify(|r, w| w.bits(r.bits() | (1 << 12)));

            // 3. Unlock write access to IWDG_PR and IWDG_RLR registers
            iwdg.kr.write(|w| w.bits(0x5555));

            // 4. Set prescaler to /64 (PR = 4): 40 kHz / 64 = 625 Hz tick (1.6 ms/count)
            iwdg.pr.write(|w| w.bits(4));

            // 5. Set reload value to 1250 (1250 * 1.6 ms = 2.000 seconds timeout)
            iwdg.rlr.write(|w| w.bits(1250));

            // 6. Bounded wait for register synchronization flags
            let mut sync_timeout = 10_000u32;
            while (iwdg.sr.read().bits() & 0x03) != 0 && sync_timeout > 0 {
                sync_timeout -= 1;
            }

            // 7. Initial refresh to reload counter with RLR value
            iwdg.kr.write(|w| w.bits(0xAAAA));

            // 8. Start the watchdog (irreversible until system reset!)
            iwdg.kr.write(|w| w.bits(0xCCCC));
        }
    }
}

/// Standalone zero-cost watchdog feed ("feed the dog").
#[inline(always)]
pub fn feed() {
    unsafe {
        (&*pac::IWDG::ptr()).kr.write(|w| w.bits(0xAAAA));
    }
}

/// Convenience alias to initialize the watchdog.
#[inline(always)]
pub fn start() {
    WatchdogManager::init();
}
