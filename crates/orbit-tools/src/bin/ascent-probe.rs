use orbit_core::rocket::presets;
use orbit_flight::ascent::{run_ascent, AscentConfig};
use orbit_flight::nbody::Ephemeris;
fn main() {
    for exp in [0.65,0.7,0.75,0.8,0.85] {
        println!("=== {exp}");
        for rocket in presets::all() {
            let mut cfg=AscentConfig::default(); cfg.target_altitude=400_000.0; cfg.pitch_exponent=exp;
            let r=run_ascent(&rocket,&Ephemeris::default(),0.0,&cfg).expect("ascent");
            println!("{:<9} ok={} {:?} t={:.0} rem={:.0} peri={:.0} apo={:.0} alt={:.0}",
                rocket.name,r.success,r.status,r.t,r.remaining_delta_v(&rocket),
                r.periapsis_altitude().unwrap_or(-1.0),r.apoapsis_altitude().unwrap_or(-1.0),r.state.altitude());
        }
    }
}
