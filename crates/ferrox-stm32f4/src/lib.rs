//! Shared hardware peripheral drivers for RadioMaster Pocket (STM32F407VET6).

#![no_std]
#![allow(static_mut_refs)]

pub mod adc;
pub mod buzzer;
pub mod chip;
pub mod display;
pub mod keys;
pub mod power;
pub mod storage;
pub mod time;
pub mod trainer;
pub mod uart;
pub mod watchdog;

pub use adc::Stm32f4AdcDriver;
pub use buzzer::DacToneBuzzer;
pub use chip::{
    check_and_clear_reset_flags, enter_dfu_bootloader, init_system_clock, Stm32f4SystemDriver,
};
pub use display::St7567SpiBus;
pub use keys::PocketKeyDriver;
pub use power::Stm32f4PowerDriver;
pub use storage::PocketEepromI2cBus;
pub use trainer::Stm32f4TrainerDriver;
pub use uart::PocketSerialPort;
pub use watchdog::{feed as watchdog_feed, start as watchdog_start, WatchdogManager};

/// Create PlatformContext for RadioMaster Pocket.
pub fn create_pocket_context() -> ferrox_core::hal::PlatformContext<'static> {
    static mut SYSTEM_DRIVER: Stm32f4SystemDriver = Stm32f4SystemDriver;
    static mut BUZZER: DacToneBuzzer = DacToneBuzzer;
    static mut DISPLAY: St7567SpiBus = St7567SpiBus;
    static mut STORAGE: PocketEepromI2cBus = PocketEepromI2cBus;
    static mut TRAINER: Stm32f4TrainerDriver = Stm32f4TrainerDriver::new();
    static mut RF_BUS: ferrox_core::hal::NullRfBus = ferrox_core::hal::NullRfBus;
    static mut USB: ferrox_core::hal::NullUsbDriver = ferrox_core::hal::NullUsbDriver;
    static mut ADC: Stm32f4AdcDriver = Stm32f4AdcDriver::new();
    static mut KEYS: PocketKeyDriver = PocketKeyDriver::new();
    static mut UART: PocketSerialPort = PocketSerialPort::new();
    static mut POWER: Stm32f4PowerDriver = Stm32f4PowerDriver;

    unsafe {
        ferrox_core::watchdog::register_watchdog(watchdog_start, watchdog_feed);
        ferrox_core::hal::PlatformContext {
            system: &mut SYSTEM_DRIVER,
            tone: &mut BUZZER,
            display: &mut DISPLAY,
            storage: &mut STORAGE,
            trainer: &mut TRAINER,
            rf_bus: &mut RF_BUS,
            usb: &mut USB,
            adc: &mut ADC,
            keys: &mut KEYS,
            serial: &mut UART,
            power: Some(&mut POWER),
            touch: None,
            watchdog: None,
        }
    }
}
