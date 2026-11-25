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

#[test]
fn tli_planner_solves_a_real_perilune_with_small_residual() {
    use orbit_flight::plan::plan_tli;

    let rocket = presets::atlas();
    let eph = Ephemeris::default();
    let state = parking();
    let vehicle = Vehicle::new(0, rocket.stages[0].propellant_mass(), rocket.liftoff_mass());
    let plan = plan_tli(&eph, &rocket, &vehicle, 0.0, state, 100_000.0).unwrap();
    assert!(plan.evaluations > 20);
    assert!(plan.burn_delta_v > 2_500.0 && plan.burn_delta_v < 4_000.0);
    assert!(plan.closest.altitude.is_finite());
    assert!((plan.closest.altitude - 100_000.0).abs() < 1_000.0);
    assert!(plan.residual.abs() < 1_000.0);
    assert!(plan.closest.relative_speed > 0.0);
}
