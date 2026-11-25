use orbit_core::rocket::presets;
use orbit_flight::mission::{run_mission, MissionConfig};

fn main() {
    let name = std::env::args().nth(1).unwrap_or_else(|| "atlas".into());
    let rocket = presets::by_id(&name).expect("rocket");
    let cfg = MissionConfig::default();
    let start = std::time::Instant::now();
    match run_mission(&rocket, &cfg) {
        Ok(m) => {
            println!("status={:?} success={} msg={}", m.status, m.success, m.message);
            println!("t={:.0} dv: ascent={:.0} parking={:.0} tli={:.0} capture={:.0} final_mass={:.0} prop={:.0}",
                m.total_time, m.delta_v_ascent, m.delta_v_parking, m.delta_v_transfer, m.delta_v_capture, m.final_mass, m.propellant_remaining);
            if let Some(t) = &m.transfer { println!("transfer: {:?}", t); }
            if let Some(l) = &m.lunar {
                println!("lunar: entry={:.3}x{:.3} peri={:.3} apo={:.3} e={:.6} spent={:.0} iters={} status={:?} err=({:.1},{:.1})",
                    l.entry_periapsis_altitude/1000.0,l.entry_apoapsis_altitude/1000.0,l.periapsis_altitude/1000.0,l.apoapsis_altitude/1000.0,l.elements.eccentricity,l.spent_delta_v,l.iterations,l.status,
                    m.periapsis_error.unwrap_or(0.0)/1000.0,m.apoapsis_error.unwrap_or(0.0)/1000.0);
            }
            for e in &m.events { println!("  t={:>10.0} {:?} {:?}: {}", e.t, e.phase, e.kind, e.detail); }
            println!("samples={} elapsed={:?}", m.samples.len(), start.elapsed());
        }
        Err(e) => println!("ERR {e}"),
    }
}
