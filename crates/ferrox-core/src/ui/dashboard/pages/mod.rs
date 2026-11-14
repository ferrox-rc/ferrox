pub mod channels;
#[cfg(any(feature = "soaring", test))]
pub mod f5j;
pub mod gimbals;
pub mod model;
#[cfg(feature = "multiswitch")]
pub mod multiswitch;
pub mod telemetry;
