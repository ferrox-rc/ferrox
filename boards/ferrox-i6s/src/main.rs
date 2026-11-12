#![no_std]
#![no_main]

use stm32f0xx_hal as _;

use core::mem::MaybeUninit;

// Panic loop guard placed in .uninit section so RAM value survives warm resets
#[link_section = ".uninit"]
static mut PANIC_COUNT: MaybeUninit<u32> = MaybeUninit::uninit();

const PANIC_MAGIC: u32 = 0x5041_4E00; // 'PAN\0'
const PANIC_MAX_CONSECUTIVE: u32 = 3;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // Disable interrupts to prevent ISR execution during fault handling
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
            // Trigger rapid Cortex-M0 warm reset (<20ms recovery vs 2.0s watchdog)
            cortex_m::peripheral::SCB::sys_reset();
        }

        // Exceeded consecutive panic threshold: deterministic crash loop detected.
        // Halt and wait for hardware watchdog or power cycle rather than thrashing in an infinite loop.
        loop {
            cortex_m::asm::wfi();
        }
    }
}

fn clear_panic_count() {
    unsafe {
        let val = core::ptr::read_volatile(core::ptr::addr_of!(PANIC_COUNT).cast::<u32>());
        if val != 0 {
            core::ptr::write_volatile(core::ptr::addr_of_mut!(PANIC_COUNT).cast::<u32>(), 0);
        }
    }
}

use cortex_m_rt::entry;

#[entry]
fn main() -> ! {
    let ctx = ferrox_stm32f0::create_i6s_context();
    ferrox_core::runtime::run(ctx, Some(clear_panic_count))
}
