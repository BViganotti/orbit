//! Seeded property tests for the numerical core.

use orbit_core::math::{Rng, Vec2};

#[test]
fn vector_norm_and_rotation_properties() {
    let mut rng = Rng::new(0x51c7_0001);
    for _ in 0..20_000 {
        let v = Vec2::new(rng.range(-1e6, 1e6), rng.range(-1e6, 1e6));
        assert!(v.norm() >= 0.0);
        assert!((v.norm() - v.norm_sq().sqrt()).abs() <= 1e-9 * v.norm().max(1.0));
        let angle = rng.range(-10.0, 10.0);
        let r = v.rotated(angle);
        assert!((r.norm() - v.norm()).abs() <= 1e-8 * v.norm().max(1.0));
        assert!((r.dot(v) - v.norm_sq() * angle.cos()).abs() <= 1e-7 * v.norm_sq().max(1.0));
        let unit = v.normalized_or_x();
        if v.norm() > 1e-6 {
            assert!((unit.norm() - 1.0).abs() < 1e-12);
        }
        let t = rng.next_f64();
        let w = Vec2::new(rng.range(-1e6, 1e6), rng.range(-1e6, 1e6));
        let lerp = v.lerp(w, t);
        if t == 0.0 {
            assert!((lerp - v).norm() < 1e-9);
        }
        assert!((lerp - v).norm() <= (w - v).norm() + 1e-9);
    }
}

#[test]
fn rk4_convergence_is_fourth_order() {
    fn decay(_t: f64, y: &[f64], out: &mut [f64]) {
        out[0] = -y[0];
    }
    let exact = (-1.0f64).exp();
    let run = |h: f64| {
        let mut y = [1.0];
        orbit_core::math::integrate_rk4(
            &(decay as fn(f64, &[f64], &mut [f64])),
            &mut y,
            0.0,
            1.0,
            h,
            &mut |_, _| {},
        )
        .unwrap();
        (y[0] - exact).abs()
    };
    let coarse = run(0.1);
    let fine = run(0.05);
    let ratio = coarse / fine.max(1e-18);
    assert!(ratio > 8.0 && ratio < 30.0, "ratio {ratio}");
    assert!(fine < 1e-6);
}

#[test]
fn adaptive_integrator_respects_tolerance() {
    fn oscillator(_t: f64, y: &[f64], out: &mut [f64]) {
        out[0] = y[1];
        out[1] = -y[0];
    }
    let dp = orbit_core::math::DormandPrince {
        rtol: 1e-11,
        atol: 1e-12,
        ..orbit_core::math::DormandPrince::default()
    };
    let mut y = [1.0, 0.0];
    dp.propagate(
        &(oscillator as fn(f64, &[f64], &mut [f64])),
        &mut y,
        0.0,
        50.0 * std::f64::consts::TAU,
        0.1,
        &mut |_, _| orbit_core::math::Flow::Continue,
    )
    .unwrap();
    assert!((y[0] - 1.0).abs() < 1e-8, "x={}", y[0]);
    assert!(y[1].abs() < 1e-8, "v={}", y[1]);
}

#[test]
fn root_finders_solve_random_linear_and_quadratic_functions() {
    use orbit_core::math::{bisect, newton, secant};
    let mut rng = Rng::new(0x7207_0001);
    for _ in 0..5_000 {
        let root = rng.range(-100.0, 100.0);
        let slope = rng.range(0.1, 10.0);
        let offset = rng.range(-50.0, 50.0);
        let f = |x: f64| slope * (x - root) + offset;
        if let Some(r) = bisect(f, root - 100.0, root + 100.0, 1e-10, 200) {
            assert!((r.x - root + offset / slope).abs() < 1e-6);
        }
        if let Some(r) = secant(f, root - 5.0, root + 5.0, 1e-10, 50) {
            assert!((r.x - root + offset / slope).abs() < 1e-6);
        }
        if let Some(r) = newton(f, |_| slope, root + 3.0, 1e-10, 20) {
            assert!((r.x - root + offset / slope).abs() < 1e-6);
        }
        // Quadratic with two well-separated real roots; bracket just the first.
        let other = root + rng.range(5.0, 100.0);
        let q = |x: f64| (x - root) * (x - other);
        let r = bisect(q, root - 1.0, root + 1.0, 1e-10, 200).unwrap();
        assert!((r.x - root).abs() < 1e-6);
    }
}

#[test]
fn atmosphere_and_drag_are_bounded_for_random_inputs() {
    use orbit_core::aero::{density, dynamic_pressure, pressure_ratio, speed_of_sound, DragModel};
    let mut rng = Rng::new(0xa3e0_0001);
    let model = DragModel::new(0.3, 10.0);
    for _ in 0..10_000 {
        let h = rng.range(-10_000.0, 2_000_000.0);
        let d = density(h);
        assert!(d.is_finite() && d >= 0.0 && d <= 1.3);
        let p = pressure_ratio(h);
        assert!(p.is_finite() && (0.0..=1.0).contains(&p));
        let speed = rng.range(0.0, 20_000.0);
        let q = dynamic_pressure(h, speed);
        assert!(q.is_finite() && q >= 0.0);
        let a = speed_of_sound(h);
        assert!(a.is_finite() && a >= 0.0 && a < 1000.0);
        let v = Vec2::new(
            rng.range(-20_000.0, 20_000.0),
            rng.range(-20_000.0, 20_000.0),
        );
        let drag = model.acceleration(d, v, rng.range(1.0, 1_000_000.0));
        assert!(drag.is_finite());
        if v.norm() > 1e-6 && d > 0.0 {
            assert!(drag.dot(v) <= 1e-9);
        }
    }
}

#[test]
fn random_rockets_never_panic_validation() {
    use orbit_core::rocket::{Engine, Propellant, Rocket, Stage, Tank};
    let mut rng = Rng::new(0x0b0c_0001);
    for _ in 0..10_000 {
        let engine = Engine::new(
            "E",
            rng.range(-1e5, 5e6),
            rng.range(-1e5, 5e6),
            rng.range(-10.0, 500.0),
            rng.range(-10.0, 500.0),
            rng.range(-10.0, 1e4),
        );
        let mut stage = Stage::new("S", rng.range(-1e3, 1e5), engine, rng.integer(0, 70) as u32);
        stage.reserve_fraction = rng.range(-0.5, 0.8);
        let tank = Tank::new(
            "T",
            rng.range(-1e3, 3e6),
            rng.range(-1e3, 1e5),
            Propellant::new("P", rng.range(-10.0, 4000.0)),
        );
        stage.tanks.push(tank);
        let mut rocket = Rocket::new("R", rng.range(-1e3, 2e6));
        rocket.drag_area = rng.range(-10.0, 2000.0);
        rocket.drag_coefficient = rng.range(-1.0, 10.0);
        rocket.stages.push(stage);
        let result = rocket.validate();
        if result.is_ok() {
            let s = rocket.summary();
            assert!(s.total_delta_v.is_finite() && s.total_delta_v > 0.0);
            assert!(s.liftoff_mass.is_finite() && s.liftoff_mass > 0.0);
        }
    }
}
