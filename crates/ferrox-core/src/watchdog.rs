//! Platform-agnostic hardware watchdog hooks for Ferrox flight controller.

static mut WATCHDOG_START_HOOK: Option<fn()> = None;
static mut WATCHDOG_FEED_HOOK: Option<fn()> = None;

/// Register MCU hardware watchdog initialization and feed hooks.
pub fn register_watchdog(start_fn: fn(), feed_fn: fn()) {
    unsafe {
        WATCHDOG_START_HOOK = Some(start_fn);
        WATCHDOG_FEED_HOOK = Some(feed_fn);
    }
}

/// Standalone watchdog feed ("feed the dog").
/// Safe and lock-free to call from anywhere in the codebase.
#[inline(always)]
pub fn feed() {
    unsafe {
        if let Some(hook) = WATCHDOG_FEED_HOOK {
            hook();
        }
    }
}

/// Convenience alias to initialize and start the watchdog.
#[inline(always)]
pub fn start() {
    unsafe {
        if let Some(hook) = WATCHDOG_START_HOOK {
            hook();
        }
    }
}
