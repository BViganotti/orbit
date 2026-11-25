//! Gravity, osculating elements and orbital diagnostics.

use orbit_core::constants::{MOON_SOI, MU_EARTH, MU_MOON, R_MOON};
use orbit_core::math::Vec2;

use super::body::{Body, BodyState};
use super::ephemeris::Ephemeris;

/// Acceleration in the Earth-centred frame from Earth, Moon and Sun.
///
/// The Moon and Sun terms include the indirect acceleration of the Earth, so a
/// spacecraft at rest at the geocentre experiences zero acceleration. This is
/// the correct relative-frame formulation and avoids a spurious uniform drift.
pub fn acceleration(eph: &Ephemeris, t: f64, position: Vec2) -> Vec2 {
    let r2 = position.norm_sq();
    if !r2.is_finite() || r2 < 1.0 {
        return Vec2::ZERO;
    }
    let r = r2.sqrt();
    let mut acc = -position * (MU_EARTH / (r2 * r));

    let moon = eph.position(Body::Moon, t);
    acc += direct_relative(MU_MOON, position, moon);
    let sun = eph.position(Body::Sun, t);
    acc += direct_relative(orbit_core::constants::MU_SUN, position, sun);

    if acc.is_finite() {
        acc
    } else {
        Vec2::ZERO
    }
}

/// Relative acceleration from a perturber at `body_pos`.
fn direct_relative(mu: f64, position: Vec2, body_pos: Vec2) -> Vec2 {
    let d = position - body_pos;
    let d2 = d.norm_sq();
    if !d2.is_finite() || d2 < 1.0 {
        return Vec2::ZERO;
    }
    let d3 = d2 * d2.sqrt();
    let b2 = body_pos.norm_sq();
    let direct = -d * (mu / d3);
    let indirect = if b2 > 1.0 {
        -body_pos * (mu / (b2 * b2.sqrt()))
    } else {
        Vec2::ZERO
    };
    direct + indirect
}

/// Two-body acceleration about a single body at the origin.
pub fn two_body(mu: f64, position: Vec2) -> Vec2 {
    let r2 = position.norm_sq();
    if !r2.is_finite() || r2 < 1.0 {
        return Vec2::ZERO;
    }
    -position * (mu / (r2 * r2.sqrt()))
}

/// Acceleration relative to the Moon, including Earth and Sun perturbations.
///
/// Used by capture diagnostics; the osculating lunar orbit itself is computed
/// from the instantaneous Moon-relative state.
pub fn acceleration_moon_relative(eph: &Ephemeris, t: f64, rel: Vec2) -> Vec2 {
    let moon = eph.state(Body::Moon, t);
    let earth_from_moon = -moon.position;
    let mut acc = two_body(MU_MOON, rel);
    let earth_rel = rel - earth_from_moon;
    let re2 = earth_rel.norm_sq();
    if re2 > 1.0 {
        acc -= earth_rel * (MU_EARTH / (re2 * re2.sqrt()));
    }
    // Indirect acceleration of the Moon due to Earth.
    let rm2 = moon.position.norm_sq();
    if rm2 > 1.0 {
        acc -= moon.position * (MU_EARTH / (rm2 * rm2.sqrt()));
    }
    let sun = eph.position(Body::Sun, t);
    let sun_rel = rel - (sun - moon.position);
    let rs2 = sun_rel.norm_sq();
    if rs2 > 1.0 {
        acc -= sun_rel * (orbit_core::constants::MU_SUN / (rs2 * rs2.sqrt()));
    }
    if acc.is_finite() {
        acc
    } else {
        Vec2::ZERO
    }
}

/// Classical osculating conic elements in the plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Elements {
    pub semi_major_axis: f64,
    pub eccentricity: f64,
    pub periapsis: f64,
    pub apoapsis: f64,
    pub period: f64,
    pub specific_energy: f64,
    pub angular_momentum: f64,
    pub true_anomaly: f64,
}

impl Elements {
    pub fn is_bound(&self) -> bool {
        self.semi_major_axis.is_finite() && self.semi_major_axis > 0.0 && self.eccentricity < 1.0
    }

    pub fn periapsis_altitude(&self, body_radius: f64) -> f64 {
        self.periapsis - body_radius
    }

    pub fn apoapsis_altitude(&self, body_radius: f64) -> f64 {
        self.apoapsis - body_radius
    }
}

/// Compute osculating elements from a body-relative state.
///
/// Returns `None` for non-finite or degenerate states. A radial state with zero
/// angular momentum is reported with zero angular momentum and infinite
/// apoapsis rather than producing NaN.
pub fn elements(position: Vec2, velocity: Vec2, mu: f64) -> Option<Elements> {
    if !position.is_finite() || !velocity.is_finite() || !mu.is_finite() || mu <= 0.0 {
        return None;
    }
    let r = position.norm();
    if r < 1.0 {
        return None;
    }
    let v2 = velocity.norm_sq();
    let rv = position.dot(velocity);
    let h = position.cross(velocity);
    let energy = 0.5 * v2 - mu / r;
    let a = if energy.abs() < 1.0e-18 {
        f64::INFINITY
    } else {
        -mu / (2.0 * energy)
    };
    let e_vec = position * (v2 - mu / r) - velocity * rv;
    let e = (e_vec / mu).norm();
    if !e.is_finite() || !a.is_finite() && energy.abs() > 1.0e-18 {
        return None;
    }
    let p = h * h / mu;
    let (periapsis, apoapsis) = if e < 1.0 - 1.0e-12 {
        (p / (1.0 + e), p / (1.0 - e))
    } else if (e - 1.0).abs() <= 1.0e-12 {
        (p * 0.5, f64::INFINITY)
    } else {
        (p / (1.0 + e), f64::INFINITY)
    };
    let period = if a.is_finite() && a > 0.0 {
        std::f64::consts::TAU * (a * a * a / mu).sqrt()
    } else {
        f64::INFINITY
    };
    let true_anomaly = if e > 1.0e-10 {
        let e_angle = e_vec.angle();
        let mut nu = position.angle() - e_angle;
        while nu > std::f64::consts::PI {
            nu -= std::f64::consts::TAU;
        }
        while nu < -std::f64::consts::PI {
            nu += std::f64::consts::TAU;
        }
        nu
    } else {
        position.angle()
    };
    let elements = Elements {
        semi_major_axis: a,
        eccentricity: e,
        periapsis,
        apoapsis,
        period,
        specific_energy: energy,
        angular_momentum: h,
        true_anomaly,
    };
    if elements.semi_major_axis.is_nan()
        || elements.eccentricity.is_nan()
        || elements.periapsis.is_nan()
        || elements.apoapsis.is_nan()
        || elements.angular_momentum.is_nan()
    {
        None
    } else {
        Some(elements)
    }
}

/// Moon-relative state from an Earth-centred state.
pub fn moon_relative(eph: &Ephemeris, t: f64, position: Vec2, velocity: Vec2) -> BodyState {
    let moon = eph.state(Body::Moon, t);
    BodyState::new(position - moon.position, velocity - moon.velocity)
}

/// Osculating lunar elements for an Earth-centred state.
pub fn lunar_elements(eph: &Ephemeris, t: f64, position: Vec2, velocity: Vec2) -> Option<Elements> {
    let rel = moon_relative(eph, t, position, velocity);
    elements(rel.position, rel.velocity, MU_MOON)
}

/// Whether a position is inside the lunar sphere of influence.
pub fn inside_moon_soi(eph: &Ephemeris, t: f64, position: Vec2) -> bool {
    (position - eph.position(Body::Moon, t)).norm() < MOON_SOI
}

/// Circular speed about the Moon at a given altitude.
pub fn lunar_circular_speed(altitude: f64) -> f64 {
    if altitude <= -R_MOON {
        return 0.0;
    }
    (MU_MOON / (R_MOON + altitude)).sqrt()
}

/// Circular speed about Earth at a given altitude.
pub fn earth_circular_speed(altitude: f64) -> f64 {
    if altitude <= -orbit_core::constants::R_EARTH {
        return 0.0;
    }
    (MU_EARTH / (orbit_core::constants::R_EARTH + altitude)).sqrt()
}

/// Specific orbital energy about Earth.
pub fn specific_energy(position: Vec2, velocity: Vec2) -> f64 {
    let r = position.norm();
    if r < 1.0 {
        return f64::NAN;
    }
    0.5 * velocity.norm_sq() - MU_EARTH / r
}

/// Specific angular momentum magnitude about Earth.
pub fn specific_angular_momentum(position: Vec2, velocity: Vec2) -> f64 {
    position.cross(velocity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_core::constants::{R_EARTH, R_MOON};

    #[test]
    fn circular_orbit_elements() {
        let r = R_EARTH + 200_000.0;
        let v = (MU_EARTH / r).sqrt();
        let e = elements(Vec2::new(r, 0.0), Vec2::new(0.0, v), MU_EARTH).unwrap();
        assert!(e.eccentricity < 1e-10);
        assert!((e.periapsis - r).abs() < 1e-3);
        assert!((e.apoapsis - r).abs() < 1e-3);
        assert!(e.is_bound());
        assert!((e.period / 3600.0 - 1.47).abs() < 0.02);
    }

    #[test]
    fn elliptical_periapsis_and_apoapsis() {
        let rp = 7.0e6;
        let ra = 1.0e7;
        let a = 0.5 * (rp + ra);
        let vp = (MU_EARTH * (2.0 / rp - 1.0 / a)).sqrt();
        let e = elements(Vec2::new(rp, 0.0), Vec2::new(0.0, vp), MU_EARTH).unwrap();
        assert!((e.periapsis - rp).abs() < 1.0);
        assert!((e.apoapsis - ra).abs() < 10.0);
        assert!((e.eccentricity - (ra - rp) / (ra + rp)).abs() < 1e-10);
    }

    #[test]
    fn escape_is_hyperbolic() {
        let r = R_EARTH + 300_000.0;
        let v = 1.5 * (2.0 * MU_EARTH / r).sqrt();
        let e = elements(Vec2::new(r, 0.0), Vec2::new(0.0, v), MU_EARTH).unwrap();
        assert!(e.eccentricity > 1.0);
        assert!(e.semi_major_axis < 0.0);
        assert_eq!(e.apoapsis, f64::INFINITY);
        assert!(!e.is_bound());
    }

    #[test]
    fn degenerate_states_are_rejected() {
        assert!(elements(Vec2::ZERO, Vec2::ZERO, MU_EARTH).is_none());
        assert!(elements(Vec2::new(f64::NAN, 0.0), Vec2::ZERO, MU_EARTH).is_none());
        assert!(elements(Vec2::new(1.0, 0.0), Vec2::ZERO, 0.0).is_none());
    }

    #[test]
    fn lunar_soi_and_circular_speed() {
        let eph = Ephemeris::default();
        let moon = eph.position(Body::Moon, 0.0);
        assert!(inside_moon_soi(&eph, 0.0, moon));
        assert!(!inside_moon_soi(&eph, 0.0, Vec2::ZERO));
        let v = lunar_circular_speed(100_000.0);
        let expected = (MU_MOON / (R_MOON + 100_000.0)).sqrt();
        assert!((v - expected).abs() < 1e-12);
        assert_eq!(lunar_circular_speed(-R_MOON - 1.0), 0.0);
    }

    #[test]
    fn geocentric_rest_has_no_perturbation() {
        // At the geocentre the indirect terms cancel exactly in exact arithmetic;
        // our guard returns zero for the singular Earth term.
        let eph = Ephemeris::default();
        let a = acceleration(&eph, 0.0, Vec2::ZERO);
        assert!(a.is_finite());
    }

    #[test]
    fn energy_and_momentum_are_consistent() {
        let r = 8.0e6;
        let v = (MU_EARTH / r).sqrt();
        let energy = specific_energy(Vec2::new(r, 0.0), Vec2::new(0.0, v));
        assert!((energy + MU_EARTH / (2.0 * r)).abs() < 1e-6);
        let h = specific_angular_momentum(Vec2::new(r, 0.0), Vec2::new(0.0, v));
        assert!((h - r * v).abs() < 1e-6);
    }
}
