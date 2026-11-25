//! Drag and static stability.

use crate::math::Vec2;

/// A simple drag model with a constant reference area and coefficient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragModel {
    pub coefficient: f64,
    pub area: f64,
}

impl DragModel {
    pub fn new(coefficient: f64, area: f64) -> Self {
        Self { coefficient, area }
    }

    /// Drag force magnitude in N for a relative speed and density.
    pub fn force_magnitude(&self, density: f64, relative_speed: f64) -> f64 {
        if !density.is_finite() || !relative_speed.is_finite() {
            return 0.0;
        }
        0.5 * density * relative_speed * relative_speed * self.coefficient * self.area
    }

    /// Drag acceleration vector opposing `velocity_rel` (m/s^2).
    pub fn acceleration(&self, density: f64, velocity_rel: Vec2, mass: f64) -> Vec2 {
        if mass <= 0.0 || !mass.is_finite() {
            return Vec2::ZERO;
        }
        let speed = velocity_rel.norm();
        if speed < 1.0e-9 || !speed.is_finite() {
            return Vec2::ZERO;
        }
        let acceleration = self.force_magnitude(density, speed) / mass;
        if !acceleration.is_finite() {
            return Vec2::ZERO;
        }
        -velocity_rel * (acceleration / speed)
    }
}

/// Rough static stability classification from the centre-of-pressure margin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stability {
    Stable,
    Marginal,
    Unstable,
}

/// Classify stability using the normalized static margin
/// `(x_cp - x_cg) / length`. Positive margin means the centre of pressure is
/// behind the centre of mass.
pub fn classify(static_margin: f64) -> Stability {
    if !static_margin.is_finite() {
        return Stability::Unstable;
    }
    if static_margin >= 0.08 {
        Stability::Stable
    } else if static_margin >= 0.0 {
        Stability::Marginal
    } else {
        Stability::Unstable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_opposes_velocity() {
        let model = DragModel::new(0.3, 10.0);
        let v = Vec2::new(100.0, 0.0);
        let a = model.acceleration(1.225, v, 1000.0);
        assert!(a.x < 0.0);
        assert!(a.y.abs() < 1e-15);
        let expected = 0.5 * 1.225 * 10_000.0 * 0.3 * 10.0 / 1000.0;
        assert!((a.norm() - expected).abs() < 1e-12);
    }

    #[test]
    fn drag_is_zero_in_vacuum_or_at_rest() {
        let model = DragModel::new(0.3, 10.0);
        assert_eq!(model.acceleration(0.0, Vec2::new(100.0, 0.0), 1000.0), Vec2::ZERO);
        assert_eq!(model.acceleration(1.0, Vec2::ZERO, 1000.0), Vec2::ZERO);
        assert_eq!(model.acceleration(1.0, Vec2::new(1.0, 0.0), 0.0), Vec2::ZERO);
    }

    #[test]
    fn stability_thresholds() {
        assert_eq!(classify(0.2), Stability::Stable);
        assert_eq!(classify(0.0), Stability::Marginal);
        assert_eq!(classify(-0.1), Stability::Unstable);
        assert_eq!(classify(f64::NAN), Stability::Unstable);
    }
}
