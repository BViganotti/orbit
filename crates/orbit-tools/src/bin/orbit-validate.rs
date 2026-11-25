//! Validate a rocket JSON file without starting the Studio.
//!
//! Usage: orbit-validate path/to/rocket.json

use std::io::Read;

use orbit_core::rocket::checks::inspect;
use orbit_studio::json;
use orbit_studio::model;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: orbit-validate <rocket.json>");
        std::process::exit(2);
    };
    let mut text = String::new();
    match std::fs::File::open(&path).and_then(|mut f| f.read_to_string(&mut text)) {
        Ok(_) => {}
        Err(error) => {
            eprintln!("{path}: {error}");
            std::process::exit(1);
        }
    }
    if text.len() > json::MAX_INPUT {
        eprintln!("{path}: file exceeds the 2 MiB input limit");
        std::process::exit(1);
    }
    let value = match json::parse(&text) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("{path}: {error}");
            std::process::exit(1);
        }
    };
    let rocket = match model::parse_rocket(&value) {
        Ok(rocket) => rocket,
        Err(error) => {
            eprintln!("{path}: {error}");
            std::process::exit(1);
        }
    };
    let report = inspect(&rocket);
    println!("{}: {} stage(s)", rocket.name, rocket.stages.len());
    let summary = rocket.summary();
    println!(
        "liftoff {:.0} kg, ideal delta-v {:.0} m/s, liftoff TWR {:.2}",
        summary.liftoff_mass, summary.total_delta_v, summary.liftoff_twr
    );
    for issue in &report.issues {
        println!("  [{:?}] {}: {}", issue.severity, issue.code, issue.message);
    }
    if !report.ok() {
        std::process::exit(1);
    }
}
