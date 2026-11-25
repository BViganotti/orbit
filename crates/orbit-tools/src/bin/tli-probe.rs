use orbit_core::rocket::presets;
use orbit_flight::ascent::{run_ascent, AscentConfig};
use orbit_flight::nbody::Ephemeris;
use orbit_flight::plan::{circularize, plan_tli, BurnConfig, Vehicle};

fn main() {
    let name = std::env::args().nth(1).unwrap_or_else(|| "atlas".into());
    let rocket = presets::by_id(&name).expect("rocket");
    let eph = Ephemeris::default();
    let mut cfg = AscentConfig::default();
    cfg.target_altitude = 400_000.0;
    let a = run_ascent(&rocket, &eph, 0.0, &cfg).expect("ascent");
    let mut v =
        Vehicle::from_ascent(&rocket, a.stage_index, a.propellant_remaining, a.mass).unwrap();
    let (state, t, spent, status) =
        circularize(&eph, &rocket, &mut v, a.t, a.state, &BurnConfig::default()).expect("circ");
    println!(
        "ascent ok={} alt={:.0} rem={:.0}; circ spent={:.0} {:?} alt={:.0} rem={:.0}",
        a.success,
        a.state.altitude(),
        a.remaining_delta_v(&rocket),
        spent,
        status,
        state.altitude(),
        v.remaining_delta_v(&rocket)
    );
    let start = std::time::Instant::now();
    match plan_tli(&eph, &rocket, &v, t, state, 100_000.0) {
        Ok(p) => println!("TLI ok: dv={:.2} wait={:.0}s burn_t={:.0} peri_alt={:.3}km resid={:.1}m evals={} closest_t={:.0} rel_v={:.0} {:?}",
            p.burn_delta_v,p.coast_duration,p.burn_time,p.closest.altitude/1000.0,p.residual,p.evaluations,p.closest.time,p.closest.relative_speed,start.elapsed()),
        Err(e) => println!("TLI err: {:?} {:?}", e, start.elapsed()),
    }
}
