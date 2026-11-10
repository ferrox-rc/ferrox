use stm32f4::stm32f407 as pac;

pub struct WatchdogManager;

impl WatchdogManager {
    /// Initialize and start hardware watchdog with ~2.0s timeout using 32 kHz LSI.
    pub fn init() {
        let dbgmcu = unsafe { &*pac::DBGMCU::ptr() };
        let iwdg = unsafe { &*pac::IWDG::ptr() };

        unsafe {
            // Freeze IWDG counter when core is halted in debug mode (APB1_FZ bit 12)
            dbgmcu.apb1_fz.modify(|r, w| w.bits(r.bits() | (1 << 12)));

            // Unlock IWDG registers
            iwdg.kr.write(|w| w.bits(0x5555));

            // Prescaler /64 (PR = 4): 32 kHz / 64 = 500 Hz (2 ms per tick)
            iwdg.pr.write(|w| w.bits(4));

            // Reload value 1000 (1000 * 2 ms = 2.000s)
            iwdg.rlr.write(|w| w.bits(1000));

            let mut timeout = 10_000u32;
            while (iwdg.sr.read().bits() & 0x03) != 0 && timeout > 0 {
                timeout -= 1;
            }

            // Refresh counter
            iwdg.kr.write(|w| w.bits(0xAAAA));

            // Start watchdog
            iwdg.kr.write(|w| w.bits(0xCCCC));
        }
    }
}

#[inline(always)]
pub fn feed() {
    unsafe {
        (&*pac::IWDG::ptr()).kr.write(|w| w.bits(0xAAAA));
    }
}

#[inline(always)]
pub fn start() {
    WatchdogManager::init();
}
