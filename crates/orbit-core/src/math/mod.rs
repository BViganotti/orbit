//! Numerical primitives used across the simulation.

pub mod integrator;
pub mod root;
pub mod vec2;

pub use integrator::{integrate_rk4, DormandPrince, OdeSystem, StepReport};
pub use root::{bisect, golden_minimize, newton, secant};
pub use vec2::Vec2;
