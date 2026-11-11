//! Shared hardware peripheral drivers for Q X7 STM32F407 derivative targets.

#![no_std]
#![allow(static_mut_refs)]

pub mod adc;
pub mod buzzer;
pub mod chip;
pub mod descriptor;
pub mod display;
pub mod keys;
pub mod power;
pub mod sdcard;
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
pub use descriptor::*;
pub use display::St7567SpiBus;
pub use keys::{PocketKeyDriver, Qx7KeyDriver};
pub use power::Stm32f4PowerDriver;
pub use sdcard::SpiSdCard;
pub use storage::PocketEepromI2cBus;
pub use trainer::Stm32f4TrainerDriver;
pub use uart::PocketSerialPort;
pub use watchdog::{feed as watchdog_feed, start as watchdog_start, WatchdogManager};

#[macro_export]
macro_rules! define_qx7_context {
    ($fn_name:ident, $hal:ty) => {
        pub fn $fn_name() -> ferrox_core::hal::PlatformContext<'static> {
            static mut SYSTEM_DRIVER: $crate::Stm32f4SystemDriver = $crate::Stm32f4SystemDriver;
            static mut BUZZER: $crate::DacToneBuzzer = $crate::DacToneBuzzer;
            static mut DISPLAY: $crate::St7567SpiBus<$hal> = $crate::St7567SpiBus::new();
            static mut STORAGE: $crate::PocketEepromI2cBus<$hal> = $crate::PocketEepromI2cBus::new();
            static mut TRAINER: $crate::Stm32f4TrainerDriver = $crate::Stm32f4TrainerDriver::new();
            static mut RF_BUS: ferrox_core::hal::NullRfBus = ferrox_core::hal::NullRfBus;
            static mut USB: ferrox_core::hal::NullUsbDriver = ferrox_core::hal::NullUsbDriver;
            static mut ADC: $crate::Stm32f4AdcDriver<$hal> = $crate::Stm32f4AdcDriver::new();
            static mut KEYS: $crate::Qx7KeyDriver<$hal> = $crate::Qx7KeyDriver::new();
            static mut UART: $crate::PocketSerialPort<$hal> = $crate::PocketSerialPort::new();
            static mut POWER: $crate::Stm32f4PowerDriver<$hal> = $crate::Stm32f4PowerDriver::new();

            unsafe {
                ferrox_core::watchdog::register_watchdog($crate::watchdog_start, $crate::watchdog_feed);
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
    };
}

define_qx7_context!(create_pocket_context, PocketHal);
define_qx7_context!(create_boxer_context, BoxerHal);
define_qx7_context!(create_zorro_context, ZorroHal);
define_qx7_context!(create_tx12_context, Tx12Hal);
define_qx7_context!(create_qx7_context, Qx7Hal);
define_qx7_context!(create_tprov2_context, TProV2Hal);
define_qx7_context!(create_t12_context, T12Hal);
define_qx7_context!(create_x9d_context, X9dHal);
define_qx7_context!(create_x9e_context, X9eHal);
define_qx7_context!(create_at9spro_context, At9sProHal);
define_qx7_context!(create_gx12_context, Gx12Hal);

/// Create PlatformContext for the active board target feature.
pub fn create_active_context() -> ferrox_core::hal::PlatformContext<'static> {
    #[cfg(feature = "board-boxer")]
    return create_boxer_context();
    #[cfg(feature = "board-zorro")]
    return create_zorro_context();
    #[cfg(feature = "board-tx12")]
    return create_tx12_context();
    #[cfg(feature = "board-qx7")]
    return create_qx7_context();
    #[cfg(feature = "board-tprov2")]
    return create_tprov2_context();
    #[cfg(feature = "board-t12")]
    return create_t12_context();
    #[cfg(feature = "board-x9d")]
    return create_x9d_context();
    #[cfg(feature = "board-x9e")]
    return create_x9e_context();
    #[cfg(feature = "board-at9spro")]
    return create_at9spro_context();
    #[cfg(feature = "board-gx12")]
    return create_gx12_context();
    #[cfg(not(any(
        feature = "board-boxer",
        feature = "board-zorro",
        feature = "board-tx12",
        feature = "board-qx7",
        feature = "board-tprov2",
        feature = "board-t12",
        feature = "board-x9d",
        feature = "board-x9e",
        feature = "board-at9spro",
        feature = "board-gx12"
    )))]
    create_pocket_context()
}
