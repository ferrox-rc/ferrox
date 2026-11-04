#![cfg_attr(not(test), no_std)]

pub mod board;
pub use board::{ActiveBoard, BoardProfile};

pub mod buzzer;
pub mod calib;
pub mod chip;
pub mod crsf;
pub mod curve;
pub mod display;
pub mod hal;
pub mod hw;
pub use hw::adc;
pub use hw::keys as boot;
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
pub mod vario;
pub mod watchdog;
