//! Manoeuvre-level integration tests.

use orbit_core::constants::{MU_EARTH, R_EARTH};
use orbit_core::math::Vec2;
use orbit_core::rocket::presets;
use orbit_flight::nbody::{Ephemeris, State};
use orbit_flight::plan::{finite_burn, prograde, retrograde, BurnConfig, Vehicle};

fn parking() -> State {
    let r = R_EARTH + 500_000.0;
    let v = (MU_EARTH / r).sqrt();
    State::new(Vec2::new(r, 0.0), Vec2::new(0.0, v))
}

fn specific_energy(state: State) -> f64 {
    0.5 * state.velocity.norm_sq() - MU_EARTH / state.position.norm()
}

#[test]
fn prograde_burn_adds_energy_and_retrograde_removes_it() {
    let rocket = presets::atlas();
    let eph = Ephemeris::default();
    let state = parking();
    let e0 = specific_energy(state);
    let cfg = BurnConfig::default();

    let mut up = Vehicle::new(0, rocket.stages[0].propellant_mass(), rocket.liftoff_mass());
    let boosted = finite_burn(&eph, &rocket, &mut up, 0.0, state, prograde(state), 20.0, &cfg)
        .unwrap();
    assert!(specific_energy(boosted.state) > e0);
    assert!(up.mass < rocket.liftoff_mass());
    assert!(boosted.delta_v > 19.0);

    let mut down = Vehicle::new(0, rocket.stages[0].propellant_mass(), rocket.liftoff_mass());
    let lowered = finite_burn(
        &eph,
        &rocket,
        &mut down,
        0.0,
        state,
        retrograde(state),
        20.0,
        &cfg,
    )
    .unwrap();
    assert!(specific_energy(lowered.state) < e0);
    assert!(down.mass < rocket.liftoff_mass());
}
