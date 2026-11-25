//! Project acceptance tests from PLAN.md / REQUIREMENTS.md.

use orbit_core::constants::DAY;
use orbit_core::math::Rng;
use orbit_core::rocket::{presets, Rocket};
use orbit_core::G0;
use orbit_flight::ascent::{run_ascent, AscentConfig};
use orbit_flight::mission::{run_mission, MissionConfig, MissionStatus};
use orbit_flight::nbody::conservation_30_days;
use orbit_flight::nbody::Ephemeris;

#[test]
fn tsiolkovsky_matches_independent_computation_within_0_1_percent() {
    for rocket in presets::all() {
        let mut total = 0.0;
        for (i, stage) in rocket.stages.iter().enumerate() {
            let above = rocket.mass_above(i);
            let wet = above + stage.wet_mass();
            let dry = above + stage.dry_mass() + stage.propellant_mass() * stage.reserve_fraction;
            let expected = stage.engine.isp_vacuum * G0 * (wet / dry).ln();
            let actual = rocket.stage_delta_v(i);
            let error = (actual - expected).abs() / expected.max(1.0);
            assert!(
                error <= 0.001,
                "{} stage {} error {}",
                rocket.name,
                i + 1,
                error
            );
            total += actual;
        }
        assert!((total - rocket.total_delta_v()).abs() <= 0.001 * total.max(1.0));
    }
}

#[test]
fn thirty_day_full_nbody_energy_conservation() {
    let report = conservation_30_days().expect("conservation run");
    assert!(
        report.relative_energy_error < 1.0e-6,
        "energy drift {:e}",
        report.relative_energy_error
    );
    assert!(
        report.relative_momentum_error < 1.0e-9,
        "momentum drift {:e}",
        report.relative_momentum_error
    );
}

#[test]
fn ascent_reaches_a_parking_orbit_for_every_preset() {
    let eph = Ephemeris::default();
    for rocket in presets::all() {
        let mut config = AscentConfig::default();
        config.target_altitude = 400_000.0;
        let result = run_ascent(&rocket, &eph, 0.0, &config).unwrap();
        assert!(result.success, "{}: {:?}", rocket.name, result.status);
        let orbit = result.orbit.expect("orbit elements");
        assert!(orbit.periapsis > orbit_core::constants::R_EARTH + 300_000.0);
        assert!(orbit.apoapsis.is_finite());
        assert!(result.max_q > 0.0);
        assert!(result.delta_v.is_finite() && result.delta_v > 1000.0);
    }
}

#[test]
fn one_hundred_randomized_missions_never_produce_nan_or_unbounded_state() {
    let mut rng = Rng::new(0x0b17_2026);
    let mut successes = 0;
    let mut failures = 0;
    for i in 0..100 {
        let base = if i % 2 == 0 {
            presets::atlas()
        } else {
            presets::voyager()
        };
        let rocket = perturb(&mut rng, base);
        if rocket.validate().is_err() {
            failures += 1;
            continue;
        }
        let mut config = MissionConfig::default();
        config.name = format!("random-{i}");
        config.lunar_altitude = rng.range(70_000.0, 160_000.0);
        config.epoch = rng.range(0.0, 20.0 * DAY);
        config.moon_phase = rng.range(0.0, std::f64::consts::TAU);
        config.sun_phase = rng.range(0.0, std::f64::consts::TAU);
        config.ascent.target_altitude = 400_000.0;
        config.validate().expect("generated config is valid");
        let result = run_mission(&rocket, &config).expect("mission should not error");
        assert_ne!(result.status, MissionStatus::Invalid);
        assert!(result.total_time.is_finite() && result.total_time < 1.0e9);
        assert!(result.final_mass.is_finite() && result.final_mass >= 0.0);
        assert!(result.message.len() < 500);
        assert!(result.samples.len() < 30_000);
        for sample in &result.samples {
            assert!(sample.t.is_finite());
            assert!(sample.altitude.is_finite());
            assert!(sample.speed.is_finite() && sample.speed.abs() < 1.0e8);
            assert!(sample.moon_distance.is_finite());
            assert!(sample.mass.is_finite() && sample.mass >= 0.0 && sample.mass < 1.0e8);
            assert!(sample.x.is_finite() && sample.x.abs() < 1.0e12);
            assert!(sample.y.is_finite() && sample.y.abs() < 1.0e12);
        }
        if let Some(lunar) = &result.lunar {
            assert!(lunar.periapsis_altitude.is_finite());
            assert!(lunar.apoapsis_altitude.is_finite());
            assert!(lunar.elements.eccentricity.is_finite());
        }
        if result.success {
            successes += 1;
        } else {
            failures += 1;
        }
    }
    assert_eq!(successes + failures, 100);
    eprintln!("randomized missions: {successes} successes, {failures} honest failures");
    // Randomised perturbations are allowed to fail honestly, but the pipeline
    // must remain finite. Report the observed split in the failure message.
    assert!(
        successes >= 80,
        "only {successes}/100 randomised missions succeeded"
    );
}

fn perturb(rng: &mut Rng, mut rocket: Rocket) -> Rocket {
    rocket.payload_mass = (rocket.payload_mass * rng.range(0.85, 1.15)).max(200.0);
    for stage in &mut rocket.stages {
        stage.structure_mass = (stage.structure_mass * rng.range(0.9, 1.1)).max(10.0);
        for tank in &mut stage.tanks {
            tank.propellant_mass *= rng.range(0.97, 1.03);
        }
        stage.engine.isp_vacuum *= rng.range(0.99, 1.01);
        stage.engine.isp_sea_level = stage.engine.isp_sea_level.min(stage.engine.isp_vacuum);
    }
    rocket
}
