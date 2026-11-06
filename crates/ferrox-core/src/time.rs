//! Platform-agnostic monotonic timekeeping for Ferrox flight controller.

use core::sync::atomic::{AtomicU32, Ordering};

static SYSTEM_MILLIS: AtomicU32 = AtomicU32::new(0);

/// Initialize monotonic timekeeper (delegates to hardware HAL SystemDriver).
#[inline(always)]
pub fn init() {
    crate::chip::init_time();
}

/// Monotonic system uptime in milliseconds since boot.
/// Lock-free, zero-overhead atomic read with Relaxed ordering.
#[inline(always)]
pub fn millis() -> u32 {
    SYSTEM_MILLIS.load(Ordering::Relaxed)
}

/// Advance monotonic millisecond counter by 1 ms.
/// Called directly by the target MCU's SysTick exception handler.
#[inline(always)]
pub fn tick_ms() {
    let current = SYSTEM_MILLIS.load(Ordering::Relaxed);
    SYSTEM_MILLIS.store(current.wrapping_add(1), Ordering::Relaxed);
}

/// Set system uptime in milliseconds (used for testing and initial synchronization).
pub fn set_millis(ms: u32) {
    SYSTEM_MILLIS.store(ms, Ordering::Relaxed);
}

/// Advance system uptime by `ms` (used for test simulation).
pub fn advance_millis(ms: u32) {
    let cur = SYSTEM_MILLIS.load(Ordering::Relaxed);
    SYSTEM_MILLIS.store(cur.wrapping_add(ms), Ordering::Relaxed);
}
