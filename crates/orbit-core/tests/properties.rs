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
