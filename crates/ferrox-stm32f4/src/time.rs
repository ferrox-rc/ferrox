//! SysTick monotonic timer for STM32F407 (168 MHz core clock).

use cortex_m_rt::exception;

/// Initialize SysTick timer for 1 kHz (1.000 ms) ticks at 168 MHz core clock.
pub fn init() {
    ferrox_core::time::init_systick(168_000_000);
}

/// Monotonic system uptime in milliseconds since boot.
#[inline(always)]
pub fn millis() -> u32 {
    ferrox_core::time::millis()
}

pub fn delay_ms(ms: u32) {
    let start = millis();
    while millis().wrapping_sub(start) < ms {
        cortex_m::asm::nop();
    }
}

#[exception]
fn SysTick() {
    ferrox_core::time::tick_ms();
}
