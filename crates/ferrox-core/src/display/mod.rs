//! Display module for ST7567 128×64 monochrome LCD.

pub mod fonts;
pub mod st7567;
pub use st7567::{register_display_bus, St7567};
