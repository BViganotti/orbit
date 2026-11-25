//! Rocket hardware: engines, tanks, stages, the assembled vehicle, validation
//! and mass/delta-v bookkeeping.

pub mod checks;
pub mod engine;
pub mod presets;
pub mod rocket;
pub mod stage;

pub use checks::{CheckReport, Issue, Severity};
pub use engine::Engine;
pub use rocket::{Rocket, RocketSummary};
pub use stage::{Propellant, Stage, Tank};
