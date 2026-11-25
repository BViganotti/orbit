//! Deterministic fuzz harness: 100,000 malformed documents plus structured
//! rocket, mission and ascent mutations. Any panic fails the run.

use std::panic::{self, catch_unwind, AssertUnwindSafe};

use orbit_core::math::Rng;
use orbit_core::rocket::presets;
use orbit_studio::json::{self, Value};
use orbit_studio::model;

fn main() {
    let mut rounds = 100_000usize;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--rounds" {
            rounds = args
                .next()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100_000);
        }
    }
    let previous = panic::take_hook();
    panic::set_hook(Box::new(|_info| {}));
    let mut failures = 0usize;

    // 1. Random documents. They may accidentally be valid JSON; the contract is
    // that they never panic and stay bounded.
    let mut rng = Rng::new(0x0f0_1234);
    let alphabet = b"{}[]\":,0123456789.eE+-truefalsenull \\n\\t\\u0001abcXYZ";
    let mut parsed = 0usize;
    let mut rejected = 0usize;
    for _ in 0..rounds {
        let len = rng.next_u64() as usize % 512;
        let mut text = String::with_capacity(len);
        for _ in 0..len {
            text.push(alphabet[rng.next_u64() as usize % alphabet.len()] as char);
        }
        match catch_unwind(AssertUnwindSafe(|| json::parse(&text))) {
            Ok(Ok(_)) => parsed += 1,
            Ok(Err(_)) => rejected += 1,
            Err(_) => {
                failures += 1;
                eprintln!("panic on random document: {text:?}");
            }
        }
    }

    // 2. Structured malformed rocket objects derived from a valid rocket.
    let base = model::rocket_to_json(&presets::atlas());
    let mut rocket_rejected = 0usize;
    let mut rocket_accepted = 0usize;
    for i in 0..2_000usize {
        let mut value = base.clone();
        if let Value::Object(entries) = &mut value {
            match i % 6 {
                0 => entries.retain(|(k, _)| k != "stages"),
                1 => entries.retain(|(k, _)| k != "name"),
                2 => entries.push(("payload_mass".into(), json::number(-1.0))),
                3 => entries.push(("stages".into(), Value::Array(vec![Value::Null; 9]))),
                4 => {
                    for entry in entries.iter_mut() {
                        if entry.0 == "drag_area" {
                            entry.1 = Value::String("x".into());
                        }
                    }
                }
                _ => {
                    let key = ["name", "payload_mass", "stages"][i % 3];
                    for entry in entries.iter_mut() {
                        if entry.0 == key {
                            entry.1 = Value::Null;
                        }
                    }
                }
            }
        }
        match catch_unwind(AssertUnwindSafe(|| model::parse_rocket(&value))) {
            Ok(Ok(_)) => rocket_accepted += 1,
            Ok(Err(_)) => rocket_rejected += 1,
            Err(_) => {
                failures += 1;
                eprintln!("panic on malformed rocket {i}");
            }
        }
    }

    // 3. Random mission requests.
    let mut mission_ok = 0usize;
    let mut mission_err = 0usize;
    for i in 0..1_000usize {
        let mut fields = Vec::new();
        if i % 2 == 0 {
            fields.push(("rocket_id".to_string(), Value::String("atlas".into())));
        }
        if i % 3 == 0 {
            fields.push(("script_id".to_string(), Value::String("lunar-atlas-100".into())));
        }
        fields.push((
            "lunar_altitude_m".to_string(),
            json::number(rng.range(-1.0e7, 1.0e8)),
        ));
        fields.push(("epoch_s".to_string(), json::number(rng.range(-1.0e12, 1.0e12))));
        let value = Value::Object(fields);
        match catch_unwind(AssertUnwindSafe(|| model::parse_mission(&value))) {
            Ok(Ok(_)) => mission_ok += 1,
            Ok(Err(_)) => mission_err += 1,
            Err(_) => {
                failures += 1;
                eprintln!("panic on mission request {i}");
            }
        }
    }

    // 4. Random ascent configurations with a valid vehicle.
    let mut ascents = 0usize;
    for i in 0..100usize {
        let mut rocket = presets::atlas();
        rocket.payload_mass = rng.range(100.0, 9000.0);
        for stage in &mut rocket.stages {
            stage.structure_mass *= rng.range(0.7, 1.3);
            stage.engine.isp_vacuum *= rng.range(0.95, 1.05);
        }
        let config = orbit_flight::ascent::AscentConfig {
            target_altitude: rng.range(80_000.0, 1_800_000.0),
            dt: rng.range(0.05, 1.0),
            max_time: rng.range(300.0, 1500.0),
            pitch_exponent: rng.range(0.15, 1.4),
            ..orbit_flight::ascent::AscentConfig::default()
        };
        if rocket.validate().is_err() || config.validate().is_err() {
            continue;
        }
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            orbit_flight::ascent::run_ascent(&rocket, &orbit_flight::nbody::Ephemeris::default(), 0.0, &config)
        }));
        match outcome {
            Ok(Ok(result)) => {
                ascents += 1;
                let ok = result.state.is_finite()
                    && result.samples.iter().all(|s| s.position.is_finite() && s.speed.is_finite());
                if !ok {
                    failures += 1;
                    eprintln!("non-finite ascent sample at iteration {i}");
                }
            }
            Ok(Err(_)) => {}
            Err(_) => {
                failures += 1;
                eprintln!("panic on ascent iteration {i}");
            }
        }
    }

    panic::set_hook(previous);
    println!(
        "fuzz: {} random documents ({} parsed, {} rejected), rockets {} rejected / {} accepted, missions {} ok / {} rejected, {} ascents, {} failures",
        rounds, parsed, rejected, rocket_rejected, rocket_accepted, mission_ok, mission_err, ascents, failures
    );
    if failures > 0 {
        std::process::exit(1);
    }
    println!("PASS: no panics and no non-finite state");
}
