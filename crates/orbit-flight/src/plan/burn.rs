//! Finite manoeuvre burns.
//!
//! A burn integrates the vehicle through the gravity field with thrust along a
//! fixed inertial direction. Mass flow and stage depletion are modelled, so the
//! delta-v actually delivered differs slightly from the ideal impulse. Mission
//! planning accounts for that difference by re-solving after each burn.

use orbit_core::aero::pressure_ratio;
use orbit_core::constants::R_EARTH;
use orbit_core::math::Vec2;
use orbit_core::rocket::Rocket;
use orbit_core::G0;

use crate::nbody::gravity::acceleration;
use crate::nbody::{Ephemeris, State};

use super::vehicle::Vehicle;

/// How a burn ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BurnStatus {
    Complete,
    PropellantExhausted,
    Impact,
    StepLimit,
}

/// Result of a finite burn.
#[derive(Debug, Clone, PartialEq)]
pub struct BurnOutcome {
    pub state: State,
    pub t: f64,
    pub delta_v: f64,
    pub duration: f64,
    pub status: BurnStatus,
    pub stages_used: u32,
}

/// Configuration for a finite burn.
#[derive(Debug, Clone, Copy)]
pub struct BurnConfig {
    pub dt: f64,
    pub max_duration: f64,
    pub max_steps: u32,
}

impl Default for BurnConfig {
    fn default() -> Self {
        Self {
            dt: 0.15,
            max_duration: 3600.0,
            max_steps: 200_000,
        }
    }
}

impl BurnConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !self.dt.is_finite()
            || !self.max_duration.is_finite()
            || self.dt <= 0.0
            || self.max_duration <= 0.0
            || self.max_steps == 0
        {
            return Err("invalid burn configuration".into());
        }
        Ok(())
    }
}

struct StageDynamics<'a> {
    eph: &'a Ephemeris,
    direction: Vec2,
    thrust: f64,
    isp: f64,
    base_mass: f64,
}

impl StageDynamics<'_> {
    fn derivative(&self, t: f64, y: &[f64], out: &mut [f64]) {
        let position = Vec2::new(y[0], y[1]);
        let velocity = Vec2::new(y[2], y[3]);
        let prop = y[4];
        let mass = (self.base_mass + prop).max(1.0);
        let mut acc = acceleration(self.eph, t, position);
        if prop > 0.0 {
            acc += self.direction * (self.thrust / mass);
        }
        out[0] = velocity.x;
        out[1] = velocity.y;
        out[2] = acc.x;
        out[3] = acc.y;
        out[4] = if prop > 0.0 {
            -(self.thrust / (self.isp * G0)).max(0.0)
        } else {
            0.0
        };
    }
}

/// Burn the current stage by up to `target_delta_v`, automatically staging when
/// a stage runs dry.
pub fn finite_burn(
    eph: &Ephemeris,
    rocket: &Rocket,
    vehicle: &mut Vehicle,
    t0: f64,
    state: State,
    direction: Vec2,
    target_delta_v: f64,
    config: &BurnConfig,
) -> Result<BurnOutcome, String> {
    eph.validate()?;
    rocket.validate()?;
    vehicle.validate(rocket)?;
    config.validate()?;
    if !state.is_finite() || !t0.is_finite() || !direction.is_finite() {
        return Err("non-finite burn input".into());
    }
    if !target_delta_v.is_finite() || target_delta_v < 0.0 {
        return Err("burn delta-v must be finite and non-negative".into());
    }
    let direction = direction
        .try_normalized()
        .ok_or_else(|| "burn direction is degenerate".to_string())?;
    if target_delta_v <= 1.0e-9 {
        return Ok(BurnOutcome {
            state,
            t: t0,
            delta_v: 0.0,
            duration: 0.0,
            status: BurnStatus::Complete,
            stages_used: 0,
        });
    }

    let mut state = state;
    let mut t = t0;
    let mut achieved = 0.0;
    let mut total_duration = 0.0;
    let mut stages_used = 0u32;
    let mut status = BurnStatus::Complete;
    let mut steps = 0u32;

    while achieved < target_delta_v - 1.0e-6 {
        if !vehicle.has_stage(rocket) {
            status = BurnStatus::PropellantExhausted;
            break;
        }
        let usable = vehicle.usable_propellant(rocket);
        if usable <= 1.0e-9 {
            if vehicle.advance(rocket) {
                stages_used += 1;
                continue;
            } else {
                status = BurnStatus::PropellantExhausted;
                break;
            }
        }
        let stage = &rocket.stages[vehicle.stage_index];
        let altitude = state.altitude().max(0.0);
        let pr = pressure_ratio(altitude);
        let thrust = stage.engine.thrust_at(pr) * f64::from(stage.engine_count);
        let isp = stage.engine.isp_at(pr).max(1.0);
        if thrust <= 0.0 {
            status = BurnStatus::PropellantExhausted;
            break;
        }
        let base_mass = vehicle.mass - vehicle.propellant;
        let dyn_ = StageDynamics {
            eph,
            direction,
            thrust,
            isp,
            base_mass,
        };
        let mut y = [
            state.position.x,
            state.position.y,
            state.velocity.x,
            state.velocity.y,
            vehicle.propellant,
        ];
        let dt = config.dt;
        // The delta-v available in this step and the remaining target.
        let accel = thrust / vehicle.mass.max(1.0);
        let remaining_target = target_delta_v - achieved;
        let remaining_prop_time = usable / (thrust / (isp * G0)).max(1.0e-12);
        let step = dt
            .min(remaining_prop_time)
            .min((remaining_target / accel.max(1.0e-9)).max(1.0e-4))
            .max(1.0e-6);
        // RK4 with a closure borrowing the stage dynamics.
        let mut k1 = [0.0; 5];
        let mut k2 = [0.0; 5];
        let mut k3 = [0.0; 5];
        let mut k4 = [0.0; 5];
        let mut tmp = [0.0; 5];
        dyn_.derivative(t, &y, &mut k1);
        for i in 0..5 {
            tmp[i] = y[i] + 0.5 * step * k1[i];
        }
        dyn_.derivative(t + 0.5 * step, &tmp, &mut k2);
        for i in 0..5 {
            tmp[i] = y[i] + 0.5 * step * k2[i];
        }
        dyn_.derivative(t + 0.5 * step, &tmp, &mut k3);
        for i in 0..5 {
            tmp[i] = y[i] + step * k3[i];
        }
        dyn_.derivative(t + step, &tmp, &mut k4);
        for i in 0..5 {
            let delta = step / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
            y[i] += delta;
            if !y[i].is_finite() {
                return Err("burn produced a non-finite state".into());
            }
        }
        t += step;
        total_duration += step;
        achieved += accel * step;
        // Mass bookkeeping from the integrated propellant.
        let prop_after = y[4].max(0.0);
        let mass_after = base_mass + prop_after;
        if mass_after <= 0.0 {
            return Err("burn consumed more mass than available".into());
        }
        vehicle.propellant = prop_after;
        vehicle.mass = mass_after;
        state = State::from_arrays(&y).ok_or("burn produced an invalid state")?;
        if state.radius() <= R_EARTH {
            status = BurnStatus::Impact;
            break;
        }
        steps += 1;
        if steps >= config.max_steps || total_duration >= config.max_duration {
            status = BurnStatus::StepLimit;
            break;
        }
        // If the stage has reached its reserve, jettison it before continuing.
        if vehicle.usable_propellant(rocket) <= 1.0e-9 && achieved < target_delta_v - 1.0e-6 {
            if vehicle.advance(rocket) {
                stages_used += 1;
            } else {
                status = BurnStatus::PropellantExhausted;
                break;
            }
        }
    }

    Ok(BurnOutcome {
        state,
        t,
        delta_v: achieved,
        duration: total_duration,
        status,
        stages_used,
    })
}

/// Prograde unit vector (counter-clockwise tangential) at a state.
pub fn prograde(state: State) -> Vec2 {
    state.position.normalized_or_x().perp()
}

/// Retrograde unit vector.
pub fn retrograde(state: State) -> Vec2 {
    -prograde(state)
}

/// Circularise an Earth orbit by repeated finite prograde/retrograde burns.
/// Returns the final state and the total delta-v spent.
pub fn circularize(
    eph: &Ephemeris,
    rocket: &Rocket,
    vehicle: &mut Vehicle,
    t0: f64,
    state: State,
    config: &BurnConfig,
) -> Result<(State, f64, f64, BurnStatus), String> {
    let mu = orbit_core::constants::MU_EARTH;
    let mut state = state;
    let mut t = t0;
    let mut spent = 0.0;
    let mut status = BurnStatus::Complete;
    for _ in 0..6 {
        let r = state.radius();
        if !r.is_finite() || r < R_EARTH + 1_000.0 {
            return Err("circularisation state is inside the Earth".into());
        }
        let desired = prograde(state) * (mu / r).sqrt();
        let delta = desired - state.velocity;
        let dv = delta.norm();
        if dv < 0.5 {
            break;
        }
        if !delta.is_finite() {
            return Err("circularisation delta-v is not finite".into());
        }
        let outcome = finite_burn(
            eph,
            rocket,
            vehicle,
            t,
            state,
            delta / dv,
            dv,
            config,
        )?;
        state = outcome.state;
        t = outcome.t;
        spent += outcome.delta_v;
        status = outcome.status;
        if !matches!(status, BurnStatus::Complete) {
            break;
        }
    }
    Ok((state, t, spent, status))
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_core::constants::MU_EARTH;
    use orbit_core::rocket::presets;

    fn parking_state(alt: f64) -> State {
        let r = R_EARTH + alt;
        let v = (MU_EARTH / r).sqrt();
        State::new(Vec2::new(r, 0.0), Vec2::new(0.0, v))
    }

    #[test]
    fn tiny_burn_changes_velocity() {
        let rocket = presets::atlas();
        let eph = Ephemeris::default();
        let state = parking_state(400_000.0);
        let mut v = Vehicle::new(0, rocket.stages[0].propellant_mass(), rocket.liftoff_mass());
        let out = finite_burn(
            &eph,
            &rocket,
            &mut v,
            0.0,
            state,
            prograde(state),
            10.0,
            &BurnConfig::default(),
        )
        .unwrap();
        assert_eq!(out.status, BurnStatus::Complete);
        assert!((out.delta_v - 10.0).abs() < 0.2);
        assert!((out.state.velocity.norm() - state.velocity.norm() - 10.0).abs() < 0.5);
        assert!(v.mass < rocket.liftoff_mass());
    }

    #[test]
    fn circularize_corrects_eccentric_orbit() {
        let rocket = presets::atlas();
        let eph = Ephemeris::default();
        let r = R_EARTH + 400_000.0;
        let mut state = State::new(
            Vec2::new(r, 0.0),
            Vec2::new(0.0, (MU_EARTH / r).sqrt() * 1.02),
        );
        let mut v = Vehicle::new(0, rocket.stages[0].propellant_mass(), rocket.liftoff_mass());
        let (out, spent, _, status) = circularize(
            &eph,
            &rocket,
            &mut v,
            0.0,
            state,
            &BurnConfig::default(),
        )
        .unwrap();
        assert_eq!(status, BurnStatus::Complete);
        assert!(spent > 0.0);
        let el = crate::nbody::elements(out.position, out.velocity, MU_EARTH).unwrap();
        assert!(el.eccentricity < 0.01, "e={}", el.eccentricity);
        state = out;
        assert!(state.radius() > R_EARTH);
    }

    #[test]
    fn burn_rejects_bad_inputs() {
        let rocket = presets::atlas();
        let eph = Ephemeris::default();
        let state = parking_state(400_000.0);
        let mut v = Vehicle::new(0, rocket.stages[0].propellant_mass(), rocket.liftoff_mass());
        assert!(finite_burn(
            &eph,
            &rocket,
            &mut v,
            0.0,
            state,
            Vec2::ZERO,
            10.0,
            &BurnConfig::default()
        )
        .is_err());
        assert!(finite_burn(
            &eph,
            &rocket,
            &mut v,
            0.0,
            state,
            prograde(state),
            f64::NAN,
            &BurnConfig::default()
        )
        .is_err());
    }

    #[test]
    fn staging_is_used_when_the_first_stage_runs_dry() {
        let rocket = presets::sparrow();
        let eph = Ephemeris::default();
        let state = parking_state(400_000.0);
        let stage0 = &rocket.stages[0];
        let reserve = stage0.propellant_mass() * stage0.reserve_fraction;
        let mass = rocket.mass_above(0) + stage0.dry_mass() + reserve;
        let mut v = Vehicle::new(0, reserve, mass);
        let out = finite_burn(
            &eph,
            &rocket,
            &mut v,
            0.0,
            state,
            prograde(state),
            2000.0,
            &BurnConfig::default(),
        )
        .unwrap();
        assert!(out.stages_used >= 1);
        assert!(out.delta_v > 1500.0);
    }
}
