//! Standard-library benchmarks. Run with `--release` and an explicit scale.
//!
//! Example: cargo run --release -p orbit-tools --bin orbit-bench -- --scale 2

use std::hint::black_box;
use std::time::Instant;

use orbit_core::math::{DormandPrince, Flow, Rng};
use orbit_core::rocket::presets;
use orbit_flight::ascent::{run_ascent, AscentConfig};
use orbit_flight::mission::{run_mission, MissionConfig};
use orbit_flight::nbody::system::conservation_30_days;
use orbit_flight::nbody::Ephemeris;
use orbit_flight::plan::{circularize, plan_tli, BurnConfig, Vehicle};
use orbit_studio::json;
use orbit_studio::model;

fn time(label: &str, iterations: u32, mut f: impl FnMut()) {
    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    let elapsed = start.elapsed().as_secs_f64();
    let per = elapsed / f64::from(iterations);
    println!(
        "{:<28} {:>10.3} ms/op {:>12.1} ops/s  ({} iterations)",
        label,
        per * 1000.0,
        1.0 / per.max(1.0e-12),
        iterations
    );
}

fn main() {
    let mut scale = 1u32;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--scale" {
            scale = args
                .next()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1)
                .clamp(1, 20);
        }
    }
    let eph = Ephemeris::default();

    // JSON parse/stringify throughput.
    let mut values = Vec::new();
    for i in 0..500 {
        values.push(json::object(vec![
            ("id", json::number(i as f64)),
            ("name", json::string(format!("sample-{i}"))),
            (
                "values",
                json::array((0..20).map(|j| json::number(j as f64 * 1.5)).collect()),
            ),
        ]));
    }
    let document = json::stringify(&json::array(values));
    println!("JSON document: {:.1} KiB", document.len() as f64 / 1024.0);
    time("json parse", 500 * scale, || {
        black_box(json::parse(black_box(&document)).unwrap());
    });
    time("json stringify", 500 * scale, || {
        black_box(json::stringify(black_box(&json::array(vec![
            json::number(1.0),
        ]))));
    });

    // Ascent.
    let rocket = presets::atlas();
    let mut ascent_config = AscentConfig::default();
    ascent_config.target_altitude = 400_000.0;
    time("atlas ascent", 10 * scale, || {
        let result = run_ascent(&rocket, &eph, 0.0, &ascent_config).unwrap();
        black_box(result.state);
    });

    // Full mission.
    let mission_config = MissionConfig::default();
    let mut success = 0;
    time("atlas full mission", 5 * scale, || {
        let result = run_mission(&rocket, &mission_config).unwrap();
        if result.success {
            success += 1;
        }
    });
    println!("successful mission iterations: {success}");

    // TLI evaluation (ascent + parking + one plan).
    let ascent = run_ascent(&rocket, &eph, 0.0, &ascent_config).unwrap();
    let mut vehicle = Vehicle::from_ascent(
        &rocket,
        ascent.stage_index,
        ascent.propellant_remaining,
        ascent.mass,
    )
    .unwrap();
    let (state, t, _, _) = circularize(
        &eph,
        &rocket,
        &mut vehicle,
        ascent.t,
        ascent.state,
        &BurnConfig::default(),
    )
    .unwrap();
    time("tli plan (98 evals)", 5 * scale, || {
        let plan = plan_tli(&eph, &rocket, &vehicle, t, state, 100_000.0).unwrap();
        black_box(plan.burn_delta_v);
    });

    // Coupled n-body conservation.
    let mut steps = 0u32;
    time("nbody 30-day conservation", 2 * scale, || {
        let report = conservation_30_days().unwrap();
        steps += report.steps;
    });
    let elapsed_steps = steps;
    println!("conservation steps executed: {elapsed_steps}");

    // Integrator raw throughput.
    let dp = DormandPrince::default();
    let harmonic = |_t: f64, y: &[f64], out: &mut [f64]| {
        out[0] = y[1];
        out[1] = -y[0];
    };
    let start = Instant::now();
    let mut total_steps = 0u32;
    for _ in 0..10 * scale {
        let mut y = [1.0, 0.0];
        let report = dp
            .propagate(
                &harmonic,
                &mut y,
                0.0,
                100.0 * std::f64::consts::TAU,
                0.1,
                &mut |_, _| Flow::Continue,
            )
            .unwrap();
        total_steps += report.steps;
        black_box(y);
    }
    let elapsed = start.elapsed().as_secs_f64();
    println!(
        "{:<28} {:>10.1} ksteps/s ({:.1} ms, {} steps)",
        "dormand-prince harmonic",
        f64::from(total_steps) / elapsed / 1000.0,
        elapsed * 1000.0,
        total_steps
    );

    // Deterministic RNG throughput.
    let mut rng = Rng::new(7);
    time("rng u64", 1_000_000 * scale, || {
        black_box(rng.next_u64());
    });

    // Rocket JSON round trip.
    let rocket_json = model::rocket_to_json(&rocket);
    let rocket_text = json::stringify(&rocket_json);
    time("rocket parse", 2_000 * scale, || {
        let value = json::parse(black_box(&rocket_text)).unwrap();
        black_box(model::parse_rocket(&value).unwrap());
    });
}
