//! Manoeuvre planning: finite burns, staging, circularisation and transfer
//! targeting.

pub mod burn;
pub mod capture;
pub mod transfer;
pub mod vehicle;

pub use burn::{circularize, finite_burn, prograde, retrograde, BurnConfig, BurnOutcome, BurnStatus};
pub use capture::{insert_lunar_orbit, LunarOrbit};
pub use transfer::{plan_tli, scan_tli, TliPlan};
pub use vehicle::Vehicle;
