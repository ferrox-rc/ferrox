#![no_std]
#![no_main]

use stm32f4 as _;

#[link_section = ".uninit"]
static mut PANIC_COUNT: u32 = 0;

const PANIC_MAGIC: u32 = 0x5041_4E00; // 'PAN\0'
const PANIC_MAX_CONSECUTIVE: u32 = 3;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    cortex_m::interrupt::disable();

    unsafe {
        let val = core::ptr::read_volatile(core::ptr::addr_of!(PANIC_COUNT));
        let count = if (val & 0xFFFF_FF00) == PANIC_MAGIC {
            val & 0xFF
        } else {
            0
        };

        if count < PANIC_MAX_CONSECUTIVE {
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!(PANIC_COUNT),
                PANIC_MAGIC | (count + 1),
            );
            cortex_m::peripheral::SCB::sys_reset();
        }

        loop {
            cortex_m::asm::wfi();
        }
    }
}

use cortex_m_rt::entry;

#[entry]
fn main() -> ! {
    let ctx = ferrox_stm32f4::create_pocket_context();
    ferrox_core::runtime::run(ctx, core::ptr::addr_of_mut!(PANIC_COUNT))
}
