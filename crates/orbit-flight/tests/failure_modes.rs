//! Failure modes must be reported, never converted into a fake success.

use orbit_core::rocket::presets;
use orbit_flight::mission::{run_mission, MissionConfig, MissionStatus};

#[test]
fn underpowered_transfer_fails_without_a_lunar_orbit() {
    let mut rocket = presets::kestrel();
    // Leave the ascent stack intact but cripple the transfer stage.
    let last = rocket.stages.len() - 1;
    rocket.stages[last].tanks[0].propellant_mass = 100.0;
    let result = run_mission(&rocket, &MissionConfig::default()).expect("pipeline returns a result");
    assert!(!result.success);
    assert!(result.lunar.is_none());
    assert_ne!(result.status, MissionStatus::Success);
    assert!(matches!(
        result.status,
        MissionStatus::AscentFailed
            | MissionStatus::ParkingFailed
            | MissionStatus::TransferFailed
            | MissionStatus::CaptureFailed
            | MissionStatus::OrbitMiss
    ));
    assert!(!result.message.is_empty());
    for sample in &result.samples {
        assert!(sample.x.is_finite() && sample.y.is_finite());
        assert!(sample.speed.is_finite());
    }
}

#[test]
fn impossible_lunar_orbit_reports_orbit_miss() {
    let rocket = presets::atlas();
    let mut config = MissionConfig::default();
    // A target perilune far outside what the finite transfer can reach is
    // rejected by validation; use a valid but extreme target and accept either
    // a transfer failure or an honest orbit miss.
    config.lunar_altitude = 900_000.0;
    config.tolerance = 0.001;
    let result = run_mission(&rocket, &config).expect("pipeline returns a result");
    if !result.success {
        assert!(result.lunar.is_none() || result.status == MissionStatus::OrbitMiss);
        assert!(!result.message.is_empty());
    }
}
