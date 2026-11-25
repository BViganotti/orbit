//! Trans-lunar injection targeting.
//!
//! The planner starts from a circular Earth parking orbit, chooses a departure
//! point from the analytic lunar phase, then solves a finite TLI burn so that
//! the n-body coast passes the Moon at the requested perilune altitude. The
//! search is a bracketed bisection on burn magnitude with a broad scan to
//! establish the bracket. All evaluations are real propagations; the planner
//! reports its residual instead of claiming success.

use orbit_core::constants::{MOON_ORBIT_RADIUS, MU_EARTH, R_MOON};
use orbit_core::math::Vec2;
use orbit_core::rocket::Rocket;

use crate::nbody::gravity::elements;
use crate::nbody::propagator::{coast, CoastConfig, Coast, State};
use crate::nbody::Ephemeris;

use super::burn::{finite_burn, prograde, BurnConfig, BurnStatus};
use super::vehicle::Vehicle;

/// A solved trans-lunar injection.
#[derive(Debug, Clone, PartialEq)]
pub struct TliPlan {
    /// Coast time from the supplied epoch to the ignition point, s.
    pub coast_duration: f64,
    /// Absolute ignition time, s.
    pub burn_time: f64,
    /// State at ignition (before the burn).
    pub burn_state: State,
    /// Vehicle at ignition (before the burn).
    pub burn_vehicle: Vehicle,
    /// Burn direction (unit, inertial).
    pub direction: Vec2,
    /// Finite-burn delta-v target.
    pub burn_delta_v: f64,
    /// Resulting closest lunar approach.
    pub closest: ClosestLunar,
    /// Evaluations used by the search.
    pub evaluations: u32,
    /// Final residual in metres.
    pub residual: f64,
}

/// Closest lunar approach and associated state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClosestLunar {
    pub time: f64,
    pub state: State,
    pub distance: f64,
    pub altitude: f64,
    pub relative_speed: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    InvalidInput(String),
    NoEncounter,
    PropellantExhausted,
    NoBracket,
    ResidualTooLarge(i64),
    Propagation(String),
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanError::InvalidInput(s) => write!(f, "invalid input: {s}"),
            PlanError::NoEncounter => write!(f, "no lunar encounter"),
            PlanError::PropellantExhausted => write!(f, "propellant exhausted"),
            PlanError::NoBracket => write!(f, "could not bracket the target"),
            PlanError::ResidualTooLarge(cm) => {
                write!(f, "target residual too large: {} m", cm)
            }
            PlanError::Propagation(s) => write!(f, "propagation failed: {s}"),
        }
    }
}

impl std::error::Error for PlanError {}

/// One targeting evaluation.
#[derive(Debug, Clone)]
pub struct Evaluation {
    pub closest: ClosestLunar,
    pub burn_end_time: f64,
    pub burn_end_state: State,
    pub vehicle: Vehicle,
    pub achieved_delta_v: f64,
    pub burn_duration: f64,
    pub coast: Coast,
}

fn coast_config() -> CoastConfig {
    CoastConfig {
        rtol: 1.0e-9,
        atol: 1.0e-3,
        max_step: 300.0,
        max_steps: 200_000,
        sample_interval: 3_600.0,
        max_samples: 4_000,
        stop_at_moon_soi: false,
    }
}

/// Coast along the parking orbit to the requested ignition time.
fn coast_parking(
    eph: &Ephemeris,
    state: State,
    t0: f64,
    duration: f64,
) -> Result<State, PlanError> {
    if duration <= 0.0 {
        return Ok(state);
    }
    let config = CoastConfig {
        sample_interval: duration.max(1.0),
        max_step: 120.0,
        rtol: 1.0e-10,
        atol: 1.0e-4,
        ..CoastConfig::default()
    };
    let result = coast(eph, state, t0, t0 + duration, &config)
        .map_err(|e| PlanError::Propagation(e.to_string()))?;
    Ok(result.state)
}

/// Evaluate one finite TLI burn and the following coast.
pub fn evaluate(
    eph: &Ephemeris,
    rocket: &Rocket,
    burn_vehicle: &Vehicle,
    burn_time: f64,
    burn_state: State,
    burn_delta_v: f64,
    tof_nominal: f64,
) -> Result<Evaluation, PlanError> {
    if !(burn_delta_v.is_finite() && burn_delta_v >= 0.0) {
        return Err(PlanError::InvalidInput("burn delta-v".into()));
    }
    let direction = prograde(burn_state);
    let mut vehicle = *burn_vehicle;
    let burn = finite_burn(
        eph,
        rocket,
        &mut vehicle,
        burn_time,
        burn_state,
        direction,
        burn_delta_v,
        &BurnConfig::default(),
    )
    .map_err(PlanError::InvalidInput)?;
    if !matches!(burn.status, BurnStatus::Complete) {
        return Err(PlanError::PropellantExhausted);
    }
    let end = burn.t;
    // Allow enough time for the outbound leg and the lunar encounter.
    let end_time = end + tof_nominal + 4.0 * 86_400.0;
    let result = coast(eph, burn.state, end, end_time, &coast_config())
        .map_err(|e| PlanError::Propagation(e.to_string()))?;
    let closest = result
        .closest_moon
        .ok_or(PlanError::NoEncounter)?;
    if !closest.distance.is_finite() || closest.distance > 1.0e10 {
        return Err(PlanError::NoEncounter);
    }
    Ok(Evaluation {
        closest: ClosestLunar {
            time: closest.time,
            state: closest.state,
            distance: closest.distance,
            altitude: closest.altitude,
            relative_speed: closest.relative_speed,
        },
        burn_end_time: end,
        burn_end_state: burn.state,
        vehicle,
        achieved_delta_v: burn.delta_v,
        burn_duration: burn.duration,
        coast: result,
    })
}

/// Nominal parking-orbit angular rate.
fn parking_mean_motion(radius: f64) -> f64 {
    (MU_EARTH / (radius * radius * radius)).sqrt()
}

/// Time until the parking orbit reaches the departure phase for a Hohmann-like
/// transfer to the Moon.
pub fn phase_wait(eph: &Ephemeris, state: State, t0: f64) -> Option<(f64, f64)> {
    let r0 = state.radius();
    if !r0.is_finite() || r0 < 6.5e6 {
        return None;
    }
    let n0 = parking_mean_motion(r0);
    let n_moon = eph.moon_mean_motion();
    let tof = std::f64::consts::PI * ((0.5 * (r0 + MOON_ORBIT_RADIUS)).powi(3) / MU_EARTH).sqrt();
    let moon0 = eph.moon_angle(t0);
    let theta0 = state.position.angle();
    let delta = (moon0 + n_moon * tof - std::f64::consts::PI - theta0)
        .rem_euclid(std::f64::consts::TAU);
    let rate = n0 - n_moon;
    if rate.abs() < 1.0e-12 {
        return None;
    }
    let synodic = std::f64::consts::TAU / rate.abs();
    let mut wait = if rate > 0.0 {
        delta / rate
    } else {
        (std::f64::consts::TAU - delta) / rate.abs()
    };
    wait = wait.rem_euclid(synodic);
    Some((wait, tof))
}

/// Impulsive Hohmann TLI delta-v from a circular orbit at `state`.
pub fn hohmann_delta_v(state: State) -> f64 {
    let r0 = state.radius();
    if !r0.is_finite() || r0 <= 0.0 {
        return 0.0;
    }
    let a = 0.5 * (r0 + MOON_ORBIT_RADIUS);
    let v0 = (MU_EARTH / r0).sqrt();
    let vt = (MU_EARTH * (2.0 / r0 - 1.0 / a)).sqrt();
    (vt - v0).max(0.0)
}

/// Plan a trans-lunar injection.
pub fn plan_tli(
    eph: &Ephemeris,
    rocket: &Rocket,
    vehicle: &Vehicle,
    t0: f64,
    state: State,
    target_perilune_altitude: f64,
) -> Result<TliPlan, PlanError> {
    rocket
        .validate()
        .map_err(|e| PlanError::InvalidInput(e))?;
    vehicle
        .validate(rocket)
        .map_err(PlanError::InvalidInput)?;
    if !state.is_finite() || !t0.is_finite() {
        return Err(PlanError::InvalidInput("state or time is not finite".into()));
    }
    if !(0.0..=1_000_000.0).contains(&target_perilune_altitude) {
        return Err(PlanError::InvalidInput("perilune altitude".into()));
    }
    let el = elements(state.position, state.velocity, MU_EARTH)
        .ok_or_else(|| PlanError::InvalidInput("degenerate parking orbit".into()))?;
    if el.eccentricity > 0.08 {
        return Err(PlanError::InvalidInput(
            "parking orbit is not circular enough for TLI planning".into(),
        ));
    }
    let (wait, tof) = phase_wait(eph, state, t0)
        .ok_or_else(|| PlanError::InvalidInput("parking orbit geometry".into()))?;
    let burn_time = t0 + wait;
    let burn_state = coast_parking(eph, state, t0, wait)?;
    if !burn_state.is_finite() {
        return Err(PlanError::Propagation("parking coast diverged".into()));
    }
    let dv0 = hohmann_delta_v(burn_state);
    if dv0 <= 0.0 || !dv0.is_finite() {
        return Err(PlanError::InvalidInput("degenerate Hohmann delta-v".into()));
    }
    let mut evaluations = 0u32;
    let target_radius = R_MOON + target_perilune_altitude;
    let mut evaluate_dv = |dv: f64| -> Result<(Evaluation, f64), PlanError> {
        evaluations += 1;
        let ev = evaluate(eph, rocket, vehicle, burn_time, burn_state, dv, tof)?;
        let residual = ev.closest.distance - target_radius;
        Ok((ev, residual))
    };

    // Coarse scan for the burn magnitude that brings the trajectory closest to
    // the Moon. The encounter is narrow in delta-v, so a direct sign-change
    // scan is unreliable; minimising miss distance first is robust.
    let mut best: Option<(Evaluation, f64, f64)> = None;
    let mut best_distance = f64::INFINITY;
    let mut best_dv = dv0;
    let mut step = dv0 * 0.005;
    let mut failures = 0u32;
    for i in 0..=60 {
        let factor = 0.90 + 0.005 * i as f64;
        let dv = dv0 * factor;
        match evaluate_dv(dv) {
            Ok((ev, residual)) => {
                let distance = ev.closest.distance;
                if distance < best_distance {
                    best_distance = distance;
                    best_dv = dv;
                    best = Some((ev, dv, residual));
                }
            }
            Err(_) => failures += 1,
        }
    }
    if !best_dv.is_finite() || best_distance > 1.0e9 {
        return Err(if failures > 0 {
            PlanError::PropellantExhausted
        } else {
            PlanError::NoEncounter
        });
    }

    // Golden-section refinement of the miss-distance minimum.
    let mut lo = (best_dv - step).max(0.0);
    let mut hi = best_dv + step;
    let inv_phi = (5.0f64.sqrt() - 1.0) * 0.5;
    let mut c = hi - inv_phi * (hi - lo);
    let mut d = lo + inv_phi * (hi - lo);
    let mut fc = evaluate_dv(c).map(|(_, r)| r + target_radius).unwrap_or(f64::INFINITY);
    let mut fd = evaluate_dv(d).map(|(_, r)| r + target_radius).unwrap_or(f64::INFINITY);
    for _ in 0..40 {
        if fc < fd {
            hi = d;
            d = c;
            fd = fc;
            c = hi - inv_phi * (hi - lo);
            fc = evaluate_dv(c)
                .map(|(ev, r)| {
                    if r + target_radius < best_distance {
                        best_distance = r + target_radius;
                        best_dv = c;
                        best = Some((ev, c, r));
                    }
                    r + target_radius
                })
                .unwrap_or(f64::INFINITY);
        } else {
            lo = c;
            c = d;
            fc = fd;
            d = lo + inv_phi * (hi - lo);
            fd = evaluate_dv(d)
                .map(|(ev, r)| {
                    if r + target_radius < best_distance {
                        best_distance = r + target_radius;
                        best_dv = d;
                        best = Some((ev, d, r));
                    }
                    r + target_radius
                })
                .unwrap_or(f64::INFINITY);
        }
        if (hi - lo).abs() < 0.02 {
            break;
        }
    }
    let dv_min = best_dv;
    let min_distance = best_distance;

    // If even the best encounter stays outside the target radius there is no
    // solution to report.
    if min_distance > target_radius + 10_000.0 {
        let residual = min_distance - target_radius;
        return Err(PlanError::ResidualTooLarge((residual / 100.0).round() as i64));
    }

    // Bracket the target altitude on the inbound side of the minimum.
    step = (dv0 * 0.005).max(1.0);
    let mut lower = (dv_min - step).max(0.0);
    let mut f_lower = match evaluate_dv(lower) {
        Ok((_, r)) => r,
        Err(_) => {
            lower = dv_min * 0.95;
            match evaluate_dv(lower) {
                Ok((_, r)) => r,
                Err(e) => return Err(e),
            }
        }
    };
    for _ in 0..24 {
        if f_lower > 0.0 {
            break;
        }
        lower = (lower - step).max(0.0);
        f_lower = evaluate_dv(lower).map(|(_, r)| r).unwrap_or(-1.0);
    }
    if f_lower <= 0.0 {
        return Err(PlanError::NoBracket);
    }
    let mut upper = dv_min;
    let mut f_upper = evaluate_dv(upper)
        .map(|(ev, r)| {
            if r.abs() < best.as_ref().map(|(_, _, br)| br.abs()).unwrap_or(f64::INFINITY) {
                best = Some((ev, upper, r));
            }
            r
        })
        .map_err(|e| e)?;
    if f_upper > 0.0 {
        // The refined minimum did not cross the target; use the coarse best.
        if let Some((_, dv, r)) = &best {
            if *r < 0.0 {
                upper = *dv;
                f_upper = *r;
            } else {
                return Err(PlanError::NoBracket);
            }
        }
    }
    for _ in 0..48 {
        let mid = 0.5 * (lower + upper);
        match evaluate_dv(mid) {
            Ok((ev, residual)) => {
                if best
                    .as_ref()
                    .map_or(true, |(_, _, br)| residual.abs() < br.abs())
                {
                    best = Some((ev, mid, residual));
                }
                if residual.abs() <= 50.0 {
                    break;
                }
                if residual > 0.0 {
                    lower = mid;
                    f_lower = residual;
                } else {
                    upper = mid;
                    f_upper = residual;
                }
            }
            Err(_) => break,
        }
    }
    let _ = (f_lower, f_upper);
    if let Some((ev, dv, residual)) = best {
        if residual.abs() <= 0.01 * target_perilune_altitude.max(1000.0) {
            return Ok(TliPlan {
                coast_duration: wait,
                burn_time,
                burn_state,
                burn_vehicle: *vehicle,
                direction: prograde(burn_state),
                burn_delta_v: dv,
                closest: ev.closest,
                evaluations,
                residual,
            });
        }
        return Err(PlanError::ResidualTooLarge(
            (residual / 100.0).round() as i64,
        ));
    }
    Err(PlanError::NoBracket)
}

/// Total transfer time used by the phase model.
pub fn nominal_tof(parking_radius: f64) -> f64 {
    std::f64::consts::PI
        * ((0.5 * (parking_radius + MOON_ORBIT_RADIUS)).powi(3) / MU_EARTH).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_core::constants::R_EARTH;
    use orbit_core::rocket::presets;

    fn circular(alt: f64) -> State {
        let r = R_EARTH + alt;
        let v = (MU_EARTH / r).sqrt();
        State::new(Vec2::new(r, 0.0), Vec2::new(0.0, v))
    }

    #[test]
    fn phase_wait_and_tof_are_finite() {
        let eph = Ephemeris::default();
        let s = circular(500_000.0);
        let (wait, tof) = phase_wait(&eph, s, 0.0).unwrap();
        assert!(wait >= 0.0 && wait < 100_000.0);
        assert!((tof / 86_400.0 - 5.0).abs() < 1.0);
        assert!((nominal_tof(s.radius()) - tof).abs() < 1.0);
    }

    #[test]
    fn hohmann_dv_is_plausible() {
        let dv = hohmann_delta_v(circular(500_000.0));
        assert!(dv > 2800.0 && dv < 3600.0, "dv={dv}");
    }

    #[test]
    fn planners_reject_non_circular_input() {
        let eph = Ephemeris::default();
        let rocket = presets::atlas();
        let v = Vehicle::new(0, rocket.stages[0].propellant_mass(), rocket.liftoff_mass());
        let r = R_EARTH + 500_000.0;
        let s = State::new(Vec2::new(r, 0.0), Vec2::new(0.0, (MU_EARTH / r).sqrt() * 1.5));
        assert!(plan_tli(&eph, &rocket, &v, 0.0, s, 100_000.0).is_err());
    }
}

/// Diagnostic scan of perilune altitude versus TLI burn magnitude.
///
/// This is used by the Studio and tests to show the shape of the targeting
/// problem. It is not a planner and never claims convergence.
pub fn scan_tli(
    eph: &Ephemeris,
    rocket: &Rocket,
    vehicle: &Vehicle,
    t0: f64,
    state: State,
    factors: &[f64],
) -> Vec<(f64, Result<f64, PlanError>)> {
    let (wait, tof) = match phase_wait(eph, state, t0) {
        Some(v) => v,
        None => return Vec::new(),
    };
    let burn_time = t0 + wait;
    let burn_state = match coast_parking(eph, state, t0, wait) {
        Ok(s) => s,
        Err(e) => return vec![(0.0, Err(e))],
    };
    let dv0 = hohmann_delta_v(burn_state);
    factors
        .iter()
        .map(|f| {
            let dv = dv0 * f;
            match evaluate(eph, rocket, vehicle, burn_time, burn_state, dv, tof) {
                Ok(ev) => (dv, Ok(ev.closest.altitude)),
                Err(e) => (dv, Err(e)),
            }
        })
        .collect()
}
