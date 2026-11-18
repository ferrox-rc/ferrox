#![no_std]
#![no_main]

use cortex_m_rt::entry;
use stm32f4 as _;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    ferrox_core::chip::handle_panic()
}

#[entry]
fn main() -> ! {
    let ctx = ferrox_stm32f4::create_active_context();
    ferrox_core::runtime::run(ctx, None)
}
