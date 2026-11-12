//! Shared hardware peripheral drivers for STM32F072 / APM32F072 targets.
//!
//! Provides hardware-level implementations of ferrox HAL traits:
//! - TIM1 PWM Buzzer (`buzzer`)
//! - ST7567 8080 8-bit parallel display controller & TIM3 backlight (`display`)
//! - I2C1 / I2C2 EEPROM storage driver (`storage`)
//! - SysTick hardware monotonic timer & delay (`time`)
//! - Independent Watchdog (IWDG) (`watchdog`)
//! - System clock, reset flags, and DFU bootloader jump (`chip`)
//! - TIM15 PPM in/out trainer driver (`trainer`)
//! - SPI1 & TIM16 RF transceiver frontend (`rf`)
//! - USB 2.0 Full-Speed controller (`usb`)
//! - ADC1 & DMA1 scanner (`adc`)
//! - Key matrix & button drivers (`keys`)
//! - CRSF USART DMA serial ports (`uart`)
//! - Electronic power latch (`power`)
//! - FT6236 capacitive touch controller (`touch`)

#![no_std]
#![allow(static_mut_refs)]

pub mod adc;
pub mod buzzer;
pub mod chip;
pub mod display;
pub mod keys;
pub mod power;
pub mod rf;
pub mod storage;
pub mod time;
pub mod touch;
pub mod trainer;
pub mod uart;
pub mod usb;
pub mod watchdog;

pub use adc::Stm32f0AdcDriver;
pub use buzzer::Tim1Buzzer;
pub use chip::{
    check_and_clear_reset_flags, enter_dfu_bootloader, init_system_clock, Stm32f0SystemDriver,
};
pub use display::St7567ParallelBus;
pub use keys::{I6sKeyDriver, I6xKeyDriver};
pub use power::Stm32f0PowerDriver;
pub use rf::Stm32f0RfBus;
pub use storage::EepromI2cBus;
pub use touch::Stm32f0TouchDriver;
pub use trainer::Stm32f0TrainerDriver;
pub use uart::{I6sSerialPort, I6xSerialPort};
pub use usb::Stm32f0UsbDriver;
pub use watchdog::{feed as watchdog_feed, start as watchdog_start, WatchdogManager};

/// Create PlatformContext for FlySky FS-i6X.
pub fn create_i6x_context() -> ferrox_core::hal::PlatformContext {
    static mut SYSTEM_DRIVER: Stm32f0SystemDriver = Stm32f0SystemDriver;
    static mut BUZZER: Tim1Buzzer = Tim1Buzzer;
    static mut DISPLAY: St7567ParallelBus = St7567ParallelBus;
    static mut STORAGE: EepromI2cBus = EepromI2cBus;
    static mut TRAINER: Stm32f0TrainerDriver = Stm32f0TrainerDriver::new();
    static mut RF_BUS: Stm32f0RfBus = Stm32f0RfBus;
    static mut USB: Stm32f0UsbDriver = Stm32f0UsbDriver;
    static mut ADC: Stm32f0AdcDriver = Stm32f0AdcDriver::new(false);
    static mut KEYS: I6xKeyDriver = I6xKeyDriver::new();
    static mut UART: I6xSerialPort = I6xSerialPort::new();

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
            power: None,
            touch: None,
            watchdog: None,
        }
    }
}

/// Create PlatformContext for FlySky FS-i6S.
pub fn create_i6s_context() -> ferrox_core::hal::PlatformContext {
    static mut SYSTEM_DRIVER: Stm32f0SystemDriver = Stm32f0SystemDriver;
    static mut BUZZER: Tim1Buzzer = Tim1Buzzer;
    static mut DISPLAY: St7567ParallelBus = St7567ParallelBus;
    static mut STORAGE: EepromI2cBus = EepromI2cBus;
    static mut TRAINER: Stm32f0TrainerDriver = Stm32f0TrainerDriver::new();
    static mut RF_BUS: Stm32f0RfBus = Stm32f0RfBus;
    static mut USB: Stm32f0UsbDriver = Stm32f0UsbDriver;
    static mut ADC: Stm32f0AdcDriver = Stm32f0AdcDriver::new(true);
    static mut KEYS: I6sKeyDriver = I6sKeyDriver::new();
    static mut UART: I6sSerialPort = I6sSerialPort::new();
    static mut POWER: Stm32f0PowerDriver = Stm32f0PowerDriver;
    static mut TOUCH: Stm32f0TouchDriver = Stm32f0TouchDriver;

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
            touch: Some(&mut TOUCH),
            watchdog: None,
        }
    }
}
