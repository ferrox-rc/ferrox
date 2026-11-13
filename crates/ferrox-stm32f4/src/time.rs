//! SysTick monotonic timer for STM32F407 (168 MHz core clock).

use cortex_m_rt::exception;

/// Initialize SysTick timer for 1 kHz (1.000 ms) ticks at 168 MHz core clock.
pub fn init() {
    unsafe {
        let mut cp = cortex_m::peripheral::Peripherals::steal();
        // Lowest hardware interrupt priority so it does not preempt RF/DMA ISRs
        cp.SCB
            .set_priority(cortex_m::peripheral::scb::SystemHandler::SysTick, 0xC0);

        // 168 MHz SYSCLK / 1000 = 168,000 counts per 1 ms
        cp.SYST.set_reload(168_000 - 1);
        cp.SYST.clear_current();
        cp.SYST
            .set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);
        cp.SYST.enable_interrupt();
        cp.SYST.enable_counter();
        cortex_m::interrupt::enable();
    }
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
