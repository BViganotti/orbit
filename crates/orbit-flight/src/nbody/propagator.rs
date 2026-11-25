//! Spacecraft propagation through the analytic Earth-Moon-Sun field.
//!
//! The propagator is adaptive and event-aware. It tracks closest lunar approach
//! with cubic Hermite refinement so that perilune targeting is not limited by
//! the integrator's step size.

use orbit_core::constants::{MOON_SOI, R_EARTH, R_MOON};
use orbit_core::math::{DormandPrince, IntegratorError, Vec2};

use super::body::Body;
use super::ephemeris::Ephemeris;
use super::gravity::{acceleration, elements, Elements};

/// Cartesian state in the Earth-centred inertial frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct State {
    pub position: Vec2,
    pub velocity: Vec2,
}

impl State {
    pub const fn new(position: Vec2, velocity: Vec2) -> Self {
        Self { position, velocity }
    }

    pub fn from_arrays(y: &[f64]) -> Option<Self> {
        if y.len() < 4 {
            return None;
        }
        let s = Self::new(Vec2::new(y[0], y[1]), Vec2::new(y[2], y[3]));
        s.is_finite().then_some(s)
    }

    pub fn to_arrays(self) -> [f64; 4] {
        [self.position.x, self.position.y, self.velocity.x, self.velocity.y]
    }

    pub fn is_finite(self) -> bool {
        self.position.is_finite() && self.velocity.is_finite()
    }

    pub fn radius(self) -> f64 {
        self.position.norm()
    }

    pub fn altitude(self) -> f64 {
        self.radius() - R_EARTH
    }

    pub fn speed(self) -> f64 {
        self.velocity.norm()
    }
}

/// One point along a trajectory.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub t: f64,
    pub position: Vec2,
    pub velocity: Vec2,
    pub altitude: f64,
    pub speed: f64,
    pub moon_distance: f64,
}

impl Sample {
    pub fn from_state(t: f64, state: State, eph: &Ephemeris) -> Self {
        let moon = eph.position(Body::Moon, t);
        Self {
            t,
            position: state.position,
            velocity: state.velocity,
            altitude: state.altitude(),
            speed: state.speed(),
            moon_distance: (state.position - moon).norm(),
        }
    }
}

/// Why propagation stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    TimeLimit,
    EarthImpact,
    MoonImpact,
    MoonSoi,
    StepLimit,
}

/// Closest lunar approach found during a coast.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClosestApproach {
    pub time: f64,
    pub state: State,
    pub distance: f64,
    pub altitude: f64,
    pub relative_speed: f64,
}

/// Configuration for a coast.
#[derive(Debug, Clone, Copy)]
pub struct CoastConfig {
    pub rtol: f64,
    pub atol: f64,
    pub max_step: f64,
    pub max_steps: u32,
    pub sample_interval: f64,
    pub max_samples: usize,
    pub stop_at_moon_soi: bool,
}

impl Default for CoastConfig {
    fn default() -> Self {
        Self {
            rtol: 1.0e-11,
            atol: 1.0e-6,
            max_step: 120.0,
            max_steps: 300_000,
            sample_interval: 900.0,
            max_samples: 20_000,
            stop_at_moon_soi: false,
        }
    }
}

/// Result of a coast.
#[derive(Debug, Clone, PartialEq)]
pub struct Coast {
    pub state: State,
    pub t: f64,
    pub reason: StopReason,
    pub steps: u32,
    pub samples: Vec<Sample>,
    pub closest_moon: Option<ClosestApproach>,
    pub moon_soi_time: Option<f64>,
    pub max_altitude: f64,
    pub max_speed: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropagationError {
    Integrator(IntegratorError),
    NonFinite,
    BadInterval,
    SampleLimit,
}

impl From<IntegratorError> for PropagationError {
    fn from(value: IntegratorError) -> Self {
        PropagationError::Integrator(value)
    }
}

impl std::fmt::Display for PropagationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PropagationError::Integrator(e) => write!(f, "integrator: {e}"),
            PropagationError::NonFinite => write!(f, "state became non-finite"),
            PropagationError::BadInterval => write!(f, "invalid propagation interval"),
            PropagationError::SampleLimit => write!(f, "trajectory sample limit exceeded"),
        }
    }
}

impl std::error::Error for PropagationError {}

fn derivative(eph: &Ephemeris, t: f64, y: &[f64], out: &mut [f64]) {
    let position = Vec2::new(y[0], y[1]);
    let acc = acceleration(eph, t, position);
    out[0] = y[2];
    out[1] = y[3];
    out[2] = acc.x;
    out[3] = acc.y;
}

/// Cubic Hermite interpolation between two Cartesian states.
fn hermite(p0: Vec2, v0: Vec2, p1: Vec2, v1: Vec2, dt: f64, s: f64) -> Vec2 {
    let s2 = s * s;
    let s3 = s2 * s;
    let h00 = 2.0 * s3 - 3.0 * s2 + 1.0;
    let h10 = s3 - 2.0 * s2 + s;
    let h01 = -2.0 * s3 + 3.0 * s2;
    let h11 = s3 - s2;
    p0 * h00 + v0 * (h10 * dt) + p1 * h01 + v1 * (h11 * dt)
}

/// Closest approach on one Hermite segment.
fn closest_on_segment(
    eph: &Ephemeris,
    t0: f64,
    s0: State,
    t1: f64,
    s1: State,
) -> Option<(f64, State, f64)> {
    let dt = t1 - t0;
    if dt <= 0.0 || !dt.is_finite() {
        return None;
    }
    let moon0 = eph.position(Body::Moon, t0);
    let moon1 = eph.position(Body::Moon, t1);
    let mut best_s = 0.0;
    let mut best_d = f64::INFINITY;
    // Coarse scan first; only refine when the segment actually comes near the
    // Moon. This keeps remote coasts cheap.
    const N: usize = 8;
    for i in 0..=N {
        let s = i as f64 / N as f64;
        let p = hermite(s0.position, s0.velocity, s1.position, s1.velocity, dt, s);
        let m = moon0.lerp(moon1, s);
        let d = (p - m).norm();
        if d < best_d {
            best_d = d;
            best_s = s;
        }
    }
    let mut lo = (best_s - 1.0 / N as f64).max(0.0);
    let mut hi = (best_s + 1.0 / N as f64).min(1.0);
    let inv_phi = (5.0f64.sqrt() - 1.0) * 0.5;
    let refine = best_d <= 5.0e7;
    for _ in 0..if refine { 36 } else { 0 } {
        let c = hi - inv_phi * (hi - lo);
        let d = lo + inv_phi * (hi - lo);
        let pc = hermite(s0.position, s0.velocity, s1.position, s1.velocity, dt, c);
        let pd = hermite(s0.position, s0.velocity, s1.position, s1.velocity, dt, d);
        let mc = moon0.lerp(moon1, c);
        let md = moon0.lerp(moon1, d);
        let dc = (pc - mc).norm();
        let dd = (pd - md).norm();
        if dc < dd {
            hi = d;
            if dc < best_d {
                best_d = dc;
                best_s = c;
            }
        } else {
            lo = c;
            if dd < best_d {
                best_d = dd;
                best_s = d;
            }
        }
    }
    let s = best_s;
    let position = hermite(s0.position, s0.velocity, s1.position, s1.velocity, dt, s);
    // Derivative of the cubic Hermite position basis.
    let s2 = s * s;
    let dh00 = 6.0 * s2 - 6.0 * s;
    let dh10 = 3.0 * s2 - 4.0 * s + 1.0;
    let dh01 = -6.0 * s2 + 6.0 * s;
    let dh11 = 3.0 * s2 - 2.0 * s;
    let velocity = s0.position * (dh00 / dt)
        + s0.velocity * dh10
        + s1.position * (dh01 / dt)
        + s1.velocity * dh11;
    let t = t0 + s * dt;
    let moon = moon0.lerp(moon1, s);
    let rel = position - moon;
    Some((t, State::new(position, velocity), rel.norm()))
}

/// Dynamic step ceiling: take small steps when close to the Moon.
fn step_limit(eph: &Ephemeris, t: f64, state: State, config: &CoastConfig) -> f64 {
    let moon = eph.position(Body::Moon, t);
    let d = (state.position - moon).norm();
    if d < 5.0e7 {
        let rel_speed = (state.velocity - eph.velocity(Body::Moon, t)).norm().max(10.0);
        let target = (d.max(R_MOON) * 0.003) / rel_speed;
        target.clamp(0.25, config.max_step)
    } else {
        config.max_step
    }
}

/// Propagate a state to `t_end`, stopping early on impacts or the lunar SOI.
pub fn coast(
    eph: &Ephemeris,
    initial: State,
    t0: f64,
    t_end: f64,
    config: &CoastConfig,
) -> Result<Coast, PropagationError> {
    if !initial.is_finite() || !t0.is_finite() || !t_end.is_finite() || t_end < t0 {
        return Err(PropagationError::BadInterval);
    }
    if config.sample_interval <= 0.0 || config.max_samples == 0 || config.max_steps == 0 {
        return Err(PropagationError::BadInterval);
    }
    let controller = DormandPrince {
        rtol: config.rtol,
        atol: config.atol,
        h_min: 1.0e-4,
        h_max: config.max_step,
        max_steps: config.max_steps,
    };
    controller
        .validate()
        .map_err(PropagationError::Integrator)?;
    let mut y = initial.to_arrays();
    let mut t = t0;
    let mut h = 1.0f64.min(config.max_step);
    let mut steps = 0u32;
    let mut samples = Vec::new();
    let mut last_sample_t = t0;
    samples.push(Sample::from_state(t0, initial, eph));
    let mut closest: Option<ClosestApproach> = None;
    let mut soi_time = None;
    let mut max_altitude = initial.altitude();
    let mut max_speed = initial.speed();
    let mut previous = initial;
    let mut reason = StopReason::TimeLimit;
    let mut inside_soi = false;

    while t < t_end {
        if steps >= config.max_steps {
            reason = StopReason::StepLimit;
            break;
        }
        let limit = step_limit(eph, t, previous, config);
        if t + h > t_end {
            h = t_end - t;
        }
        if h > limit {
            h = limit;
        }
        if h < 1.0e-3 {
            h = 1.0e-3;
        }
        let before_t = t;
        match controller.step(
            &|tt: f64, yy: &[f64], out: &mut [f64]| derivative(eph, tt, yy, out),
            &mut t,
            &mut y,
            &mut h,
        )
        {
            Ok(used) => {
                steps += 1;
                let state = State::from_arrays(&y).ok_or(PropagationError::NonFinite)?;
                if !state.is_finite() {
                    return Err(PropagationError::NonFinite);
                }
                // Closest approach refinement over the accepted segment.
                if let Some((ct, cstate, _)) = closest_on_segment(eph, before_t, previous, t, state)
                {
                    let moon = eph.position(Body::Moon, ct);
                    let d = (cstate.position - moon).norm();
                    if closest.as_ref().map_or(true, |c| d < c.distance) {
                        let rel_v =
                            cstate.velocity - eph.velocity(Body::Moon, ct);
                        closest = Some(ClosestApproach {
                            time: ct,
                            state: cstate,
                            distance: d,
                            altitude: d - R_MOON,
                            relative_speed: rel_v.norm(),
                        });
                    }
                }
                let moon = eph.position(Body::Moon, t);
                let moon_d = (state.position - moon).norm();
                if !inside_soi && moon_d < MOON_SOI {
                    inside_soi = true;
                    soi_time = Some(t);
                    if config.stop_at_moon_soi {
                        reason = StopReason::MoonSoi;
                        samples.push(Sample::from_state(t, state, eph));
                        break;
                    }
                } else if inside_soi && moon_d >= MOON_SOI {
                    inside_soi = false;
                }
                if state.radius() <= R_EARTH {
                    reason = StopReason::EarthImpact;
                    break;
                }
                if moon_d <= R_MOON {
                    reason = StopReason::MoonImpact;
                    break;
                }
                if state.altitude() > max_altitude {
                    max_altitude = state.altitude();
                }
                if state.speed() > max_speed {
                    max_speed = state.speed();
                }
                if t - last_sample_t >= config.sample_interval
                    || last_sample_t == t0
                    || samples.len() < 2
                {
                    if samples.len() < config.max_samples {
                        samples.push(Sample::from_state(t, state, eph));
                        last_sample_t = t;
                    } else if t < t_end - config.sample_interval {
                        return Err(PropagationError::SampleLimit);
                    }
                }
                previous = state;
                if used <= 0.0 {
                    return Err(PropagationError::Integrator(
                        IntegratorError::BadStep,
                    ));
                }
            }
            Err(IntegratorError::StepTooSmall) => {
                return Err(PropagationError::Integrator(IntegratorError::StepTooSmall));
            }
            Err(e) => return Err(PropagationError::Integrator(e)),
        }
    }

    let final_state = State::from_arrays(&y).ok_or(PropagationError::NonFinite)?;
    samples.push(Sample::from_state(t, final_state, eph));
    // Drop the trailing sample if it is nearly a duplicate.
    if samples.len() > 2 {
        let n = samples.len();
        if (samples[n - 1].t - samples[n - 2].t).abs() < 1e-9 {
            samples.pop();
        }
    }
    Ok(Coast {
        state: final_state,
        t,
        reason,
        steps,
        samples,
        closest_moon: closest,
        moon_soi_time: soi_time,
        max_altitude,
        max_speed,
    })
}

/// Propagate to a fixed time and return only the final state.
pub fn propagate(
    eph: &Ephemeris,
    initial: State,
    t0: f64,
    t_end: f64,
) -> Result<State, PropagationError> {
    let config = CoastConfig {
        sample_interval: (t_end - t0).max(1.0),
        stop_at_moon_soi: false,
        ..CoastConfig::default()
    };
    Ok(coast(eph, initial, t0, t_end, &config)?.state)
}

/// Osculating Earth elements for a propagated state.
pub fn earth_elements(state: State) -> Option<Elements> {
    elements(
        state.position,
        state.velocity,
        orbit_core::constants::MU_EARTH,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_core::constants::{MU_EARTH, R_EARTH};

    fn circular_state(altitude: f64) -> State {
        let r = R_EARTH + altitude;
        let v = (MU_EARTH / r).sqrt();
        State::new(Vec2::new(r, 0.0), Vec2::new(0.0, v))
    }

    #[test]
    fn circular_orbit_stays_circular_for_one_period() {
        let eph = Ephemeris::default();
        let s = circular_state(400_000.0);
        let period = std::f64::consts::TAU * (R_EARTH + 400_000.0) / s.speed();
        let result = coast(&eph, s, 0.0, period, &CoastConfig::default()).unwrap();
        let elements = earth_elements(result.state).unwrap();
        assert!(elements.eccentricity < 0.01, "e={}", elements.eccentricity);
        assert!((result.state.radius() - (R_EARTH + 400_000.0)).abs() < 50_000.0);
    }

    #[test]
    fn propagation_rejects_bad_inputs() {
        let eph = Ephemeris::default();
        let s = circular_state(200_000.0);
        assert_eq!(
            coast(&eph, s, 1.0, 0.0, &CoastConfig::default()),
            Err(PropagationError::BadInterval)
        );
        let bad = State::new(Vec2::new(f64::NAN, 0.0), Vec2::ZERO);
        assert_eq!(
            coast(&eph, bad, 0.0, 1.0, &CoastConfig::default()),
            Err(PropagationError::BadInterval)
        );
    }

    #[test]
    fn samples_are_bounded_and_finite() {
        let eph = Ephemeris::default();
        let s = circular_state(500_000.0);
        let cfg = CoastConfig {
            sample_interval: 60.0,
            max_samples: 100,
            ..CoastConfig::default()
        };
        let result = coast(&eph, s, 0.0, 6000.0, &cfg).unwrap();
        assert!(result.samples.len() <= 101);
        for sample in &result.samples {
            assert!(sample.position.is_finite());
            assert!(sample.velocity.is_finite());
            assert!(sample.altitude > -R_EARTH);
        }
    }

    #[test]
    fn earth_impact_is_detected() {
        let eph = Ephemeris::default();
        let s = State::new(Vec2::new(R_EARTH + 1000.0, 0.0), Vec2::new(0.0, 0.0));
        let result = coast(
            &eph,
            s,
            0.0,
            3600.0,
            &CoastConfig {
                max_step: 10.0,
                ..CoastConfig::default()
            },
        )
        .unwrap();
        assert_eq!(result.reason, StopReason::EarthImpact);
    }

    #[test]
    fn hermite_interpolation_matches_endpoints() {
        let p0 = Vec2::new(0.0, 0.0);
        let v0 = Vec2::new(1.0, 0.0);
        let p1 = Vec2::new(10.0, 2.0);
        let v1 = Vec2::new(1.0, 2.0);
        assert_eq!(hermite(p0, v0, p1, v1, 10.0, 0.0), p0);
        assert!((hermite(p0, v0, p1, v1, 10.0, 1.0) - p1).norm() < 1e-12);
    }
}
