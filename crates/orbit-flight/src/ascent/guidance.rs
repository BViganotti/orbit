//! Ascent guidance laws.
//!
//! The pitch program is a deterministic function of altitude with a small
//! apoapsis-error correction. It is intentionally simple: a gravity turn that
//! can be reproduced exactly, not a flight computer with hidden state.

use orbit_core::math::Vec2;

/// Pitch above the local horizon in radians.
///
/// 90 degrees is straight up, 0 degrees is prograde horizontal.
pub fn pitch_program(altitude: f64, target_altitude: f64, exponent: f64) -> f64 {
    if !altitude.is_finite() || !target_altitude.is_finite() {
        return std::f64::consts::FRAC_PI_2;
    }
    if altitude <= 1_000.0 {
        return std::f64::consts::FRAC_PI_2;
    }
    let span = (target_altitude - 1_000.0).max(1.0);
    let x = ((altitude - 1_000.0) / span).clamp(0.0, 1.0);
    let pitch = std::f64::consts::FRAC_PI_2 * (1.0 - x.powf(exponent.max(0.1)));
    pitch.clamp(0.0, std::f64::consts::FRAC_PI_2)
}

/// Unit thrust direction from a pitch angle and local radial frame.
pub fn thrust_direction(radial: Vec2, pitch: f64) -> Vec2 {
    let up = radial.normalized_or_x();
    let east = up.perp();
    (up * pitch.sin() + east * pitch.cos()).normalized_or_x()
}

/// Correct pitch toward a target apoapsis altitude.
///
/// `error` is target minus current apoapsis altitude in metres. Positive error
/// (too low) pitches up; negative pitches down. The correction is bounded.
pub fn apoapsis_correction(error: f64, gain_deg: f64) -> f64 {
    if !error.is_finite() {
        return 0.0;
    }
    let normalized = (error / 100_000.0).clamp(-1.0, 1.0);
    normalized * gain_deg.to_radians()
}

/// Throttle to keep dynamic pressure under a limit, with a floor for control.
pub fn throttle_for_q(q: f64, limit: f64) -> f64 {
    if !q.is_finite() || !limit.is_finite() || limit <= 0.0 {
        return 1.0;
    }
    if q <= limit * 0.85 {
        1.0
    } else if q >= limit {
        0.65
    } else {
        let x = (q - limit * 0.85) / (limit * 0.15);
        1.0 - 0.35 * x.clamp(0.0, 1.0)
    }
}

/// Pitch command for a constant-altitude acceleration phase.
///
/// `thrust_acceleration` is the current thrust divided by mass. The vertical
/// component supports gravity, supplies centripetal acceleration and damps
/// radial motion. The result is clamped to `[-25, 80]` degrees.
pub fn hold_altitude_pitch(
    position: Vec2,
    velocity: Vec2,
    target_altitude: f64,
    planet_radius: f64,
    mu: f64,
    thrust_acceleration: f64,
) -> f64 {
    if !position.is_finite()
        || !velocity.is_finite()
        || !thrust_acceleration.is_finite()
        || thrust_acceleration <= 0.0
        || !mu.is_finite()
        || mu <= 0.0
    {
        return std::f64::consts::FRAC_PI_2;
    }
    let r = position.norm();
    if r < 1.0 {
        return std::f64::consts::FRAC_PI_2;
    }
    let up = position / r;
    let east = up.perp();
    let vr = velocity.dot(up);
    let vt = velocity.dot(east);
    let altitude = r - planet_radius;
    let gravity = mu / (r * r);
    let centripetal = vt * vt / r;
    let altitude_error = target_altitude - altitude;
    let required_vertical = gravity - centripetal - 0.08 * vr - 0.001 * altitude_error;
    let ratio = (required_vertical / thrust_acceleration).clamp(-1.0, 1.0);
    ratio.asin().clamp(-0.44, 1.4)
}

/// Flight-path angle (radians above local horizon).
pub fn flight_path_angle(position: Vec2, velocity: Vec2) -> f64 {
    let up = position.normalized_or_x();
    let east = up.perp();
    velocity.dot(up).atan2(velocity.dot(east))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pitch_starts_vertical_and_ends_horizontal() {
        let target = 200_000.0;
        assert!((pitch_program(0.0, target, 0.6) - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert!(pitch_program(100_000.0, target, 0.6) < std::f64::consts::FRAC_PI_2);
        assert!(pitch_program(target, target, 0.6).abs() < 1e-12);
        assert!(pitch_program(2.0 * target, target, 0.6).abs() < 1e-12);
    }

    #[test]
    fn pitch_is_monotone_in_altitude() {
        let target = 200_000.0;
        let mut last = pitch_program(0.0, target, 0.6);
        let mut h = 1000.0;
        while h <= target {
            let p = pitch_program(h, target, 0.6);
            assert!(p <= last + 1e-12);
            last = p;
            h += 1000.0;
        }
    }

    #[test]
    fn thrust_direction_aligns_with_frame() {
        let up = Vec2::X;
        let vertical = thrust_direction(up, std::f64::consts::FRAC_PI_2);
        assert!((vertical - Vec2::X).norm() < 1e-12);
        let horizontal = thrust_direction(up, 0.0);
        assert!((horizontal - Vec2::Y).norm() < 1e-12);
    }

    #[test]
    fn apoapsis_correction_is_bounded() {
        assert_eq!(apoapsis_correction(0.0, 10.0), 0.0);
        assert!(apoapsis_correction(1e9, 10.0) > 0.0);
        assert!(apoapsis_correction(-1e9, 10.0) < 0.0);
        assert!(apoapsis_correction(f64::NAN, 10.0) == 0.0);
    }

    #[test]
    fn throttle_respects_q_limit() {
        assert_eq!(throttle_for_q(1.0, 40_000.0), 1.0);
        assert!(throttle_for_q(40_000.0, 40_000.0) < 1.0);
        assert!(throttle_for_q(100_000.0, 40_000.0) >= 0.65);
        assert_eq!(throttle_for_q(f64::NAN, 40_000.0), 1.0);
    }

    #[test]
    fn flight_path_angle_is_vertical_at_liftoff() {
        let a = flight_path_angle(Vec2::new(6.4e6, 0.0), Vec2::new(10.0, 0.0));
        assert!((a - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        let a = flight_path_angle(Vec2::new(6.4e6, 0.0), Vec2::new(0.0, 10.0));
        assert!(a.abs() < 1e-12);
    }
}
