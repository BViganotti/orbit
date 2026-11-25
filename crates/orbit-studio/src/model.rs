//! Conversion between JSON values and domain objects.

use orbit_core::rocket::checks::{inspect, CheckReport};
use orbit_core::rocket::{Engine, Propellant, Rocket, Stage, Tank};
use orbit_flight::mission::{
    run_mission, MissionConfig, MissionEvent, MissionResult, MissionStatus, Phase,
};
use orbit_flight::nbody::system::conservation_30_days;

use crate::json::{self, array, number, object, string, Value};

fn num(value: &Value, key: &str, min: f64, max: f64) -> Result<f64, String> {
    let v = value
        .number(key)
        .ok_or_else(|| format!("missing or invalid number '{key}'"))?;
    if !v.is_finite() || v < min || v > max {
        return Err(format!("'{key}' must be in [{min}, {max}]"));
    }
    Ok(v)
}

fn opt_num(value: &Value, key: &str, default: f64, min: f64, max: f64) -> Result<f64, String> {
    match value.get(key) {
        None => Ok(default),
        Some(_) => num(value, key, min, max),
    }
}

fn text(value: &Value, key: &str, max_len: usize) -> Result<String, String> {
    let s = value
        .string(key)
        .ok_or_else(|| format!("missing or invalid string '{key}'"))?;
    if s.trim().is_empty() || s.len() > max_len {
        return Err(format!("'{key}' must be 1..={max_len} bytes"));
    }
    Ok(s.to_string())
}

fn parse_engine(value: &Value) -> Result<Engine, String> {
    let mut engine = Engine::new(
        text(value, "name", 64)?,
        num(value, "thrust_vacuum", 1.0, 1.0e8)?,
        num(value, "thrust_sea_level", 1.0, 1.0e8)?,
        num(value, "isp_vacuum", 1.0, 2000.0)?,
        num(value, "isp_sea_level", 1.0, 2000.0)?,
        num(value, "mass", 0.0, 1.0e5)?,
    );
    engine.min_throttle = opt_num(value, "min_throttle", 0.4, 0.01, 1.0)?;
    engine.gimbal_deg = opt_num(value, "gimbal_deg", 5.0, 0.0, 30.0)?;
    engine.validate()?;
    Ok(engine)
}

fn parse_tank(value: &Value) -> Result<Tank, String> {
    let prop = value
        .get("propellant")
        .ok_or_else(|| "tank is missing 'propellant'".to_string())?;
    let propellant = Propellant::new(
        text(prop, "name", 64)?,
        num(prop, "density", 1.0, 3000.0)?,
    );
    let tank = Tank::new(
        text(value, "name", 64)?,
        num(value, "propellant_mass", 1.0, 2.0e6)?,
        num(value, "dry_mass", 0.0, 1.0e6)?,
        propellant,
    );
    tank.validate()?;
    Ok(tank)
}

fn parse_stage(value: &Value) -> Result<Stage, String> {
    let engine_value = value
        .get("engine")
        .ok_or_else(|| "stage is missing 'engine'".to_string())?;
    let engine = parse_engine(engine_value)?;
    let count = num(value, "engine_count", 1.0, 64.0)? as u32;
    let mut stage = Stage::new(
        text(value, "name", 64)?,
        num(value, "structure_mass", 0.0, 1.0e6)?,
        engine,
        count,
    );
    stage.reserve_fraction = opt_num(value, "reserve_fraction", 0.0, 0.0, 0.49)?;
    let tanks = value
        .array("tanks")
        .ok_or_else(|| "stage is missing 'tanks'".to_string())?;
    if tanks.is_empty() || tanks.len() > 16 {
        return Err("stage must have 1..=16 tanks".into());
    }
    for tank in tanks {
        stage.tanks.push(parse_tank(tank)?);
    }
    stage.validate()?;
    Ok(stage)
}

/// Parse a rocket object.
pub fn parse_rocket(value: &Value) -> Result<Rocket, String> {
    if value.as_object().is_none() {
        return Err("rocket must be a JSON object".into());
    }
    let mut rocket = Rocket::new(text(value, "name", 80)?, num(value, "payload_mass", 0.0, 1.0e6)?);
    rocket.drag_area = opt_num(value, "drag_area", 12.0, 0.1, 1000.0)?;
    rocket.drag_coefficient = opt_num(value, "drag_coefficient", 0.32, 0.02, 5.0)?;
    let stages = value
        .array("stages")
        .ok_or_else(|| "rocket is missing 'stages'".to_string())?;
    if stages.is_empty() || stages.len() > 8 {
        return Err("rocket must have 1..=8 stages".into());
    }
    for stage in stages {
        rocket.stages.push(parse_stage(stage)?);
    }
    rocket.validate()?;
    Ok(rocket)
}

/// Serialize a rocket.
pub fn rocket_to_json(rocket: &Rocket) -> Value {
    let stages = rocket
        .stages
        .iter()
        .map(|stage| {
            let tanks = stage
                .tanks
                .iter()
                .map(|tank| {
                    object(vec![
                        ("name", string(tank.name.clone())),
                        ("propellant_mass", number(tank.propellant_mass)),
                        ("dry_mass", number(tank.dry_mass)),
                        (
                            "propellant",
                            object(vec![
                                ("name", string(tank.propellant.name.clone())),
                                ("density", number(tank.propellant.density)),
                            ]),
                        ),
                    ])
                })
                .collect();
            object(vec![
                ("name", string(stage.name.clone())),
                ("structure_mass", number(stage.structure_mass)),
                ("engine_count", number(f64::from(stage.engine_count))),
                ("reserve_fraction", number(stage.reserve_fraction)),
                (
                    "engine",
                    object(vec![
                        ("name", string(stage.engine.name.clone())),
                        ("thrust_vacuum", number(stage.engine.thrust_vacuum)),
                        ("thrust_sea_level", number(stage.engine.thrust_sea_level)),
                        ("isp_vacuum", number(stage.engine.isp_vacuum)),
                        ("isp_sea_level", number(stage.engine.isp_sea_level)),
                        ("mass", number(stage.engine.mass)),
                        ("min_throttle", number(stage.engine.min_throttle)),
                        ("gimbal_deg", number(stage.engine.gimbal_deg)),
                    ]),
                ),
                ("tanks", array(tanks)),
            ])
        })
        .collect();
    object(vec![
        ("name", string(rocket.name.clone())),
        ("payload_mass", number(rocket.payload_mass)),
        ("drag_area", number(rocket.drag_area)),
        ("drag_coefficient", number(rocket.drag_coefficient)),
        ("stages", array(stages)),
        ("summary", summary_json(rocket)),
    ])
}

fn summary_json(rocket: &Rocket) -> Value {
    let s = rocket.summary();
    object(vec![
        ("liftoff_mass", number(round(s.liftoff_mass, 1))),
        ("dry_mass", number(round(s.dry_mass, 1))),
        ("propellant_mass", number(round(s.propellant_mass, 1))),
        ("payload_fraction", number(round(s.payload_fraction, 6))),
        ("total_delta_v", number(round(s.total_delta_v, 1))),
        ("liftoff_twr", number(round(s.liftoff_twr, 3))),
        ("vacuum_twr_first_stage", number(round(s.vacuum_twr_first_stage, 3))),
        ("total_burn_time", number(round(s.total_burn_time, 1))),
        ("stage_count", number(s.stage_count as f64)),
    ])
}

fn check_report_json(report: &CheckReport) -> Value {
    let issues = report
        .issues
        .iter()
        .map(|issue| {
            object(vec![
                ("severity", string(issue.severity.as_str())),
                ("code", string(issue.code)),
                ("message", string(issue.message.clone())),
            ])
        })
        .collect();
    object(vec![
        ("ok", json::Value::Bool(report.ok())),
        ("errors", number(report.errors() as f64)),
        ("warnings", number(report.warnings() as f64)),
        ("issues", array(issues)),
        (
            "summary",
            report
                .summary
                .as_ref()
                .map(|s| object(vec![
                    ("liftoff_mass", number(round(s.liftoff_mass, 1))),
                    ("total_delta_v", number(round(s.total_delta_v, 1))),
                    ("liftoff_twr", number(round(s.liftoff_twr, 3))),
                ]))
                .unwrap_or(Value::Null),
        ),
    ])
}

/// Parse and run a validation request.
pub fn validate_rocket(value: &Value) -> Result<Value, String> {
    let rocket = parse_rocket(value)?;
    let report = inspect(&rocket);
    Ok(object(vec![
        ("ok", json::Value::Bool(report.ok())),
        (
            "report",
            check_report_json(&report),
        ),
        ("rocket", rocket_to_json(&rocket)),
    ]))
}

/// Parse a mission request.
///
/// The body may contain a full `rocket` object plus mission fields, or a
/// `script_id` (with an optional rocket override).
pub fn parse_mission(value: &Value) -> Result<(Rocket, MissionConfig), String> {
    let rocket = if let Some(rocket_value) = value.get("rocket") {
        parse_rocket(rocket_value)?
    } else if let Some(id) = value.string("rocket_id") {
        orbit_core::rocket::presets::by_id(id).ok_or_else(|| format!("unknown rocket '{id}'"))?
    } else {
        return Err("mission request needs 'rocket' or 'rocket_id'".into());
    };

    let mut config = if let Some(script_id) = value.string("script_id") {
        let script = orbit_flight::mission::scripts::by_id(script_id)
            .ok_or_else(|| format!("unknown script '{script_id}'"))?;
        orbit_flight::mission::scripts::config(&script)
    } else {
        MissionConfig::default()
    };
    if let Some(name) = value.string("name") {
        if name.trim().is_empty() || name.len() > 80 {
            return Err("mission name must be 1..=80 bytes".into());
        }
        config.name = name.to_string();
    }
    if let Some(alt) = value.get("lunar_altitude_m") {
        config.lunar_altitude = alt
            .as_f64()
            .ok_or_else(|| "lunar_altitude_m must be a number".to_string())?;
    }
    if let Some(epoch) = value.get("epoch_s") {
        config.epoch = epoch
            .as_f64()
            .ok_or_else(|| "epoch_s must be a number".to_string())?;
    }
    if let Some(phase) = value.get("moon_phase_deg") {
        config.moon_phase = phase
            .as_f64()
            .ok_or_else(|| "moon_phase_deg must be a number".to_string())?
            .to_radians();
    }
    if let Some(phase) = value.get("sun_phase_deg") {
        config.sun_phase = phase
            .as_f64()
            .ok_or_else(|| "sun_phase_deg must be a number".to_string())?
            .to_radians();
    }
    if let Some(alt) = value.get("parking_altitude_m") {
        config.ascent.target_altitude = alt
            .as_f64()
            .ok_or_else(|| "parking_altitude_m must be a number".to_string())?;
    }
    config.validate()?;
    Ok((rocket, config))
}

fn round(v: f64, digits: i32) -> f64 {
    if !v.is_finite() {
        return 0.0;
    }
    let scale = 10f64.powi(digits.clamp(0, 9));
    (v * scale).round() / scale
}

/// Serialize a mission result for the browser.
pub fn mission_to_json(result: &MissionResult) -> Value {
    let samples = result
        .samples
        .iter()
        .map(|s| {
            array(vec![
                number(round(s.t, 1)),
                number(round(s.altitude, 1)),
                number(round(s.speed, 2)),
                number(round(s.moon_distance, 0)),
                number(phase_index(s.phase)),
                number(round(s.mass, 1)),
                number(round(s.x, 1)),
                number(round(s.y, 1)),
            ])
        })
        .collect();
    let events = result.events.iter().map(event_to_json).collect();
    object(vec![
        ("status", string(result.status.as_str())),
        ("success", json::Value::Bool(result.success)),
        ("message", string(result.message.clone())),
        ("rocket_name", string(result.rocket_name.clone())),
        (
            "summary",
            result
                .summary
                .as_ref()
                .map(|s| {
                    object(vec![
                        ("liftoff_mass", number(round(s.liftoff_mass, 1))),
                        ("dry_mass", number(round(s.dry_mass, 1))),
                        ("propellant_mass", number(round(s.propellant_mass, 1))),
                        ("payload_fraction", number(round(s.payload_fraction, 6))),
                        ("total_delta_v", number(round(s.total_delta_v, 1))),
                        ("liftoff_twr", number(round(s.liftoff_twr, 3))),
                        ("total_burn_time", number(round(s.total_burn_time, 1))),
                        ("stage_count", number(s.stage_count as f64)),
                    ])
                })
                .unwrap_or(Value::Null),
        ),
        (
            "metrics",
            object(vec![
                ("total_time", number(round(result.total_time, 1))),
                ("delta_v_ascent", number(round(result.delta_v_ascent, 1))),
                ("delta_v_parking", number(round(result.delta_v_parking, 1))),
                ("delta_v_transfer", number(round(result.delta_v_transfer, 1))),
                ("delta_v_capture", number(round(result.delta_v_capture, 1))),
                ("propellant_remaining", number(round(result.propellant_remaining, 1))),
                ("final_mass", number(round(result.final_mass, 1))),
                ("target_altitude", number(round(result.target_altitude, 1))),
                (
                    "periapsis_error",
                    result
                        .periapsis_error
                        .map(|v| number(round(v, 1)))
                        .unwrap_or(Value::Null),
                ),
                (
                    "apoapsis_error",
                    result
                        .apoapsis_error
                        .map(|v| number(round(v, 1)))
                        .unwrap_or(Value::Null),
                ),
            ]),
        ),
        (
            "transfer",
            result
                .transfer
                .as_ref()
                .map(|t| {
                    object(vec![
                        ("coast_seconds", number(round(t.coast_seconds, 1))),
                        ("burn_delta_v", number(round(t.burn_delta_v, 1))),
                        ("burn_seconds", number(round(t.burn_seconds, 1))),
                        ("perilune_altitude", number(round(t.perilune_altitude, 1))),
                        ("residual", number(round(t.residual, 1))),
                        ("evaluations", number(f64::from(t.evaluations))),
                        ("coast_steps", number(f64::from(t.coast_steps))),
                    ])
                })
                .unwrap_or(Value::Null),
        ),
        (
            "lunar",
            result
                .lunar
                .as_ref()
                .map(|l| {
                    object(vec![
                        ("periapsis_altitude", number(round(l.periapsis_altitude, 1))),
                        ("apoapsis_altitude", number(round(l.apoapsis_altitude, 1))),
                        ("eccentricity", number(round(l.elements.eccentricity, 8))),
                        ("period", number(round(l.elements.period, 1))),
                        ("spent_delta_v", number(round(l.spent_delta_v, 1))),
                        ("iterations", number(f64::from(l.iterations))),
                        ("entry_periapsis_altitude", number(round(l.entry_periapsis_altitude, 1))),
                    ])
                })
                .unwrap_or(Value::Null),
        ),
        ("events", array(events)),
        ("samples", array(samples)),
    ])
}

fn phase_index(phase: Phase) -> f64 {
    match phase {
        Phase::Ascent => 0.0,
        Phase::Parking => 1.0,
        Phase::Transfer => 2.0,
        Phase::Lunar => 3.0,
    }
}

fn event_to_json(event: &MissionEvent) -> Value {
    let kind = format!("{:?}", event.kind);
    object(vec![
        ("t", number(round(event.t, 1))),
        ("phase", string(event.phase.as_str())),
        ("kind", string(kind)),
        ("detail", string(event.detail.clone())),
    ])
}

/// Run a conservation experiment and return JSON.
pub fn conservation_json() -> Result<Value, String> {
    let report = conservation_30_days()?;
    Ok(object(vec![
        ("days", number(report.days)),
        ("steps", number(f64::from(report.steps))),
        ("energy_before", number(report.energy_before)),
        ("energy_after", number(report.energy_after)),
        ("relative_energy_error", number(report.relative_energy_error)),
        ("relative_momentum_error", number(report.relative_momentum_error)),
    ]))
}

/// Synchronous run used by the test harness and the simple demo endpoint.
pub fn run_request(value: &Value) -> Result<(Rocket, MissionConfig, MissionResult), String> {
    let (rocket, config) = parse_mission(value)?;
    let result = run_mission(&rocket, &config)?;
    Ok((rocket, config, result))
}

/// Status string for a result, used by tests.
pub fn status_is_success(status: MissionStatus) -> bool {
    status == MissionStatus::Success
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_core::rocket::presets;

    #[test]
    fn rocket_round_trip() {
        for rocket in presets::all() {
            let value = rocket_to_json(&rocket);
            let parsed = parse_rocket(&value).unwrap();
            assert_eq!(parsed, rocket);
        }
    }

    #[test]
    fn malformed_rockets_fail() {
        for text in [
            r#"{}"#,
            r#"{"name":"x"}"#,
            r#"{"name":"x","stages":[]}"#,
            r#"{"name":"x","payload_mass":-1,"stages":[]}"#,
            r#"{"name":"x","stages":[{"name":"s"}]}"#,
        ] {
            let v = crate::json::parse(text).unwrap();
            assert!(parse_rocket(&v).is_err(), "accepted {text}");
        }
    }

    #[test]
    fn mission_request_uses_script() {
        let v = crate::json::parse(r#"{"script_id":"lunar-atlas-100","rocket_id":"kestrel"}"#).unwrap();
        let (rocket, config) = parse_mission(&v).unwrap();
        assert_eq!(rocket.name, "Kestrel");
        assert_eq!(config.lunar_altitude, 100_000.0);
    }

    #[test]
    fn result_serialization_is_bounded_and_finite() {
        let rocket = presets::sparrow();
        let result = run_mission(&rocket, &MissionConfig::default()).unwrap();
        let value = mission_to_json(&result);
        let text = crate::json::stringify(&value);
        assert!(text.len() > 1000);
        assert!(!text.contains("NaN"));
        let parsed = crate::json::parse(&text).unwrap();
        assert_eq!(parsed.string("status"), Some("success"));
    }
}
