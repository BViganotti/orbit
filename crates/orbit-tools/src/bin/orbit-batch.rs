//! Run every scripted mission and report the real outcome.

use orbit_core::rocket::presets;
use orbit_flight::mission::{run_mission, scripts};

fn main() {
    let mut filter: Option<String> = None;
    let mut json = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => json = true,
            "--filter" => filter = args.next(),
            _ => {}
        }
    }
    let mut failures = 0;
    let mut rows = Vec::new();
    for script in &scripts::SCRIPTS {
        if let Some(filter) = &filter {
            if !script.id.contains(filter.as_str()) {
                continue;
            }
        }
        let Some(rocket) = presets::by_id(script.rocket_id) else {
            eprintln!("{}: unknown rocket {}", script.id, script.rocket_id);
            failures += 1;
            continue;
        };
        let config = scripts::config(script);
        let start = std::time::Instant::now();
        let result = match run_mission(&rocket, &config) {
            Ok(result) => result,
            Err(error) => {
                eprintln!("{}: {}", script.id, error);
                failures += 1;
                continue;
            }
        };
        let elapsed = start.elapsed();
        let (peri, apo, ecc) = match &result.lunar {
            Some(l) => (
                l.periapsis_altitude / 1000.0,
                l.apoapsis_altitude / 1000.0,
                l.elements.eccentricity,
            ),
            None => (f64::NAN, f64::NAN, f64::NAN),
        };
        if !result.success {
            failures += 1;
        }
        rows.push((
            script.id.to_string(),
            result.success,
            peri,
            apo,
            ecc,
            result.delta_v_ascent
                + result.delta_v_parking
                + result.delta_v_transfer
                + result.delta_v_capture,
            elapsed.as_secs_f64(),
        ));
    }
    if json {
        println!("[");
        for (i, row) in rows.iter().enumerate() {
            println!(
                "  {{\"id\":\"{}\",\"success\":{},\"periapsis_km\":{:.2},\"apoapsis_km\":{:.2},\"eccentricity\":{:.7},\"delta_v\":{:.1},\"wall_s\":{:.3}}}{}",
                row.0,
                row.1,
                row.2,
                row.3,
                row.4,
                row.5,
                row.6,
                if i + 1 == rows.len() { "" } else { "," }
            );
        }
        println!("]");
    } else {
        println!(
            "{:<24} {:>7} {:>10} {:>10} {:>11} {:>10} {:>8}",
            "mission", "ok", "peri km", "apo km", "ecc", "dv m/s", "wall s"
        );
        for row in &rows {
            println!(
                "{:<24} {:>7} {:>10.2} {:>10.2} {:>11.7} {:>10.0} {:>8.3}",
                row.0, row.1, row.2, row.3, row.4, row.5, row.6
            );
        }
        println!(
            "\n{}/{} missions succeeded",
            rows.len() - failures,
            rows.len()
        );
    }
    if failures > 0 {
        std::process::exit(1);
    }
}
