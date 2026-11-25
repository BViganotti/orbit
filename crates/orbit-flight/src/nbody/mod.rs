//! Earth-Moon-Sun dynamics: bodies, analytic ephemeris, gravity, orbit
//! elements and event-aware propagation.

pub mod body;
pub mod ephemeris;
pub mod gravity;
pub mod propagator;

pub use body::{Body, BodyState};
pub use ephemeris::Ephemeris;
pub use gravity::{elements, Elements};
pub use propagator::{coast, Coast, CoastConfig, Sample, State, StopReason};
