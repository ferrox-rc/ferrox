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

/// Initialize Cortex-M SysTick timer for 1 kHz (1.000 ms) ticks at `core_clock_hz`.
/// Configures SysTick to lowest hardware interrupt priority (`0xC0`) so it never
/// preempts time-critical RF or DMA ISRs.
pub fn init_systick(core_clock_hz: u32) {
    #[cfg(not(test))]
    unsafe {
        let mut p = cortex_m::peripheral::Peripherals::steal();
        p.SCB
            .set_priority(cortex_m::peripheral::scb::SystemHandler::SysTick, 0xC0);

        p.SYST.set_reload((core_clock_hz / 1000).saturating_sub(1));
        p.SYST.clear_current();
        p.SYST
            .set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);
        p.SYST.enable_interrupt();
        p.SYST.enable_counter();
        cortex_m::interrupt::enable();
    }
    #[cfg(test)]
    let _ = core_clock_hz;
}
