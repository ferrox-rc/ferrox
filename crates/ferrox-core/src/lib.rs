#![cfg_attr(not(test), no_std)]
#![allow(static_mut_refs)]

#[cfg(all(feature = "board-i6x", feature = "board-i6s"))]
compile_error!("Multiple board features cannot be enabled simultaneously (board-i6x and board-i6s).");

#[cfg(all(feature = "board-i6x", feature = "board-pocket"))]
compile_error!("Multiple board features cannot be enabled simultaneously (board-i6x and board-pocket).");

#[cfg(all(feature = "board-i6s", feature = "board-pocket"))]
compile_error!("Multiple board features cannot be enabled simultaneously (board-i6s and board-pocket).");

#[cfg(not(any(feature = "board-i6x", feature = "board-i6s", feature = "board-pocket", test)))]
compile_error!("A board target must be selected (e.g., board-i6x, board-i6s, or board-pocket).");

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
pub mod power;
pub mod touch;
pub mod input;
pub mod mixer;
#[cfg(feature = "multiswitch")]
pub mod multiswitch;
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
