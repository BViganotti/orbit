//! Mission diagnostics and exports.
//!
//! All output is plain text generated from real simulation state. Missing or
//! non-finite values are written as `nan` in CSV and called out in the report;
//! no value is invented.

use std::fmt::Write as _;

use orbit_core::constants::{MU_EARTH, MU_MOON, R_EARTH, R_MOON};
use orbit_core::math::Vec2;

use crate::mission::{MissionResult, MissionStatus};
use crate::nbody::gravity::elements;
use crate::nbody::{Body, Ephemeris, State};

pub use crate::nbody::system::{conservation_30_days, ConservationReport, FullSystem};

/// Instantaneous orbital diagnostics for one state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StateDiagnostics {
    pub altitude: f64,
    pub speed: f64,
    pub earth_specific_energy: f64,
    pub earth_angular_momentum: f64,
    pub earth_eccentricity: f64,
    pub earth_periapsis_altitude: f64,
    pub earth_apoapsis_altitude: f64,
    pub moon_distance: f64,
    pub moon_specific_energy: Option<f64>,
    pub moon_periapsis_altitude: Option<f64>,
    pub flight_path_angle: f64,
}

/// Compute diagnostics for a state in the analytic ephemeris.
pub fn diagnose(eph: &Ephemeris, t: f64, state: State) -> StateDiagnostics {
    let r = state.position.norm();
    let altitude = r - R_EARTH;
    let speed = state.velocity.norm();
    let energy = if r > 0.0 {
        0.5 * speed * speed - MU_EARTH / r
    } else {
        f64::NAN
    };
    let h = state.position.cross(state.velocity);
    let earth_el = elements(state.position, state.velocity, MU_EARTH);
    let moon = eph.position(Body::Moon, t);
    let moon_v = eph.velocity(Body::Moon, t);
    let rel = state.position - moon;
    let rel_v = state.velocity - moon_v;
    let rel_el = elements(rel, rel_v, MU_MOON);
    let up = state.position.normalized_or_x();
    let east = up.perp();
    let fpa = state.velocity.dot(up).atan2(state.velocity.dot(east));
    StateDiagnostics {
        altitude,
        speed,
        earth_specific_energy: energy,
        earth_angular_momentum: h,
        earth_eccentricity: earth_el.map(|e| e.eccentricity).unwrap_or(f64::NAN),
        earth_periapsis_altitude: earth_el.map(|e| e.periapsis - R_EARTH).unwrap_or(f64::NAN),
        earth_apoapsis_altitude: earth_el.map(|e| e.apoapsis - R_EARTH).unwrap_or(f64::NAN),
        moon_distance: rel.norm(),
        moon_specific_energy: rel_el.map(|e| e.specific_energy),
        moon_periapsis_altitude: rel_el.map(|e| e.periapsis - R_MOON),
        flight_path_angle: fpa,
    }
}

/// Export the mission trajectory as CSV.
pub fn trajectory_csv(result: &MissionResult) -> String {
    let mut out = String::with_capacity(result.samples.len() * 80 + 128);
    out.push_str("time_s,phase,altitude_m,speed_m_s,moon_distance_m,mass_kg\n");
    for sample in &result.samples {
        let _ = writeln!(
            out,
            "{:.3},{},{:.3},{:.3},{:.3},{:.3}",
            sample.t,
            sample.phase.as_str(),
            finite_or_nan(sample.altitude),
            finite_or_nan(sample.speed),
            finite_or_nan(sample.moon_distance),
            finite_or_nan(sample.mass)
        );
        if out.len() > 16 * 1024 * 1024 {
            out.push_str("# truncated at 16 MiB\n");
            break;
        }
    }
    out
}

fn finite_or_nan(v: f64) -> f64 {
    if v.is_finite() {
        v
    } else {
        f64::NAN
    }
}

/// Human-readable mission report.
pub fn mission_report(result: &MissionResult) -> String {
    let mut out = String::with_capacity(4096);
    let _ = writeln!(out, "ORBIT MISSION REPORT");
    let _ = writeln!(out, "====================");
    let _ = writeln!(out, "mission:     {}", result.rocket_name);
    let _ = writeln!(
        out,
        "status:      {} ({})",
        result.status.as_str(),
        result.message
    );
    let _ = writeln!(
        out,
        "duration:    {:.1} s ({:.2} h)",
        result.total_time,
        result.total_time / 3600.0
    );
    if let Some(s) = &result.summary {
        let _ = writeln!(out);
        let _ = writeln!(out, "VEHICLE");
        let _ = writeln!(out, "  liftoff mass      {:>12.0} kg", s.liftoff_mass);
        let _ = writeln!(out, "  dry mass          {:>12.0} kg", s.dry_mass);
        let _ = writeln!(out, "  propellant        {:>12.0} kg", s.propellant_mass);
        let _ = writeln!(out, "  ideal delta-v     {:>12.0} m/s", s.total_delta_v);
        let _ = writeln!(out, "  liftoff TWR       {:>12.2}", s.liftoff_twr);
        let _ = writeln!(out, "  stages            {:>12}", s.stage_count);
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "DELTA-V BUDGET");
    let _ = writeln!(
        out,
        "  ascent            {:>12.1} m/s",
        result.delta_v_ascent
    );
    let _ = writeln!(
        out,
        "  parking           {:>12.1} m/s",
        result.delta_v_parking
    );
    let _ = writeln!(
        out,
        "  trans-lunar       {:>12.1} m/s",
        result.delta_v_transfer
    );
    let _ = writeln!(
        out,
        "  lunar capture     {:>12.1} m/s",
        result.delta_v_capture
    );
    let total = result.delta_v_ascent
        + result.delta_v_parking
        + result.delta_v_transfer
        + result.delta_v_capture;
    let _ = writeln!(out, "  total             {:>12.1} m/s", total);
    let _ = writeln!(out, "  final mass        {:>12.0} kg", result.final_mass);
    let _ = writeln!(
        out,
        "  propellant left   {:>12.0} kg",
        result.propellant_remaining
    );
    if let Some(ascent) = &result.ascent {
        let _ = writeln!(out);
        let _ = writeln!(out, "ASCENT");
        let _ = writeln!(out, "  powered time      {:>12.1} s", ascent.t);
        let _ = writeln!(out, "  ascent delta-v    {:>12.1} m/s", ascent.delta_v);
        let _ = writeln!(out, "  max dynamic press {:>12.1} Pa", ascent.max_q);
        let _ = writeln!(out, "  max acceleration  {:>12.2} g", ascent.max_g);
        let _ = writeln!(
            out,
            "  max altitude      {:>12.1} km",
            ascent.max_altitude / 1000.0
        );
        if let Some(peri) = ascent.periapsis_altitude() {
            let _ = writeln!(out, "  insertion periapsis {:>10.1} km", peri / 1000.0);
        }
        if let Some(apo) = ascent.apoapsis_altitude() {
            let _ = writeln!(out, "  insertion apoapsis  {:>10.1} km", apo / 1000.0);
        }
    }
    if let Some(transfer) = &result.transfer {
        let _ = writeln!(out);
        let _ = writeln!(out, "TRANSFER");
        let _ = writeln!(
            out,
            "  parking coast     {:>12.1} s",
            transfer.coast_seconds
        );
        let _ = writeln!(
            out,
            "  TLI burn          {:>12.1} m/s over {:.1} s",
            transfer.burn_delta_v, transfer.burn_seconds
        );
        let _ = writeln!(
            out,
            "  perilune          {:>12.1} km",
            transfer.perilune_altitude / 1000.0
        );
        let _ = writeln!(out, "  target residual   {:>12.1} m", transfer.residual);
        let _ = writeln!(out, "  evaluations       {:>12}", transfer.evaluations);
    }
    if let Some(lunar) = &result.lunar {
        let _ = writeln!(out);
        let _ = writeln!(out, "LUNAR ORBIT");
        let _ = writeln!(
            out,
            "  periapsis         {:>12.3} km",
            lunar.periapsis_altitude / 1000.0
        );
        let _ = writeln!(
            out,
            "  apoapsis          {:>12.3} km",
            lunar.apoapsis_altitude / 1000.0
        );
        let _ = writeln!(
            out,
            "  eccentricity      {:>12.6}",
            lunar.elements.eccentricity
        );
        let _ = writeln!(
            out,
            "  target            {:>12.3} km",
            result.target_altitude / 1000.0
        );
        if let Some(e) = result.periapsis_error {
            let _ = writeln!(out, "  periapsis error   {:>12.3} km", e / 1000.0);
        }
        if let Some(e) = result.apoapsis_error {
            let _ = writeln!(out, "  apoapsis error    {:>12.3} km", e / 1000.0);
        }
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "EVENTS");
    for event in &result.events {
        let _ = writeln!(
            out,
            "  {:>10.1} s  {:<8} {:?}: {}",
            event.t,
            event.phase.as_str(),
            event.kind,
            event.detail
        );
    }
    if result.status != MissionStatus::Success {
        let _ = writeln!(out);
        let _ = writeln!(out, "RESULT: FAILED - {}", result.message);
    } else {
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "RESULT: SUCCESS - lunar orbit achieved within tolerance"
        );
    }
    out
}

/// A compact one-line summary for logs and the Studio event list.
pub fn mission_headline(result: &MissionResult) -> String {
    match (&result.lunar, result.success) {
        (Some(l), true) => format!(
            "{}: {:.1} x {:.1} km lunar orbit, {:.0} m/s total",
            result.rocket_name,
            l.periapsis_altitude / 1000.0,
            l.apoapsis_altitude / 1000.0,
            result.delta_v_ascent
                + result.delta_v_parking
                + result.delta_v_transfer
                + result.delta_v_capture
        ),
        _ => format!("{}: {}", result.rocket_name, result.message),
    }
}

/// Relative angular separation between two planar vectors, radians.
pub fn angular_separation(a: Vec2, b: Vec2) -> f64 {
    let cross = a.cross(b);
    let dot = a.dot(b);
    cross.atan2(dot).abs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mission::{run_mission, MissionConfig};
    use orbit_core::rocket::presets;

    #[test]
    fn circular_orbit_diagnostics_are_consistent() {
        let eph = Ephemeris::default();
        let r = R_EARTH + 400_000.0;
        let v = (MU_EARTH / r).sqrt();
        let state = State::new(Vec2::new(r, 0.0), Vec2::new(0.0, v));
        let d = diagnose(&eph, 0.0, state);
        assert!(d.altitude > 399_000.0);
        assert!(d.earth_eccentricity < 1e-6);
        assert!(d.earth_specific_energy < 0.0);
        assert!(d.flight_path_angle.abs() < 1e-9);
        assert!(d.moon_distance > 3.0e8);
    }

    #[test]
    fn report_and_csv_contain_real_values() {
        let rocket = presets::atlas();
        let result = run_mission(&rocket, &MissionConfig::default()).unwrap();
        let csv = trajectory_csv(&result);
        assert!(csv.starts_with("time_s,phase,altitude_m"));
        assert!(csv.lines().count() > 100);
        assert!(!csv.contains("NaN"));
        let report = mission_report(&result);
        assert!(report.contains("ORBIT MISSION REPORT"));
        assert!(report.contains("LUNAR ORBIT"));
        assert!(report.contains("RESULT:"));
        assert!(mission_headline(&result).contains("km lunar orbit"));
    }

    #[test]
    fn angular_separation_is_bounded() {
        let a = Vec2::new(1.0, 0.0);
        assert!((angular_separation(a, a)).abs() < 1e-15);
        assert!((angular_separation(a, -a) - std::f64::consts::PI).abs() < 1e-12);
    }
}
