#![cfg_attr(not(test), no_std)]
#![allow(static_mut_refs)]

pub mod board;
pub use board::{ActiveBoard, BoardProfile};

pub mod buzzer;
pub mod calib;
pub mod chip;
pub mod crsf;
pub mod curve;
pub mod display;
#[cfg(feature = "rf-dsm")]
pub mod dsm;
pub mod hal;
pub mod adc;
pub mod boot;
pub mod hw;
pub mod input;
pub mod mixer;
pub mod rf;
pub mod runtime;
pub use runtime::{BackgroundIdleManager, FlightPipeline, FlightSnapshot};
pub mod safety;
pub mod storage;
pub mod time;
pub mod trainer;
pub mod trim;
pub mod ui;
pub use ui::menu;
pub mod usb;
pub mod uart;
pub mod vario;
pub mod watchdog;
