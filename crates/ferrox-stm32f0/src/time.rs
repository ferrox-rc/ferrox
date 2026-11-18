//! Hardware SysTick 1.000 ms monotonic timekeeping driver for STM32F072.

use cortex_m_rt::exception;

/// Initialize Cortex-M SysTick timer for 1 kHz (1.000 ms) ticks at 48 MHz core clock.
/// Configures SysTick to lowest hardware interrupt priority (0xC0) so it never
/// preempts or delays RF time-critical IRQs (TIM16 and EXTI2_3 at 0x80).
pub fn init() {
    ferrox_core::time::init_systick(48_000_000);
}

/// Monotonic system uptime in milliseconds since boot.
#[inline(always)]
pub fn millis() -> u32 {
    ferrox_core::time::millis()
}

#[exception]
fn SysTick() {
    ferrox_core::time::tick_ms();
}
