//! Shared hardware peripheral drivers for STM32F072 / APM32F072 targets.
//!
//! Provides hardware-level implementations of ferrox HAL traits:
//! - TIM1 PWM Buzzer (`buzzer`)
//! - ST7567 8080 8-bit parallel display controller & TIM3 backlight (`display`)
//! - I2C1 / I2C2 EEPROM storage driver (`storage`)
//! - SysTick hardware monotonic timer & delay (`time`)
//! - Independent Watchdog (IWDG) (`watchdog`)

#![no_std]

pub mod buzzer;
pub mod display;
pub mod storage;
pub mod time;
pub mod watchdog;

pub use buzzer::Tim1Buzzer;
pub use display::St7567ParallelBus;
pub use storage::EepromI2cBus;
pub use watchdog::{feed as watchdog_feed, start as watchdog_start, WatchdogManager};
