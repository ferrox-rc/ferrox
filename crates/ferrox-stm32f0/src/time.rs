//! Hardware SysTick 1.000 ms monotonic timekeeping driver for STM32F072.

use core::sync::atomic::{AtomicU32, Ordering};
use cortex_m_rt::exception;

static SYSTEM_MILLIS: AtomicU32 = AtomicU32::new(0);

/// Initialize Cortex-M SysTick timer for 1 kHz (1.000 ms) ticks at 48 MHz core clock.
/// Configures SysTick to lowest hardware interrupt priority (0xC0) so it never
/// preempts or delays RF time-critical IRQs (TIM16 and EXTI2_3 at 0x80).
pub fn init() {
    unsafe {
        let mut p = cortex_m::peripheral::Peripherals::steal();
        p.SCB
            .set_priority(cortex_m::peripheral::scb::SystemHandler::SysTick, 0xC0);

        p.SYST.set_reload(48_000 - 1);
        p.SYST.clear_current();
        p.SYST
            .set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);
        p.SYST.enable_interrupt();
        p.SYST.enable_counter();
    }
}

/// Monotonic system uptime in milliseconds since boot.
#[inline(always)]
pub fn millis() -> u32 {
    SYSTEM_MILLIS.load(Ordering::Relaxed)
}

#[exception]
fn SysTick() {
    let current = SYSTEM_MILLIS.load(Ordering::Relaxed);
    SYSTEM_MILLIS.store(current.wrapping_add(1), Ordering::Relaxed);
}
