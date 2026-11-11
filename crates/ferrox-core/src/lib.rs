#![cfg_attr(not(test), no_std)]
#![allow(static_mut_refs)]

#[cfg(all(feature = "board-i6x", feature = "board-i6s"))]
compile_error!("Multiple board features cannot be enabled simultaneously (board-i6x and board-i6s).");

#[cfg(all(feature = "board-i6x", feature = "board-pocket"))]
compile_error!("Multiple board features cannot be enabled simultaneously (board-i6x and board-pocket).");

#[cfg(all(feature = "board-i6s", feature = "board-pocket"))]
compile_error!("Multiple board features cannot be enabled simultaneously (board-i6s and board-pocket).");

#[cfg(not(any(
    feature = "board-i6x",
    feature = "board-i6s",
    feature = "board-pocket",
    feature = "board-boxer",
    feature = "board-zorro",
    feature = "board-tx12",
    feature = "board-qx7",
    feature = "board-tprov2",
    feature = "board-t12",
    feature = "board-x9d",
    feature = "board-x9e",
    feature = "board-at9spro",
    feature = "board-gx12",
    test
)))]
compile_error!("A board target must be selected (e.g., board-i6x, board-i6s, board-pocket, board-boxer, board-zorro, board-tx12, board-qx7, board-tprov2, board-t12, board-x9d, board-x9e, board-at9spro, or board-gx12).");

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
pub mod rf;
pub mod runtime;
pub use runtime::{BackgroundIdleManager, FlightPipeline, FlightSnapshot};
pub mod safety;
pub mod storage;
pub mod time;
pub mod trainer;
pub mod trim;
pub mod telemetry_log;
pub use telemetry_log::{FlightLogFrame, TelemetryLogger};
pub mod ui;
pub use ui::menu;
pub mod usb;
pub mod uart;
pub mod vario;
pub mod watchdog;
