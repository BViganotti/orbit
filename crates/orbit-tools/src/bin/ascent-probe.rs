use orbit_core::rocket::presets;
use orbit_flight::ascent::{run_ascent, AscentConfig};
use orbit_flight::nbody::Ephemeris;
fn main() {
    for rocket in presets::all() {
        let total=rocket.total_delta_v();
        let mut cfg=AscentConfig::default(); cfg.target_altitude=400_000.0;
        let r=run_ascent(&rocket,&Ephemeris::default(),0.0,&cfg).expect("ascent");
        println!("{:<10} total={:.0} ok={} {:?} t={:.0} rem={:.0} peri={:.0} apo={:.0} alt={:.0}",
            rocket.name,total,r.success,r.status,r.t,r.remaining_delta_v(&rocket),
            r.periapsis_altitude().unwrap_or(-1.0),r.apoapsis_altitude().unwrap_or(-1.0),r.state.altitude());
    }
}
