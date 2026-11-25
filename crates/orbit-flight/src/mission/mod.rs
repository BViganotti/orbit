//! Deterministic mission orchestration.
//!
//! One mission is a four-phase pipeline: ascent, parking-orbit circularisation,
//! trans-lunar injection and lunar capture. Every phase has a bounded time and
//! state budget. Failure at any phase is reported with the residual that caused
//! it; the runner never fabricates a successful orbit.

pub mod scripts;

use orbit_core::constants::R_EARTH;
use orbit_core::rocket::{Rocket, RocketSummary};

use crate::ascent::{run_ascent, AscentConfig, AscentResult};
use crate::nbody::gravity::elements;
use crate::nbody::propagator::{coast, CoastConfig};
use crate::nbody::{Body, Ephemeris};
use crate::plan::{
    circularize, finite_burn, insert_lunar_orbit, plan_tli, BurnConfig, BurnStatus, LunarOrbit,
    TliPlan, Vehicle,
};

/// Mission configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct MissionConfig {
    pub name: String,
    pub epoch: f64,
    pub moon_phase: f64,
    pub sun_phase: f64,
    /// Requested lunar circular-orbit altitude, m.
    pub lunar_altitude: f64,
    /// Acceptance tolerance as a fraction of the lunar altitude.
    pub tolerance: f64,
    pub ascent: AscentConfig,
    pub burn: BurnConfig,
}

impl Default for MissionConfig {
    fn default() -> Self {
        Self {
            name: "Lunar demo".into(),
            epoch: 0.0,
            moon_phase: 0.0,
            sun_phase: std::f64::consts::PI,
            lunar_altitude: 100_000.0,
            tolerance: 0.01,
            ascent: AscentConfig {
                target_altitude: 400_000.0,
                ..AscentConfig::default()
            },
            burn: BurnConfig::default(),
        }
    }
}

impl MissionConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() || self.name.len() > 80 {
            return Err("mission name must be 1..=80 bytes".into());
        }
        if !self.epoch.is_finite() {
            return Err("epoch is not finite".into());
        }
        if !self.moon_phase.is_finite() || !self.sun_phase.is_finite() {
            return Err("ephemeris phase is not finite".into());
        }
        if !(10_000.0..=1_000_000.0).contains(&self.lunar_altitude) {
            return Err("lunar altitude must be in [10 km, 1000 km]".into());
        }
        if !self.tolerance.is_finite() || !(0.001..=0.2).contains(&self.tolerance) {
            return Err("tolerance must be in [0.1%, 20%]".into());
        }
        self.ascent.validate()?;
        self.burn.validate()?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Ascent,
    Parking,
    Transfer,
    Lunar,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Ascent => "ascent",
            Phase::Parking => "parking",
            Phase::Transfer => "transfer",
            Phase::Lunar => "lunar",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MissionSample {
    pub t: f64,
    pub altitude: f64,
    pub speed: f64,
    pub moon_distance: f64,
    pub phase: Phase,
    pub mass: f64,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventKind {
    Liftoff,
    StageSeparation,
    ParkingOrbit,
    TliIgnition,
    TliCutoff,
    LunarSoi,
    Perilune,
    LoiIgnition,
    LunarOrbit,
    Failure,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MissionEvent {
    pub t: f64,
    pub phase: Phase,
    pub kind: EventKind,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionStatus {
    Success,
    AscentFailed,
    ParkingFailed,
    TransferFailed,
    CaptureFailed,
    OrbitMiss,
    Invalid,
}

impl MissionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            MissionStatus::Success => "success",
            MissionStatus::AscentFailed => "ascent-failed",
            MissionStatus::ParkingFailed => "parking-failed",
            MissionStatus::TransferFailed => "transfer-failed",
            MissionStatus::CaptureFailed => "capture-failed",
            MissionStatus::OrbitMiss => "orbit-miss",
            MissionStatus::Invalid => "invalid",
        }
    }
}

/// Summary of the solved transfer, kept small for reports and the Studio.
#[derive(Debug, Clone, PartialEq)]
pub struct TransferSummary {
    pub coast_seconds: f64,
    pub burn_delta_v: f64,
    pub burn_seconds: f64,
    pub perilune_altitude: f64,
    pub residual: f64,
    pub evaluations: u32,
    pub coast_steps: u32,
}

/// Complete mission result.
#[derive(Debug, Clone, PartialEq)]
pub struct MissionResult {
    pub status: MissionStatus,
    pub success: bool,
    pub message: String,
    pub rocket_name: String,
    pub summary: Option<RocketSummary>,
    pub ascent: Option<AscentResult>,
    pub transfer: Option<TransferSummary>,
    pub lunar: Option<LunarOrbit>,
    pub events: Vec<MissionEvent>,
    pub samples: Vec<MissionSample>,
    pub total_time: f64,
    pub delta_v_ascent: f64,
    pub delta_v_parking: f64,
    pub delta_v_transfer: f64,
    pub delta_v_capture: f64,
    pub propellant_remaining: f64,
    pub final_mass: f64,
    pub target_altitude: f64,
    pub periapsis_error: Option<f64>,
    pub apoapsis_error: Option<f64>,
}

impl MissionResult {
    fn failed(
        status: MissionStatus,
        message: impl Into<String>,
        rocket: &Rocket,
        config: &MissionConfig,
        events: Vec<MissionEvent>,
        samples: Vec<MissionSample>,
    ) -> Self {
        Self {
            status,
            success: false,
            message: message.into(),
            rocket_name: rocket.name.clone(),
            summary: Some(rocket.summary()),
            ascent: None,
            transfer: None,
            lunar: None,
            events,
            samples,
            total_time: 0.0,
            delta_v_ascent: 0.0,
            delta_v_parking: 0.0,
            delta_v_transfer: 0.0,
            delta_v_capture: 0.0,
            propellant_remaining: 0.0,
            final_mass: 0.0,
            target_altitude: config.lunar_altitude,
            periapsis_error: None,
            apoapsis_error: None,
        }
    }
}

fn ascent_samples_into(
    out: &mut Vec<MissionSample>,
    ascent: &AscentResult,
    epoch: f64,
    eph: &Ephemeris,
) {
    for s in &ascent.samples {
        let t = epoch + s.t;
        let moon = eph.position(Body::Moon, t);
        out.push(MissionSample {
            t,
            altitude: s.altitude,
            speed: s.speed,
            moon_distance: (s.position - moon).norm(),
            phase: Phase::Ascent,
            mass: s.mass,
            x: s.position.x,
            y: s.position.y,
        });
    }
}

fn push_coast_samples(
    out: &mut Vec<MissionSample>,
    samples: &[crate::nbody::Sample],
    phase: Phase,
    mass: f64,
) {
    for s in samples {
        out.push(MissionSample {
            t: s.t,
            altitude: s.altitude,
            speed: s.speed,
            moon_distance: s.moon_distance,
            phase,
            mass,
            x: s.position.x,
            y: s.position.y,
        });
    }
}

/// Run a complete lunar mission.
pub fn run_mission(rocket: &Rocket, config: &MissionConfig) -> Result<MissionResult, String> {
    rocket.validate()?;
    config.validate()?;
    let eph = Ephemeris::new(config.moon_phase, config.sun_phase);
    eph.validate()?;
    let epoch = config.epoch;
    let mut events: Vec<MissionEvent> = Vec::new();
    let mut samples: Vec<MissionSample> = Vec::new();

    let ascent = match run_ascent(rocket, &eph, epoch, &config.ascent) {
        Ok(a) => a,
        Err(e) => {
            return Ok(MissionResult::failed(
                MissionStatus::AscentFailed,
                e,
                rocket,
                config,
                events,
                samples,
            ))
        }
    };
    ascent_samples_into(&mut samples, &ascent, epoch, &eph);
    events.push(MissionEvent {
        t: epoch,
        phase: Phase::Ascent,
        kind: EventKind::Liftoff,
        detail: format!("{} leaving the pad", rocket.name),
    });
    for ev in &ascent.events {
        if matches!(
            ev.kind,
            crate::ascent::AscentEventKind::StageSeparation { .. }
        ) {
            events.push(MissionEvent {
                t: epoch + ev.t,
                phase: Phase::Ascent,
                kind: EventKind::StageSeparation,
                detail: format!("stage separation at {:.0} km", ev.altitude / 1000.0),
            });
        }
    }
    if !ascent.success {
        events.push(MissionEvent {
            t: epoch + ascent.t,
            phase: Phase::Ascent,
            kind: EventKind::Failure,
            detail: format!("ascent ended with {:?}", ascent.status),
        });
        let mut result = MissionResult::failed(
            MissionStatus::AscentFailed,
            format!("ascent ended with {:?}", ascent.status),
            rocket,
            config,
            events,
            samples,
        );
        result.ascent = Some(ascent);
        return Ok(result);
    }

    let mut vehicle = match Vehicle::from_ascent(
        rocket,
        ascent.stage_index,
        ascent.propellant_remaining,
        ascent.mass,
    ) {
        Some(v) => v,
        None => {
            return Ok(MissionResult::failed(
                MissionStatus::AscentFailed,
                "ascent produced an invalid vehicle state",
                rocket,
                config,
                events,
                samples,
            ))
        }
    };
    let mut t = epoch + ascent.t;
    let mut state = ascent.state;

    // Parking-orbit circularisation.
    let (circ_state, circ_t, circ_spent, circ_status) =
        match circularize(&eph, rocket, &mut vehicle, t, state, &config.burn) {
            Ok(v) => v,
            Err(e) => {
                let mut result = MissionResult::failed(
                    MissionStatus::ParkingFailed,
                    e,
                    rocket,
                    config,
                    events,
                    samples,
                );
                result.ascent = Some(ascent);
                return Ok(result);
            }
        };
    if !matches!(circ_status, BurnStatus::Complete) {
        events.push(MissionEvent {
            t: circ_t,
            phase: Phase::Parking,
            kind: EventKind::Failure,
            detail: format!("parking burn ended with {:?}", circ_status),
        });
        let mut result = MissionResult::failed(
            MissionStatus::ParkingFailed,
            format!("parking burn ended with {:?}", circ_status),
            rocket,
            config,
            events,
            samples,
        );
        result.ascent = Some(ascent);
        return Ok(result);
    }
    state = circ_state;
    t = circ_t;
    let parking_elements = elements(
        state.position,
        state.velocity,
        orbit_core::constants::MU_EARTH,
    );
    let parking_altitude = parking_elements
        .map(|e| e.periapsis - R_EARTH)
        .unwrap_or(0.0);
    events.push(MissionEvent {
        t,
        phase: Phase::Parking,
        kind: EventKind::ParkingOrbit,
        detail: format!(
            "parking orbit at {:.0} km, e={:.4}",
            parking_altitude / 1000.0,
            parking_elements.map(|e| e.eccentricity).unwrap_or(1.0)
        ),
    });
    if parking_elements
        .map(|e| e.eccentricity > 0.05)
        .unwrap_or(true)
    {
        let mut result = MissionResult::failed(
            MissionStatus::ParkingFailed,
            "parking orbit is not circular enough for transfer planning",
            rocket,
            config,
            events,
            samples,
        );
        result.ascent = Some(ascent);
        result.delta_v_parking = circ_spent;
        return Ok(result);
    }

    // Trans-lunar injection planning.
    let plan: TliPlan = match plan_tli(&eph, rocket, &vehicle, t, state, config.lunar_altitude) {
        Ok(p) => p,
        Err(e) => {
            events.push(MissionEvent {
                t,
                phase: Phase::Transfer,
                kind: EventKind::Failure,
                detail: format!("TLI planning failed: {e}"),
            });
            let mut result = MissionResult::failed(
                MissionStatus::TransferFailed,
                format!("TLI planning failed: {e}"),
                rocket,
                config,
                events,
                samples,
            );
            result.ascent = Some(ascent);
            result.delta_v_parking = circ_spent;
            return Ok(result);
        }
    };

    // Execute the planned coast and finite TLI burn.
    let parking_coast = match coast(
        &eph,
        state,
        t,
        plan.burn_time,
        &CoastConfig {
            sample_interval: 600.0,
            max_step: 120.0,
            ..CoastConfig::default()
        },
    ) {
        Ok(c) => c,
        Err(e) => {
            let mut result = MissionResult::failed(
                MissionStatus::TransferFailed,
                format!("parking coast failed: {e}"),
                rocket,
                config,
                events,
                samples,
            );
            result.ascent = Some(ascent);
            return Ok(result);
        }
    };
    push_coast_samples(
        &mut samples,
        &parking_coast.samples,
        Phase::Parking,
        vehicle.mass,
    );
    state = parking_coast.state;
    t = parking_coast.t;

    let tli = match finite_burn(
        &eph,
        rocket,
        &mut vehicle,
        t,
        state,
        plan.direction,
        plan.burn_delta_v,
        &config.burn,
    ) {
        Ok(b) => b,
        Err(e) => {
            return Ok(MissionResult::failed(
                MissionStatus::TransferFailed,
                e,
                rocket,
                config,
                events,
                samples,
            ))
        }
    };
    events.push(MissionEvent {
        t,
        phase: Phase::Transfer,
        kind: EventKind::TliIgnition,
        detail: format!("TLI burn {:.0} m/s", plan.burn_delta_v),
    });
    if !matches!(tli.status, BurnStatus::Complete) {
        events.push(MissionEvent {
            t: tli.t,
            phase: Phase::Transfer,
            kind: EventKind::Failure,
            detail: format!("TLI ended with {:?}", tli.status),
        });
        let mut result = MissionResult::failed(
            MissionStatus::TransferFailed,
            format!("TLI ended with {:?}", tli.status),
            rocket,
            config,
            events,
            samples,
        );
        result.ascent = Some(ascent);
        result.delta_v_parking = circ_spent;
        result.delta_v_transfer = tli.delta_v;
        return Ok(result);
    }
    events.push(MissionEvent {
        t: tli.t,
        phase: Phase::Transfer,
        kind: EventKind::TliCutoff,
        detail: format!(
            "{:.0} m/s delivered over {:.0} s",
            tli.delta_v, tli.duration
        ),
    });
    state = tli.state;
    t = tli.t;

    // Coast to perilune. The planning evaluation already validated the
    // geometry; replaying it keeps the mission and the plot consistent.
    let transfer_coast = match coast(
        &eph,
        state,
        t,
        plan.closest.time,
        &CoastConfig {
            rtol: 1.0e-9,
            atol: 1.0e-3,
            sample_interval: 600.0,
            max_step: 300.0,
            ..CoastConfig::default()
        },
    ) {
        Ok(c) => c,
        Err(e) => {
            let mut result = MissionResult::failed(
                MissionStatus::TransferFailed,
                format!("transfer coast failed: {e}"),
                rocket,
                config,
                events,
                samples,
            );
            result.ascent = Some(ascent);
            result.delta_v_parking = circ_spent;
            result.delta_v_transfer = tli.delta_v;
            return Ok(result);
        }
    };
    push_coast_samples(
        &mut samples,
        &transfer_coast.samples,
        Phase::Transfer,
        vehicle.mass,
    );
    if transfer_coast.moon_soi_time.is_some() {
        events.push(MissionEvent {
            t: transfer_coast.moon_soi_time.unwrap_or(t),
            phase: Phase::Transfer,
            kind: EventKind::LunarSoi,
            detail: "entered the lunar sphere of influence".into(),
        });
    }
    state = transfer_coast.state;
    t = transfer_coast.t;
    events.push(MissionEvent {
        t,
        phase: Phase::Transfer,
        kind: EventKind::Perilune,
        detail: format!(
            "perilune {:.1} km at {:.2} km/s relative",
            plan.closest.altitude / 1000.0,
            plan.closest.relative_speed / 1000.0
        ),
    });

    // Lunar orbit insertion.
    let lunar = match insert_lunar_orbit(
        &eph,
        rocket,
        &mut vehicle,
        t,
        state,
        config.lunar_altitude,
        &config.burn,
    ) {
        Ok(l) => l,
        Err(e) => {
            let mut result = MissionResult::failed(
                MissionStatus::CaptureFailed,
                e,
                rocket,
                config,
                events,
                samples,
            );
            result.ascent = Some(ascent);
            result.delta_v_parking = circ_spent;
            result.delta_v_transfer = tli.delta_v;
            return Ok(result);
        }
    };
    events.push(MissionEvent {
        t: lunar.t,
        phase: Phase::Lunar,
        kind: EventKind::LoiIgnition,
        detail: format!("lunar insertion {:.0} m/s", lunar.spent_delta_v),
    });
    samples.push(MissionSample {
        t: lunar.t,
        altitude: lunar.periapsis_altitude,
        speed: lunar.state.speed(),
        moon_distance: (lunar.state.position - eph.position(Body::Moon, lunar.t)).norm(),
        phase: Phase::Lunar,
        mass: vehicle.mass,
        x: lunar.state.position.x,
        y: lunar.state.position.y,
    });

    let target = config.lunar_altitude;
    let peri_error = lunar.periapsis_altitude - target;
    let apo_error = lunar.apoapsis_altitude - target;
    let tolerance = config.tolerance * target.max(1.0);
    let success = matches!(lunar.status, BurnStatus::Complete)
        && peri_error.abs() <= tolerance
        && apo_error.abs() <= tolerance;
    let status = if success {
        MissionStatus::Success
    } else {
        MissionStatus::OrbitMiss
    };
    events.push(MissionEvent {
        t: lunar.t,
        phase: Phase::Lunar,
        kind: if success {
            EventKind::LunarOrbit
        } else {
            EventKind::Failure
        },
        detail: format!(
            "lunar orbit {:.1} x {:.1} km (target {:.1} km)",
            lunar.periapsis_altitude / 1000.0,
            lunar.apoapsis_altitude / 1000.0,
            target / 1000.0
        ),
    });

    if samples.len() > 20_000 {
        let stride = samples.len() / 10_000 + 1;
        samples = samples.into_iter().step_by(stride).collect();
    }
    let ascent_dv = ascent.delta_v;
    let capture_dv = lunar.spent_delta_v;

    Ok(MissionResult {
        status,
        success,
        message: if success {
            "lunar orbit achieved".into()
        } else {
            format!(
                "lunar orbit missed by {:.1} km periapsis / {:.1} km apoapsis",
                peri_error / 1000.0,
                apo_error / 1000.0
            )
        },
        rocket_name: rocket.name.clone(),
        summary: Some(rocket.summary()),
        ascent: Some(ascent),
        transfer: Some(TransferSummary {
            coast_seconds: plan.coast_duration,
            burn_delta_v: tli.delta_v,
            burn_seconds: tli.duration,
            perilune_altitude: plan.closest.altitude,
            residual: plan.residual,
            evaluations: plan.evaluations,
            coast_steps: transfer_coast.steps,
        }),
        lunar: Some(lunar),
        events,
        samples,
        total_time: (t - epoch).max(0.0),
        delta_v_ascent: ascent_dv,
        delta_v_parking: circ_spent,
        delta_v_transfer: tli.delta_v,
        delta_v_capture: capture_dv,
        propellant_remaining: vehicle.propellant,
        final_mass: vehicle.mass,
        target_altitude: target,
        periapsis_error: Some(peri_error),
        apoapsis_error: Some(apo_error),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_core::rocket::presets;

    #[test]
    fn config_validation() {
        let c = MissionConfig::default();
        assert!(c.validate().is_ok());
        let mut bad = c.clone();
        bad.lunar_altitude = 0.0;
        assert!(bad.validate().is_err());
        let mut bad = c.clone();
        bad.tolerance = 0.5;
        assert!(bad.validate().is_err());
    }

    #[test]
    fn atlas_demo_reaches_lunar_orbit() {
        let rocket = presets::atlas();
        let result = run_mission(&rocket, &MissionConfig::default()).unwrap();
        assert!(result.success, "{}", result.message);
        let lunar = result.lunar.unwrap();
        assert!(lunar.periapsis_altitude > 90_000.0);
        assert!(lunar.apoapsis_altitude < 110_000.0);
        assert!(result.delta_v_transfer > 2500.0);
        assert!(result.delta_v_capture > 500.0);
        assert!(result.samples.len() > 100);
        assert!(result
            .events
            .iter()
            .any(|e| e.kind == EventKind::LunarOrbit));
    }

    #[test]
    fn mission_is_deterministic() {
        let rocket = presets::kestrel();
        let a = run_mission(&rocket, &MissionConfig::default()).unwrap();
        let b = run_mission(&rocket, &MissionConfig::default()).unwrap();
        assert_eq!(a.status, b.status);
        assert_eq!(a.total_time, b.total_time);
        assert_eq!(
            a.lunar.unwrap().elements.eccentricity,
            b.lunar.unwrap().elements.eccentricity
        );
    }

    #[test]
    fn crippled_rocket_fails_honestly() {
        let mut rocket = presets::kestrel();
        rocket.stages[0].engine_count = 1;
        rocket.stages[0].engine.thrust_vacuum = 100_000.0;
        rocket.stages[0].engine.thrust_sea_level = 80_000.0;
        let result = run_mission(&rocket, &MissionConfig::default()).unwrap();
        assert!(!result.success);
        assert_ne!(result.status, MissionStatus::Success);
    }

    #[test]
    fn twenty_scripted_missions_reach_lunar_orbit() {
        for script in &super::scripts::SCRIPTS {
            let rocket = presets::by_id(script.rocket_id).unwrap();
            let config = super::scripts::config(script);
            let result = run_mission(&rocket, &config).unwrap();
            assert!(
                result.success,
                "{}: {} ({:?})",
                script.id, result.message, result.status
            );
            let lunar = result.lunar.expect("lunar orbit");
            let tolerance = 0.01 * config.lunar_altitude;
            assert!(
                (lunar.periapsis_altitude - config.lunar_altitude).abs() <= tolerance,
                "{} periapsis {:.1}",
                script.id,
                lunar.periapsis_altitude
            );
            assert!(
                (lunar.apoapsis_altitude - config.lunar_altitude).abs() <= tolerance,
                "{} apoapsis {:.1}",
                script.id,
                lunar.apoapsis_altitude
            );
        }
    }
}
