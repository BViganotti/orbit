//! Numerical primitives used across the simulation.

pub mod integrator;
pub mod rng;
pub mod root;
pub mod vec2;

pub use integrator::{
    integrate_rk4, rk4_step, DormandPrince, Flow, IntegratorError, OdeSystem, StepReport,
};
pub use rng::Rng;
pub use root::{bisect, golden_minimize, newton, secant};
pub use vec2::Vec2;
