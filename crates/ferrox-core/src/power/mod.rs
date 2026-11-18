//! Electronic power management subsystem for FlySky FS-i6S.

use crate::buzzer::Buzzer;
use crate::storage::{self, RadioStorage};

#[cfg(test)]
use core::sync::atomic::{AtomicBool, Ordering};

#[cfg(test)]
static TEST_PB15_LATCH: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static TEST_LEDS_ON: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static TEST_PB14_PRESSED: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static TEST_POWERED_OFF: AtomicBool = AtomicBool::new(false);

pub use crate::hal::register_power_driver;

/// Required hold duration (in milliseconds) on PB14 to trigger graceful shutdown.
pub const SHUTDOWN_HOLD_MS: u16 = 1500;

/// Initialize GPIO clocks and immediately latch system power ON.
pub fn init() {
    #[cfg(test)]
    {
        TEST_PB15_LATCH.store(true, Ordering::SeqCst);
        TEST_LEDS_ON.store(true, Ordering::SeqCst);
        TEST_POWERED_OFF.store(false, Ordering::SeqCst);
    }
    #[cfg(not(test))]
    if let Some(ref mut d) = crate::hal::get_power_driver() {
        d.init();
    }
}

/// Ensure PB15 is actively driven HIGH.
pub fn latch_on() {
    #[cfg(test)]
    {
        TEST_PB15_LATCH.store(true, Ordering::SeqCst);
    }
    #[cfg(not(test))]
    if let Some(ref mut d) = crate::hal::get_power_driver() {
        d.latch_on();
    }
}

/// Control power button blue LEDs on PD10 and PD11.
pub fn set_leds(on: bool) {
    #[cfg(test)]
    {
        TEST_LEDS_ON.store(on, Ordering::SeqCst);
    }
    #[cfg(not(test))]
    if let Some(ref mut d) = crate::hal::get_power_driver() {
        d.set_leds(on);
    }
}

/// Check if the dual front power buttons are currently pressed (PB14 Active LOW).
pub fn is_power_button_pressed() -> bool {
    #[cfg(test)]
    {
        TEST_PB14_PRESSED.load(Ordering::SeqCst)
    }
    #[cfg(not(test))]
    if let Some(ref mut d) = crate::hal::get_power_driver() {
        d.is_power_key_pressed()
    } else {
        false
    }
}

/// Cut power to the transmitter by driving PB15 LOW.
pub fn power_off() -> ! {
    set_leds(false);

    #[cfg(test)]
    {
        TEST_PB15_LATCH.store(false, Ordering::SeqCst);
        TEST_POWERED_OFF.store(true, Ordering::SeqCst);
        panic!("simulated power_off");
    }

    #[cfg(not(test))]
    {
        if let Some(ref mut d) = crate::hal::get_power_driver() {
            d.latch_off();
        }
        loop {
            cortex_m::asm::wfi();
        }
    }
}

pub struct PowerManager {
    /// Guard: user must release the power buttons after turning on before shutdown can be armed.
    boot_release_detected: bool,
    /// Continuous duration (in ms) the power button has been held.
    hold_duration_ms: u16,
    /// True once shutdown sequence has been initiated.
    is_shutting_down: bool,
    /// True once button press accepted only if flight.telem.connected and flight.is_armed.
    shutdown_pending: bool,
    /// Bitmask of safety interlocks preventing direct shutdown:
    /// Bit 0: Link active (telem connected)
    /// Bit 1: Model armed
    shutdown_guard: u8,
    /// True once user confirms safe to shutdown
    confirm_ready: bool,
}

impl Default for PowerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PowerManager {
    pub const fn new() -> Self {
        Self {
            boot_release_detected: false,
            hold_duration_ms: 0,
            is_shutting_down: false,
            shutdown_pending: false,
            shutdown_guard: 0,
            confirm_ready: false,
        }
    }

    pub fn shutdown_pending(&self) -> bool {
        self.shutdown_pending
    }

    /// Return the active shutdown guard bitmask (Bit 0: Link active, Bit 1: Model armed).
    pub fn shutdown_guard(&self) -> u8 {
        self.shutdown_guard
    }

    /// True if the shutdown sequence has completed or is in progress.
    pub fn is_shutting_down(&self) -> bool {
        self.is_shutting_down
    }

    /// Return the shutdown hold progress as a percentage (0..100) for UI display.
    pub fn shutdown_progress_pct(&self) -> u8 {
        if !self.boot_release_detected {
            0
        } else {
            ((self.hold_duration_ms as u32 * 100) / (SHUTDOWN_HOLD_MS as u32)).min(100) as u8
        }
    }

    /// Update shutdown monitoring.
    pub fn update(
        &mut self,
        dt_ms: u16,
        storage: &mut RadioStorage,
        buzzer: &mut Buzzer,
        shutdown_ok: bool,
        guard_reasons: u8,
    ) {
        if self.is_shutting_down {
            return;
        }

        let pressed = is_power_button_pressed();

        // 1. Enforce initial button release after cold boot
        if !self.boot_release_detected {
            if !pressed {
                self.boot_release_detected = true;
            }
            return;
        }

        // In update(), after the boot-release check:
        let pressed = is_power_button_pressed();

        if !pressed {
            self.hold_duration_ms = 0;
            if self.shutdown_pending {
                self.confirm_ready = true;
            }
            return;
        }

        // A pending request cannot be confirmed until the original press is released.
        if self.shutdown_pending && !self.confirm_ready {
            self.hold_duration_ms = 0;
            return;
        }

        self.hold_duration_ms = self.hold_duration_ms.saturating_add(dt_ms);
        if self.hold_duration_ms < SHUTDOWN_HOLD_MS {
            return;
        }

        if !self.shutdown_pending && !shutdown_ok {
            // First hold requested shutdown, but the safety condition requires confirmation.
            self.shutdown_pending = true;
            self.shutdown_guard = guard_reasons;
            self.confirm_ready = false;
            self.hold_duration_ms = 0;
            buzzer.warn_shutdown_link_active();
            return;
        }

        // Either shutdown was allowed on the first hold, or this is a fresh
        // confirmation hold after release.
        self.is_shutting_down = true;
        buzzer.play_tone_pattern(1800, 80, 40, 2);
        storage::save_storage(storage);
        power_off();
    }
}

#[cfg(test)]
pub fn test_set_power_button_pressed(pressed: bool) {
    TEST_PB14_PRESSED.store(pressed, Ordering::SeqCst);
}

#[cfg(test)]
pub fn test_is_latch_on() -> bool {
    TEST_PB15_LATCH.load(Ordering::SeqCst)
}

#[cfg(test)]
pub fn test_is_leds_on() -> bool {
    TEST_LEDS_ON.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::{assert, assert_eq};

    #[test]
    fn test_power_init_and_latch() {
        init();
        assert!(test_is_latch_on());
        assert!(test_is_leds_on());
    }

    #[test]
    fn test_power_manager_requires_boot_release() {
        init();
        let mut pm = PowerManager::new();
        let mut storage = RadioStorage::empty();
        let mut buzzer = Buzzer::new();

        // Simulate held at boot
        test_set_power_button_pressed(true);
        pm.update(2000, &mut storage, &mut buzzer, true, 0);
        assert!(!pm.is_shutting_down);
        assert_eq!(pm.shutdown_progress_pct(), 0);

        // Release button
        test_set_power_button_pressed(false);
        pm.update(50, &mut storage, &mut buzzer, true, 0);
        assert!(pm.boot_release_detected);

        // Press and hold for partial duration
        test_set_power_button_pressed(true);
        pm.update(750, &mut storage, &mut buzzer, true, 0);
        assert!(!pm.is_shutting_down);
        assert!(pm.shutdown_progress_pct() >= 49);

        // Release early: resets hold timer
        test_set_power_button_pressed(false);
        pm.update(50, &mut storage, &mut buzzer, true, 0);
        assert_eq!(pm.shutdown_progress_pct(), 0);
    }

    #[test]
    fn test_power_manager_shutdown_guard_reasons() {
        init();
        let mut pm = PowerManager::new();
        let mut storage = RadioStorage::empty();
        let mut buzzer = Buzzer::new();

        // Boot release
        test_set_power_button_pressed(false);
        pm.update(50, &mut storage, &mut buzzer, true, 0);
        assert!(pm.boot_release_detected);

        // Hold power button while link active & model armed (reasons: 0x03)
        test_set_power_button_pressed(true);
        pm.update(1600, &mut storage, &mut buzzer, false, 0x03);
        assert!(pm.shutdown_pending());
        assert_eq!(pm.shutdown_guard(), 0x03);
        assert!(!pm.is_shutting_down());
    }
}
