//! Simplified analytic ephemeris for the Earth-Moon-Sun system.
//!
//! The Moon and Sun move on coplanar circular orbits in the Earth-centred
//! inertial frame. The model is documented in `docs/physics.md`; it is a
//! planning approximation, not a high-precision ephemeris. Epochs are free
//! parameters, which lets the planner choose a favourable lunar phase.

use orbit_core::constants::{
    AU, DAY, MOON_ORBIT_RADIUS, MOON_SIDEREAL_PERIOD, MU_EARTH, YEAR,
};
use orbit_core::math::Vec2;

use super::body::{Body, BodyState};

/// Analytic circular ephemeris. Angles are radians at `t = 0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ephemeris {
    pub moon_phase: f64,
    pub sun_phase: f64,
    pub moon_radius: f64,
    pub sun_radius: f64,
}

impl Default for Ephemeris {
    fn default() -> Self {
        Self {
            moon_phase: 0.0,
            sun_phase: std::f64::consts::PI,
            moon_radius: MOON_ORBIT_RADIUS,
            sun_radius: AU,
        }
    }
}

impl Ephemeris {
    pub fn new(moon_phase: f64, sun_phase: f64) -> Self {
        Self {
            moon_phase,
            sun_phase,
            ..Self::default()
        }
    }

    pub fn moon_mean_motion(&self) -> f64 {
        std::f64::consts::TAU / MOON_SIDEREAL_PERIOD
    }

    pub fn sun_mean_motion(&self) -> f64 {
        std::f64::consts::TAU / YEAR
    }

    pub fn moon_angle(&self, t: f64) -> f64 {
        self.moon_phase + self.moon_mean_motion() * t
    }

    pub fn sun_angle(&self, t: f64) -> f64 {
        self.sun_phase + self.sun_mean_motion() * t
    }

    pub fn state(&self, body: Body, t: f64) -> BodyState {
        match body {
            Body::Earth => BodyState::new(Vec2::ZERO, Vec2::ZERO),
            Body::Moon => {
                let theta = self.moon_angle(t);
                let n = self.moon_mean_motion();
                BodyState::new(
                    Vec2::from_polar(self.moon_radius, theta),
                    Vec2::from_polar(self.moon_radius * n, theta + std::f64::consts::FRAC_PI_2),
                )
            }
            Body::Sun => {
                let theta = self.sun_angle(t);
                let n = self.sun_mean_motion();
                BodyState::new(
                    Vec2::from_polar(self.sun_radius, theta),
                    Vec2::from_polar(self.sun_radius * n, theta + std::f64::consts::FRAC_PI_2),
                )
            }
        }
    }

    pub fn position(&self, body: Body, t: f64) -> Vec2 {
        self.state(body, t).position
    }

    pub fn velocity(&self, body: Body, t: f64) -> Vec2 {
        self.state(body, t).velocity
    }

    pub fn earth(&self) -> BodyState {
        BodyState::new(Vec2::ZERO, Vec2::ZERO)
    }

    pub fn is_finite(&self) -> bool {
        self.moon_phase.is_finite()
            && self.sun_phase.is_finite()
            && self.moon_radius.is_finite()
            && self.sun_radius.is_finite()
            && self.moon_radius > 0.0
            && self.sun_radius > 0.0
    }

    /// Validate a user-supplied ephemeris.
    pub fn validate(&self) -> Result<(), String> {
        if !self.is_finite() {
            return Err("ephemeris contains non-finite or non-positive values".into());
        }
        if !(3.0e8..4.5e8).contains(&self.moon_radius) {
            return Err("moon radius outside [300,000 km, 450,000 km]".into());
        }
        if !(1.0e11..2.0e11).contains(&self.sun_radius) {
            return Err("sun distance outside [0.67 AU, 1.34 AU]".into());
        }
        Ok(())
    }

    /// Find the time at which the Moon reaches `angle` (monotone, `t >= t0`).
    pub fn next_moon_angle_time(&self, angle: f64, t0: f64) -> f64 {
        let n = self.moon_mean_motion();
        let current = self.moon_angle(t0);
        let target = angle;
        let mut delta = (target - current).rem_euclid(std::f64::consts::TAU);
        if delta < 1.0e-12 {
            delta = 0.0;
        }
        t0 + delta / n
    }

    /// Advance the Moon phase so that its angle at `t` equals `angle`.
    pub fn with_moon_at(mut self, angle: f64, t: f64) -> Self {
        self.moon_phase = angle - self.moon_mean_motion() * t;
        self
    }
}

/// Human-readable sanity checks used by the UI.
pub fn moon_distance_km(eph: &Ephemeris, t: f64) -> f64 {
    eph.position(Body::Moon, t).norm() / 1000.0
}

/// Time for the Moon to complete one revolution, days.
pub fn moon_period_days() -> f64 {
    MOON_SIDEREAL_PERIOD / DAY
}

/// Gravitational parameter of Earth, re-exported for convenience.
pub fn earth_mu() -> f64 {
    MU_EARTH
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moon_and_sun_distances_are_sane() {
        let e = Ephemeris::default();
        assert!((e.position(Body::Moon, 0.0).norm() - MOON_ORBIT_RADIUS).abs() < 1.0);
        assert!((e.position(Body::Sun, 0.0).norm() - AU).abs() < 1.0);
        assert_eq!(e.position(Body::Earth, 0.0), Vec2::ZERO);
    }

    #[test]
    fn velocities_are_perpendicular_and_circular() {
        let e = Ephemeris::default();
        let m = e.state(Body::Moon, 1234.0);
        assert!(m.position.dot(m.velocity).abs() < 100.0);
        let speed = m.velocity.norm();
        let expected = MOON_ORBIT_RADIUS * e.moon_mean_motion();
        assert!((speed - expected).abs() < 1e-6);
    }

    #[test]
    fn moon_period_is_about_27_days() {
        let e = Ephemeris::default();
        let p = MOON_SIDEREAL_PERIOD;
        let a = e.position(Body::Moon, 0.0);
        let b = e.position(Body::Moon, p);
        assert!((a - b).norm() < 100.0);
        assert!((moon_period_days() - 27.32).abs() < 0.1);
    }

    #[test]
    fn next_angle_time_wraps_forward() {
        let e = Ephemeris::default();
        let t = e.next_moon_angle_time(1.0, 0.0);
        assert!(t > 0.0);
        assert!((e.moon_angle(t) - 1.0).abs() < 1e-9);
        let later = e.next_moon_angle_time(1.0, t + 1.0);
        assert!(later > t + 1.0);
    }

    #[test]
    fn with_moon_at_sets_phase() {
        let e = Ephemeris::default().with_moon_at(2.0, 1000.0);
        assert!((e.moon_angle(1000.0) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn validation_rejects_bad_ephemeris() {
        let mut e = Ephemeris::default();
        e.moon_radius = 0.0;
        assert!(e.validate().is_err());
        let mut e = Ephemeris::default();
        e.moon_phase = f64::NAN;
        assert!(e.validate().is_err());
        assert!(Ephemeris::default().validate().is_ok());
    }
}
