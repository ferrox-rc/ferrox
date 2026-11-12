//! Screen implementations for FlySky FS-i6X menu system.

pub mod channels;
pub mod crsf;
pub mod diag;
#[cfg(feature = "dsm-fwd-prog")]
pub mod dsm;
pub mod main_menu;
pub mod mixer;
pub mod model;
pub mod setup;
pub mod soaring;
