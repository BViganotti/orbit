//! Gravitational bodies and their states.

use orbit_core::constants::{MU_EARTH, MU_MOON, MU_SUN, R_EARTH, R_MOON, R_SUN};
use orbit_core::math::Vec2;

/// The three bodies in the simplified solar system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Body {
    Earth,
    Moon,
    Sun,
}

impl Body {
    pub fn mu(self) -> f64 {
        match self {
            Body::Earth => MU_EARTH,
            Body::Moon => MU_MOON,
            Body::Sun => MU_SUN,
        }
    }

    pub fn radius(self) -> f64 {
        match self {
            Body::Earth => R_EARTH,
            Body::Moon => R_MOON,
            Body::Sun => R_SUN,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Body::Earth => "Earth",
            Body::Moon => "Moon",
            Body::Sun => "Sun",
        }
    }
}

/// Position and velocity of a body in the Earth-centred inertial frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyState {
    pub position: Vec2,
    pub velocity: Vec2,
}

impl BodyState {
    pub const fn new(position: Vec2, velocity: Vec2) -> Self {
        Self { position, velocity }
    }

    pub fn is_finite(&self) -> bool {
        self.position.is_finite() && self.velocity.is_finite()
    }
}

/// Speed of a circular orbit at radius `r` around a body with parameter `mu`.
pub fn circular_speed(mu: f64, r: f64) -> f64 {
    if r <= 0.0 || !r.is_finite() {
        return 0.0;
    }
    (mu / r).sqrt()
}

/// Circular orbital period at radius `r`.
pub fn circular_period(mu: f64, r: f64) -> f64 {
    let v = circular_speed(mu, r);
    if v <= 0.0 {
        return f64::INFINITY;
    }
    std::f64::consts::TAU * r / v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_constants_are_ordered() {
        assert!(Body::Sun.mu() > Body::Earth.mu());
        assert!(Body::Earth.mu() > Body::Moon.mu());
        assert!(Body::Sun.radius() > Body::Earth.radius());
        assert!(Body::Earth.radius() > Body::Moon.radius());
    }

    #[test]
    fn circular_helpers() {
        let v = circular_speed(MU_EARTH, R_EARTH + 200_000.0);
        assert!((v - 7784.0).abs() < 20.0);
        let p = circular_period(MU_EARTH, R_EARTH + 200_000.0);
        assert!((p / 3600.0 - 1.47).abs() < 0.05);
        assert_eq!(circular_speed(1.0, 0.0), 0.0);
        assert_eq!(circular_period(1.0, 0.0), f64::INFINITY);
    }
}
