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
