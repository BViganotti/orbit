//! Orbit core: two-dimensional vectors, numerical integrators, root finding,
//! rocket mass properties, engines, staging and atmosphere.
//!
//! The crate is deliberately dependency-free and deterministic. All public
//! numerical entry points reject non-finite input instead of propagating NaN.

pub mod aero;
pub mod constants;
pub mod math;
pub mod rocket;

/// Smallest positive number treated as a meaningful physical quantity.
pub const EPS: f64 = 1.0e-12;

/// Standard gravity used for specific impulse conversions (m/s^2).
pub const G0: f64 = 9.806_65;

/// Returns true when every component is finite.
pub fn all_finite(values: &[f64]) -> bool {
    values.iter().all(|v| v.is_finite())
}

/// Clamp `v` into `[lo, hi]`, panicking only on programmer error.
pub fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    debug_assert!(lo <= hi);
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}
