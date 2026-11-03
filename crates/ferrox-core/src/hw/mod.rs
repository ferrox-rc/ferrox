//! Board hardware abstraction multiplexer for Ferrox targets.

#[cfg(feature = "board-i6s")]
pub mod i6s;
#[cfg(feature = "board-i6s")]
pub use i6s::*;

#[cfg(not(feature = "board-i6s"))]
pub mod i6x;
#[cfg(not(feature = "board-i6s"))]
pub use i6x::*;
