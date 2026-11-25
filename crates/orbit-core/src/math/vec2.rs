//! Minimal two-dimensional vector algebra.
//!
//! Orbit works in a plane: the Earth-Moon-Sun system is treated as coplanar and
//! prograde. That keeps the state small and the numerics easy to audit while
//! retaining the physics that matters for ascent, transfer and capture.

use std::fmt;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// A two-dimensional vector of `f64` components.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };
    pub const X: Vec2 = Vec2 { x: 1.0, y: 0.0 };
    pub const Y: Vec2 = Vec2 { x: 0.0, y: 1.0 };

    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn splat(v: f64) -> Self {
        Self { x: v, y: v }
    }

    pub fn from_polar(radius: f64, angle: f64) -> Self {
        Self {
            x: radius * angle.cos(),
            y: radius * angle.sin(),
        }
    }

    pub fn dot(self, other: Vec2) -> f64 {
        self.x * other.x + self.y * other.y
    }

    /// Two-dimensional cross product (the z component of the 3D cross).
    pub fn cross(self, other: Vec2) -> f64 {
        self.x * other.y - self.y * other.x
    }

    pub fn norm_sq(self) -> f64 {
        self.dot(self)
    }

    pub fn norm(self) -> f64 {
        self.norm_sq().sqrt()
    }

    pub fn angle(self) -> f64 {
        self.y.atan2(self.x)
    }

    /// Unit vector, or `None` when the vector is negligible or non-finite.
    pub fn try_normalized(self) -> Option<Vec2> {
        let n = self.norm();
        if !n.is_finite() || n < 1.0e-15 {
            None
        } else {
            Some(self / n)
        }
    }

    /// Unit vector with a safe fallback to +X for degenerate input.
    pub fn normalized_or_x(self) -> Vec2 {
        self.try_normalized().unwrap_or(Vec2::X)
    }

    pub fn rotated(self, angle: f64) -> Vec2 {
        let (s, c) = angle.sin_cos();
        Vec2::new(self.x * c - self.y * s, self.x * s + self.y * c)
    }

    /// Counter-clockwise perpendicular.
    pub fn perp(self) -> Vec2 {
        Vec2::new(-self.y, self.x)
    }

    pub fn lerp(self, other: Vec2, t: f64) -> Vec2 {
        self + (other - self) * t
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    pub fn max_component(self) -> f64 {
        self.x.max(self.y)
    }

    pub fn min_component(self) -> f64 {
        self.x.min(self.y)
    }

    /// Component-wise clamp. Returns `None` for non-finite bounds.
    pub fn clamped(self, lo: Vec2, hi: Vec2) -> Option<Vec2> {
        if !lo.is_finite() || !hi.is_finite() || lo.x > hi.x || lo.y > hi.y {
            return None;
        }
        Some(Vec2::new(
            self.x.clamp(lo.x, hi.x),
            self.y.clamp(lo.y, hi.y),
        ))
    }
}

impl fmt::Debug for Vec2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Vec2({:.6}, {:.6})", self.x, self.y)
    }
}

impl fmt::Display for Vec2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:.6}, {:.6})", self.x, self.y)
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl AddAssign for Vec2 {
    fn add_assign(&mut self, rhs: Vec2) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl SubAssign for Vec2 {
    fn sub_assign(&mut self, rhs: Vec2) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl Mul<f64> for Vec2 {
    type Output = Vec2;
    fn mul(self, rhs: f64) -> Vec2 {
        Vec2::new(self.x * rhs, self.y * rhs)
    }
}

impl Mul<Vec2> for f64 {
    type Output = Vec2;
    fn mul(self, rhs: Vec2) -> Vec2 {
        rhs * self
    }
}

impl MulAssign<f64> for Vec2 {
    fn mul_assign(&mut self, rhs: f64) {
        self.x *= rhs;
        self.y *= rhs;
    }
}

impl Div<f64> for Vec2 {
    type Output = Vec2;
    fn div(self, rhs: f64) -> Vec2 {
        Vec2::new(self.x / rhs, self.y / rhs)
    }
}

impl DivAssign<f64> for Vec2 {
    fn div_assign(&mut self, rhs: f64) {
        self.x /= rhs;
        self.y /= rhs;
    }
}

impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        Vec2::new(-self.x, -self.y)
    }
}

impl From<(f64, f64)> for Vec2 {
    fn from(value: (f64, f64)) -> Self {
        Vec2::new(value.0, value.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_is_componentwise() {
        let a = Vec2::new(1.0, 2.0);
        let b = Vec2::new(3.0, -1.0);
        assert_eq!(a + b, Vec2::new(4.0, 1.0));
        assert_eq!(a - b, Vec2::new(-2.0, 3.0));
        assert_eq!(a * 2.0, Vec2::new(2.0, 4.0));
        assert_eq!(2.0 * a, Vec2::new(2.0, 4.0));
        assert_eq!(a / 2.0, Vec2::new(0.5, 1.0));
        assert_eq!(-a, Vec2::new(-1.0, -2.0));
    }

    #[test]
    fn dot_and_cross_match_definitions() {
        let a = Vec2::new(2.0, 3.0);
        let b = Vec2::new(-1.0, 4.0);
        assert_eq!(a.dot(b), 10.0);
        assert_eq!(a.cross(b), 11.0);
        assert_eq!(a.cross(b), -b.cross(a));
    }

    #[test]
    fn normalization_is_safe() {
        assert!(Vec2::ZERO.try_normalized().is_none());
        assert_eq!(Vec2::ZERO.normalized_or_x(), Vec2::X);
        let n = Vec2::new(3.0, 4.0).normalized_or_x();
        assert!((n.norm() - 1.0).abs() < 1e-15);
    }

    #[test]
    fn rotation_preserves_length_and_angle() {
        let v = Vec2::new(1.0, 0.0);
        let r = v.rotated(std::f64::consts::FRAC_PI_2);
        assert!((r.x).abs() < 1e-15);
        assert!((r.y - 1.0).abs() < 1e-15);
        assert!((r.norm() - 1.0).abs() < 1e-15);
        assert!((Vec2::new(1.0, 1.0).angle() - std::f64::consts::FRAC_PI_4).abs() < 1e-15);
    }

    #[test]
    fn perp_is_orthogonal() {
        let v = Vec2::new(3.0, -2.0);
        assert!(v.dot(v.perp()).abs() < 1e-15);
        assert!((v.cross(v.perp()) - v.norm_sq()).abs() < 1e-15);
    }

    #[test]
    fn from_polar_round_trips() {
        let v = Vec2::from_polar(2.5, 0.7);
        assert!((v.norm() - 2.5).abs() < 1e-15);
        assert!((v.angle() - 0.7).abs() < 1e-15);
    }

    #[test]
    fn clamping_rejects_bad_bounds() {
        let v = Vec2::new(5.0, -5.0);
        let c = v.clamped(Vec2::new(0.0, -1.0), Vec2::new(1.0, 1.0)).unwrap();
        assert_eq!(c, Vec2::new(1.0, -1.0));
        assert!(v.clamped(Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)).is_none());
    }
}
