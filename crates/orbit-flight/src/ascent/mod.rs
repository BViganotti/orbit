//! Powered ascent in a rotating two-dimensional Earth frame.
//!
//! The vehicle starts on the equator at longitude zero with the Earth's
//! rotational velocity. Gravity is the full Earth-Moon-Sun field, the atmosphere
//! co-rotates with Earth, and stages separate when their usable propellant is
//! exhausted. Guidance is the deterministic altitude/apoapsis program in
//! [`guidance`].

pub mod guidance;

use orbit_core::aero::{density, dynamic_pressure, pressure_ratio};
use orbit_core::constants::{MU_EARTH, OMEGA_EARTH, R_EARTH};
use orbit_core::math::{rk4_step, Vec2};
use orbit_core::rocket::Rocket;
use orbit_core::G0;

use crate::nbody::gravity::{acceleration, elements, Elements};
use crate::nbody::{Ephemeris, State};

use guidance::{pitch_program, throttle_for_q, thrust_direction};

/// Ascent configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AscentConfig {
    /// Desired parking-orbit altitude, m.
    pub target_altitude: f64,
    /// Fixed integration step, s.
    pub dt: f64,
    /// Wall-clock simulated limit, s.
    pub max_time: f64,
    /// Maximum dynamic pressure before throttling, Pa.
    pub max_q: f64,
    /// Pitch-program exponent (lower pitches over earlier).
    pub pitch_exponent: f64,
    /// Apoapsis correction gain in degrees.
    pub apoapsis_gain_deg: f64,
    /// Sample cadence for plots, s.
    pub sample_interval: f64,
    /// Launch longitude, radians.
    pub launch_longitude: f64,
}

impl Default for AscentConfig {
    fn default() -> Self {
        Self {
            target_altitude: 200_000.0,
            dt: 0.2,
            max_time: 1500.0,
            max_q: 60_000.0,
            pitch_exponent: 0.75,
            apoapsis_gain_deg: 0.0,
            sample_interval: 5.0,
            launch_longitude: 0.0,
        }
    }
}

impl AscentConfig {
    pub fn validate(&self) -> Result<(), String> {
        let values = [
            self.target_altitude,
            self.dt,
            self.max_time,
            self.max_q,
            self.pitch_exponent,
            self.apoapsis_gain_deg,
            self.sample_interval,
            self.launch_longitude,
        ];
        if values.iter().any(|v| !v.is_finite()) {
            return Err("ascent configuration contains a non-finite value".into());
        }
        if !(80_000.0..=2_000_000.0).contains(&self.target_altitude) {
            return Err("target altitude must be in [80 km, 2000 km]".into());
        }
        if !(0.01..=2.0).contains(&self.dt) {
            return Err("ascent step must be in [0.01, 2] s".into());
        }
        if !(60.0..=7200.0).contains(&self.max_time) {
            return Err("ascent time limit must be in [60, 7200] s".into());
        }
        if !(1_000.0..=200_000.0).contains(&self.max_q) {
            return Err("dynamic pressure limit must be in [1, 200] kPa".into());
        }
        if !(0.1..=1.5).contains(&self.pitch_exponent) {
            return Err("pitch exponent must be in [0.1, 1.5]".into());
        }
        if !(0.0..=45.0).contains(&self.apoapsis_gain_deg) {
            return Err("apoapsis gain must be in [0, 45] degrees".into());
        }
        if !(0.5..=60.0).contains(&self.sample_interval) {
            return Err("sample interval must be in [0.5, 60] s".into());
        }
        Ok(())
    }
}

/// Event kinds recorded during ascent.
#[derive(Debug, Clone, PartialEq)]
pub enum AscentEventKind {
    Liftoff,
    MaxQ { q: f64 },
    StageSeparation { stage: usize },
    EngineCutoff,
    Apogee,
    Impact,
    Timeout,
}

/// A timestamped ascent event.
#[derive(Debug, Clone, PartialEq)]
pub struct AscentEvent {
    pub t: f64,
    pub altitude: f64,
    pub speed: f64,
    pub kind: AscentEventKind,
}

/// One ascent sample for plotting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AscentSample {
    pub t: f64,
    pub altitude: f64,
    pub speed: f64,
    pub mass: f64,
    pub dynamic_pressure: f64,
    pub stage: u8,
    pub pitch: f64,
    pub horizontal_speed: f64,
    pub vertical_speed: f64,
    pub position: Vec2,
}

/// How the powered phase ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AscentStatus {
    OrbitInsertion,
    StageDepleted,
    Impact,
    Timeout,
    Aborted,
}

/// Complete ascent result.
#[derive(Debug, Clone, PartialEq)]
pub struct AscentResult {
    pub state: State,
    pub t: f64,
    pub mass: f64,
    pub stage_index: usize,
    pub propellant_remaining: f64,
    pub orbit: Option<Elements>,
    pub status: AscentStatus,
    pub success: bool,
    pub events: Vec<AscentEvent>,
    pub samples: Vec<AscentSample>,
    pub max_q: f64,
    pub max_q_time: f64,
    pub max_g: f64,
    pub max_altitude: f64,
    pub delta_v: f64,
}

impl AscentResult {
    /// Remaining ideal delta-v of the vehicle after ascent.
    pub fn remaining_delta_v(&self, rocket: &Rocket) -> f64 {
        if self.stage_index >= rocket.stages.len() {
            return 0.0;
        }
        let stage = &rocket.stages[self.stage_index];
        let reserve = stage.propellant_mass() * stage.reserve_fraction;
        let usable = (self.propellant_remaining - reserve).max(0.0);
        let mut dv = if self.mass > usable && usable > 0.0 {
            stage.engine.isp_vacuum * G0 * (self.mass / (self.mass - usable)).ln()
        } else {
            0.0
        };
        for i in (self.stage_index + 1)..rocket.stages.len() {
            dv += rocket.stage_delta_v(i);
        }
        dv
    }

    pub fn periapsis_altitude(&self) -> Option<f64> {
        self.orbit
            .map(|e| e.periapsis - orbit_core::constants::R_EARTH)
    }

    pub fn apoapsis_altitude(&self) -> Option<f64> {
        self.orbit
            .map(|e| e.apoapsis - orbit_core::constants::R_EARTH)
    }
}

struct Dynamics<'a> {
    eph: &'a Ephemeris,
    rocket: &'a Rocket,
    stage_index: usize,
    base_mass: f64,
    pitch: f64,
    throttle: f64,
    prop_floor: f64,
}

impl Dynamics<'_> {
    fn mass(&self, prop: f64) -> f64 {
        (self.base_mass + prop).max(1.0)
    }

    fn acceleration(&self, t: f64, position: Vec2, velocity: Vec2, prop: f64) -> Vec2 {
        let mut acc = acceleration(self.eph, t, position);
        let radius = position.norm();
        let altitude = radius - R_EARTH;
        let mass = self.mass(prop);
        let stage = &self.rocket.stages[self.stage_index];
        if prop > self.prop_floor && self.throttle > 0.0 {
            let pr = pressure_ratio(altitude);
            let thrust = stage.engine.thrust_at(pr) * f64::from(stage.engine_count) * self.throttle;
            let direction = thrust_direction(position, self.pitch);
            acc += direction * (thrust / mass);
        }
        let v_atm = Vec2::new(-OMEGA_EARTH * position.y, OMEGA_EARTH * position.x);
        let v_rel = velocity - v_atm;
        let rho = density(altitude);
        let speed = v_rel.norm();
        if speed > 1.0e-9 && rho > 0.0 {
            let q = 0.5 * rho * speed * speed;
            let drag = q * self.rocket.drag_coefficient * self.rocket.drag_area;
            acc -= v_rel * (drag / (mass * speed));
        }
        if acc.is_finite() {
            acc
        } else {
            Vec2::ZERO
        }
    }
}

fn derivative(dyn_: &Dynamics<'_>, t: f64, y: &[f64], out: &mut [f64]) {
    let position = Vec2::new(y[0], y[1]);
    let velocity = Vec2::new(y[2], y[3]);
    let prop = y[4];
    let acc = dyn_.acceleration(t, position, velocity, prop);
    out[0] = velocity.x;
    out[1] = velocity.y;
    out[2] = acc.x;
    out[3] = acc.y;
    if prop > dyn_.prop_floor && dyn_.throttle > 0.0 {
        let altitude = position.norm() - R_EARTH;
        let stage = &dyn_.rocket.stages[dyn_.stage_index];
        let pr = pressure_ratio(altitude);
        let thrust = stage.engine.thrust_at(pr) * f64::from(stage.engine_count) * dyn_.throttle;
        let isp = stage.engine.isp_at(pr).max(1.0);
        out[4] = -(thrust / (isp * G0)).max(0.0);
    } else {
        out[4] = 0.0;
    }
}

fn make_event(t: f64, state: State, kind: AscentEventKind) -> AscentEvent {
    AscentEvent {
        t,
        altitude: state.altitude(),
        speed: state.speed(),
        kind,
    }
}

/// Run the powered ascent.
pub fn run_ascent(
    rocket: &Rocket,
    eph: &Ephemeris,
    t0: f64,
    config: &AscentConfig,
) -> Result<AscentResult, String> {
    rocket.validate()?;
    eph.validate()?;
    config.validate()?;
    if rocket.stages.is_empty() {
        return Err("rocket has no stages".into());
    }
    let radius = R_EARTH;
    let angle = config.launch_longitude;
    let position = Vec2::from_polar(radius, angle);
    let v_atm = Vec2::new(-OMEGA_EARTH * position.y, OMEGA_EARTH * position.x);
    let state = State::new(position, v_atm);

    let mut stage_index = 0usize;
    let stage = &rocket.stages[0];
    let mut base_mass = rocket.mass_above(0) + stage.dry_mass();
    let mut prop = stage.propellant_mass();
    let mut y = [state.position.x, state.position.y, state.velocity.x, state.velocity.y, prop];
    let mut t = t0;
    let mut events = vec![make_event(t0, state, AscentEventKind::Liftoff)];
    let mut samples = vec![AscentSample {
        t: 0.0,
        altitude: 0.0,
        speed: state.speed(),
        mass: base_mass + prop,
        dynamic_pressure: 0.0,
        stage: 0,
        pitch: std::f64::consts::FRAC_PI_2,
        horizontal_speed: 0.0,
        vertical_speed: 0.0,
        position,
    }];
    let mut max_q = 0.0;
    let mut max_q_time = 0.0;
    let mut max_g = 0.0f64;
    let mut max_altitude = 0.0;
    let mut delta_v = 0.0;
    let mut last_sample = t0;
    let mut status = AscentStatus::Timeout;
    let mut max_q_event_recorded = false;

    let max_iterations = (config.max_time / config.dt).ceil() as u64 + 10;
    for _ in 0..max_iterations {
        let position = Vec2::new(y[0], y[1]);
        let velocity = Vec2::new(y[2], y[3]);
        let current = State::new(position, velocity);
        if !current.is_finite() || !y[4].is_finite() {
            status = AscentStatus::Aborted;
            break;
        }
        let altitude = current.altitude();
        if altitude > max_altitude {
            max_altitude = altitude;
        }
        if altitude <= 0.0 && t > t0 + 1.0 {
            status = AscentStatus::Impact;
            events.push(make_event(t - t0, current, AscentEventKind::Impact));
            break;
        }
        let elapsed = t - t0;
        let stage = &rocket.stages[stage_index];
        let prop_floor = stage.propellant_mass() * stage.reserve_fraction;
        // End the powered phase when the horizontal speed reaches local
        // circular speed with little radial motion. The vehicle is then in a
        // near-circular parking orbit and can coast or transfer.
        if elapsed > 30.0 {
            let radius_now = position.norm();
            let up = position.normalized_or_x();
            let east = up.perp();
            let vt = velocity.dot(east);
            let vr = velocity.dot(up);
            let v_circ = (MU_EARTH / radius_now).sqrt();
            if altitude >= config.target_altitude - 120_000.0
                && vt >= v_circ * 0.995
                && vr.abs() < 1200.0
            {
                status = AscentStatus::OrbitInsertion;
                events.push(make_event(elapsed, current, AscentEventKind::EngineCutoff));
                break;
            }
        }
        if y[4] <= prop_floor + 1e-9 {
            // Stage is spent.
            if stage_index + 1 < rocket.stages.len() {
                events.push(make_event(
                    elapsed,
                    current,
                    AscentEventKind::StageSeparation {
                        stage: stage_index + 1,
                    },
                ));
                stage_index += 1;
                let next = &rocket.stages[stage_index];
                base_mass = rocket.mass_above(stage_index) + next.dry_mass();
                prop = next.propellant_mass();
                y[4] = prop;
                let up = current.position.normalized_or_x();
                let east = up.perp();
                samples.push(AscentSample {
                    t: elapsed,
                    altitude,
                    speed: current.speed(),
                    mass: base_mass + prop,
                    dynamic_pressure: dynamic_pressure(altitude, current.speed()),
                    stage: stage_index as u8,
                    pitch: 0.0,
                    horizontal_speed: current.velocity.dot(east),
                    vertical_speed: current.velocity.dot(up),
                    position: current.position,
                });
                last_sample = t;
                continue;
            } else {
                status = AscentStatus::StageDepleted;
                events.push(make_event(elapsed, current, AscentEventKind::EngineCutoff));
                break;
            }
        }

        // Guidance for this step.
        let pr = pressure_ratio(altitude);
        let q = dynamic_pressure(altitude, {
            let v_atm = Vec2::new(-OMEGA_EARTH * position.y, OMEGA_EARTH * position.x);
            (velocity - v_atm).norm()
        });
        if q > max_q {
            max_q = q;
            max_q_time = elapsed;
        }
        if q > config.max_q * 0.5 && !max_q_event_recorded {
            // Record a single max-Q marker when the vehicle actually passes the
            // configured corridor.
            if q > config.max_q {
                max_q_event_recorded = true;
                events.push(make_event(
                    elapsed,
                    current,
                    AscentEventKind::MaxQ { q },
                ));
            }
        }
        let throttle = throttle_for_q(q, config.max_q);
        let thrust = stage.engine.thrust_at(pr) * f64::from(stage.engine_count) * throttle;
        let mass = base_mass + y[4];
        let pitch = pitch_program(
            altitude,
            config.target_altitude,
            config.pitch_exponent,
        )
        .clamp(0.0, std::f64::consts::FRAC_PI_2);
        max_g = max_g.max(thrust / (mass * G0));
        delta_v += (thrust / mass) * config.dt;

        let dyn_ = Dynamics {
            eph,
            rocket,
            stage_index,
            base_mass,
            pitch,
            throttle,
            prop_floor,
        };
        let step = config.dt;
        let mut scratch = [0.0; 5];
        if rk4_step(
            &|tt: f64, yy: &[f64], out: &mut [f64]| derivative(&dyn_, tt, yy, out),
            t,
            &mut y,
            step,
            &mut scratch,
        )
        .is_err()
        {
            status = AscentStatus::Aborted;
            break;
        }
        t += step;
        // Clamp propellant to its floor and enforce mass consistency.
        let new_floor = rocket.stages[stage_index].propellant_mass()
            * rocket.stages[stage_index].reserve_fraction;
        if y[4] < new_floor {
            y[4] = new_floor;
        }
        if y[4] > rocket.stages[stage_index].propellant_mass() {
            y[4] = rocket.stages[stage_index].propellant_mass();
        }
        delta_v = delta_v.max(0.0);

        if t - last_sample >= config.sample_interval || events.len() > 1 && t - last_sample > 0.5 {
            let st = State::from_arrays(&y).unwrap_or(current);
            let v_atm = Vec2::new(-OMEGA_EARTH * st.position.y, OMEGA_EARTH * st.position.x);
            let up = st.position.normalized_or_x();
            let east = up.perp();
            samples.push(AscentSample {
                t: t - t0,
                altitude: st.altitude(),
                speed: st.speed(),
                mass: base_mass + y[4],
                dynamic_pressure: dynamic_pressure(st.altitude(), (st.velocity - v_atm).norm()),
                stage: stage_index as u8,
                pitch,
                horizontal_speed: st.velocity.dot(east),
                vertical_speed: st.velocity.dot(up),
                position: st.position,
            });
            last_sample = t;
            if samples.len() > 100_000 {
                return Err("ascent produced too many samples".into());
            }
        }
        if t - t0 >= config.max_time {
            status = AscentStatus::Timeout;
            events.push(make_event(t - t0, current, AscentEventKind::Timeout));
            break;
        }
    }

    let final_state = State::from_arrays(&y).ok_or("ascent produced a non-finite state")?;
    if !final_state.is_finite() {
        return Err("ascent produced a non-finite state".into());
    }
    let final_orbit = elements(final_state.position, final_state.velocity, MU_EARTH);
    let success = match final_orbit {
        Some(el) => {
            el.apoapsis > R_EARTH + config.target_altitude - 5_000.0
                && final_state.altitude() > 30_000.0
                && y[4] > rocket.stages[stage_index].propellant_mass()
                    * rocket.stages[stage_index].reserve_fraction
        }
        None => false,
    };
    let mass = base_mass + y[4];
    Ok(AscentResult {
        state: final_state,
        t: t - t0,
        mass,
        stage_index,
        propellant_remaining: y[4],
        orbit: final_orbit,
        status,
        success,
        events,
        samples,
        max_q,
        max_q_time,
        max_g,
        max_altitude,
        delta_v,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_core::rocket::presets;
    use orbit_core::rocket::Rocket;

    #[test]
    fn ascent_config_rejects_bad_values() {
        let mut c = AscentConfig::default();
        assert!(c.validate().is_ok());
        c.dt = 0.0;
        assert!(c.validate().is_err());
        let mut c = AscentConfig::default();
        c.target_altitude = 1000.0;
        assert!(c.validate().is_err());
        let mut c = AscentConfig::default();
        c.max_time = f64::NAN;
        assert!(c.validate().is_err());
    }

    #[test]
    fn small_rocket_reaches_parking_orbit() {
        let rocket = presets::atlas();
        let eph = Ephemeris::default();
        let mut config = AscentConfig::default();
        config.target_altitude = 400_000.0;
        let result = run_ascent(&rocket, &eph, 0.0, &config).unwrap();
        assert!(
            result.success,
            "status={:?} peri={:?} apo={:?} events={:?}",
            result.status,
            result.periapsis_altitude(),
            result.apoapsis_altitude(),
            result.events
        );
        assert!(result.max_q > 1000.0);
        assert!(result.max_g > 1.0);
        assert!(result.delta_v > 5000.0);
        assert!(!result.events.is_empty());
    }

    #[test]
    fn underpowered_rocket_does_not_claim_success() {
        let mut rocket = presets::kestrel();
        // Cripple the vehicle.
        rocket.stages[0].engine_count = 1;
        rocket.stages[0].engine.thrust_vacuum = 100_000.0;
        rocket.stages[0].engine.thrust_sea_level = 80_000.0;
        let eph = Ephemeris::default();
        let mut config = AscentConfig::default();
        config.target_altitude = 400_000.0;
        let result = run_ascent(&rocket, &eph, 0.0, &config).unwrap();
        assert!(!result.success);
    }

    #[test]
    fn remaining_delta_v_is_bounded() {
        let rocket = presets::atlas();
        let eph = Ephemeris::default();
        let mut config = AscentConfig::default();
        config.target_altitude = 400_000.0;
        let result = run_ascent(&rocket, &eph, 0.0, &config).unwrap();
        let dv = result.remaining_delta_v(&rocket);
        assert!(dv >= 0.0 && dv < 20_000.0);
    }

    #[test]
    fn ascent_is_deterministic() {
        let rocket = Rocket::clone(&presets::sparrow());
        let eph = Ephemeris::default();
        let mut config = AscentConfig::default();
        config.target_altitude = 400_000.0;
        let a = run_ascent(&rocket, &eph, 0.0, &config).unwrap();
        let b = run_ascent(&rocket, &eph, 0.0, &config).unwrap();
        assert_eq!(a.state, b.state);
        assert_eq!(a.t, b.t);
        assert_eq!(a.samples.len(), b.samples.len());
    }
}
