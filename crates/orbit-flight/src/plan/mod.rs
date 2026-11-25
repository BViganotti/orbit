//! Manoeuvre planning: finite burns, staging, circularisation and transfer
//! targeting.

pub mod burn;
pub mod vehicle;

pub use burn::{circularize, finite_burn, prograde, retrograde, BurnConfig, BurnOutcome, BurnStatus};
pub use vehicle::Vehicle;
