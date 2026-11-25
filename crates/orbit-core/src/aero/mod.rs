//! Atmosphere and aerodynamics.

pub mod atmosphere;
pub mod drag;

pub use atmosphere::{density, dynamic_pressure, pressure_ratio, speed_of_sound};
pub use drag::{DragModel, Stability};
