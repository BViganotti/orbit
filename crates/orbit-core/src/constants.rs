//! Physical and astronomical constants for the two-dimensional model.
//!
//! Values are rounded IAU/NASA figures. The simulator does not claim
//! flight-dynamics fidelity beyond the documented model in `docs/physics.md`.

/// Earth gravitational parameter, m^3/s^2.
pub const MU_EARTH: f64 = 3.986_004_418e14;
/// Earth mean equatorial radius, m.
pub const R_EARTH: f64 = 6.371_000e6;
/// Earth sidereal rotation rate, rad/s.
pub const OMEGA_EARTH: f64 = 7.292_115_0e-5;
/// Earth sidereal day, s.
pub const EARTH_SIDEREAL_DAY: f64 = 86_164.090_5;

/// Moon gravitational parameter, m^3/s^2.
pub const MU_MOON: f64 = 4.904_869_5e12;
/// Moon mean radius, m.
pub const R_MOON: f64 = 1.737_4e6;
/// Moon sidereal period, s.
pub const MOON_SIDEREAL_PERIOD: f64 = 2_360_591.0;
/// Mean Earth-Moon distance, m.
pub const MOON_ORBIT_RADIUS: f64 = 3.844_0e8;
/// Approximate lunar sphere of influence radius, m.
pub const MOON_SOI: f64 = 6.618_0e7;

/// Sun gravitational parameter, m^3/s^2.
pub const MU_SUN: f64 = 1.327_124_400_18e20;
/// Astronomical unit, m.
pub const AU: f64 = 1.495_978_707e11;
/// Sun radius, m.
pub const R_SUN: f64 = 6.957e8;

/// Standard gravity, m/s^2.
pub const G0: f64 = 9.806_65;
/// Sea-level atmospheric pressure, Pa.
pub const P_SEA_LEVEL: f64 = 101_325.0;
/// Sea-level density, kg/m^3.
pub const RHO_SEA_LEVEL: f64 = 1.225;

/// Seconds per day.
pub const DAY: f64 = 86_400.0;
/// Seconds per Julian year.
pub const YEAR: f64 = 31_557_600.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_physically_consistent() {
        assert!(MU_EARTH > 3.9e14 && MU_EARTH < 4.1e14);
        assert!(R_EARTH > 6.3e6 && R_EARTH < 6.4e6);
        assert!(MU_MOON > 4.8e12 && MU_MOON < 5.0e12);
        assert!(MOON_ORBIT_RADIUS > 3.8e8 && MOON_ORBIT_RADIUS < 3.9e8);
        assert!((MOON_SOI / MOON_ORBIT_RADIUS - 0.172).abs() < 0.01);
        let low = (MU_EARTH / R_EARTH).sqrt();
        assert!((low - 7909.0).abs() < 5.0);
    }
}
