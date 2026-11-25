//! A compact layered exponential atmosphere.
//!
//! The model is intentionally simple and smooth. Density anchors are
//! interpolated logarithmically in altitude, which avoids the discontinuities
//! of a hard layer table while remaining monotone and cheap.

use crate::constants::{P_SEA_LEVEL, RHO_SEA_LEVEL};

/// Highest altitude with any atmosphere in this model, m.
pub const ATMOSPHERE_TOP: f64 = 1.0e6;

struct Anchor {
    altitude: f64,
    density: f64,
}

const ANCHORS: [Anchor; 14] = [
    Anchor { altitude: 0.0, density: 1.225 },
    Anchor { altitude: 11_000.0, density: 0.363_9 },
    Anchor { altitude: 20_000.0, density: 0.088_0 },
    Anchor { altitude: 32_000.0, density: 0.013_2 },
    Anchor { altitude: 47_000.0, density: 0.001_43 },
    Anchor { altitude: 51_000.0, density: 0.000_861 },
    Anchor { altitude: 71_000.0, density: 0.000_064 },
    Anchor { altitude: 86_000.0, density: 0.000_006_9 },
    Anchor { altitude: 100_000.0, density: 0.000_000_56 },
    Anchor { altitude: 150_000.0, density: 0.000_000_002_1 },
    Anchor { altitude: 200_000.0, density: 0.000_000_000_26 },
    Anchor { altitude: 300_000.0, density: 0.000_000_000_019 },
    Anchor { altitude: 500_000.0, density: 0.000_000_000_000_001_4 },
    Anchor { altitude: 1_000_000.0, density: 1.0e-18 },
];

/// Atmospheric density in kg/m^3 at geometric altitude `h` metres.
///
/// Negative altitudes are treated as sea level; altitudes above
/// [`ATMOSPHERE_TOP`] return zero. Non-finite input returns zero.
pub fn density(h: f64) -> f64 {
    if !h.is_finite() || h >= ATMOSPHERE_TOP {
        return 0.0;
    }
    let h = h.max(0.0);
    if h <= 0.0 {
        return RHO_SEA_LEVEL;
    }
    for pair in ANCHORS.windows(2) {
        let a = &pair[0];
        let b = &pair[1];
        if h <= b.altitude {
            let span = b.altitude - a.altitude;
            if span <= 0.0 {
                return b.density;
            }
            let frac = (h - a.altitude) / span;
            let ln_rho = a.density.ln() + (b.density.ln() - a.density.ln()) * frac;
            return ln_rho.exp().max(0.0);
        }
    }
    0.0
}

/// Atmospheric pressure in Pa, approximated with an isothermal scale height
/// anchored at sea level. Used only for engine thrust interpolation.
pub fn pressure(h: f64) -> f64 {
    if !h.is_finite() || h >= ATMOSPHERE_TOP {
        return 0.0;
    }
    let h = h.max(0.0);
    // Fit an effective scale height that matches the density table: use the
    // density ratio directly, which keeps thrust consistent with drag.
    let ratio = (density(h) / RHO_SEA_LEVEL).clamp(0.0, 1.0);
    P_SEA_LEVEL * ratio
}

/// Ambient pressure divided by sea-level pressure, clamped to `[0, 1]`.
pub fn pressure_ratio(h: f64) -> f64 {
    (pressure(h) / P_SEA_LEVEL).clamp(0.0, 1.0)
}

/// Speed of sound in m/s using a two-layer temperature approximation.
pub fn speed_of_sound(h: f64) -> f64 {
    if !h.is_finite() || h > ATMOSPHERE_TOP {
        return 0.0;
    }
    let t = if h < 11_000.0 {
        288.15 - 0.0065 * h.max(0.0)
    } else if h < 25_000.0 {
        216.65
    } else if h < 50_000.0 {
        216.65 + 0.0028 * (h - 25_000.0)
    } else {
        270.0
    };
    let t = t.max(180.0);
    (1.4 * 287.05 * t).sqrt()
}

/// Dynamic pressure `q = 0.5 * rho * v^2` in Pa.
pub fn dynamic_pressure(h: f64, speed: f64) -> f64 {
    if !speed.is_finite() {
        return 0.0;
    }
    0.5 * density(h) * speed * speed
}

/// Scale height at altitude `h`, derived from the local density gradient.
pub fn scale_height(h: f64) -> f64 {
    let d = density(h);
    if d <= 0.0 {
        return f64::INFINITY;
    }
    let dh = 100.0;
    let d2 = density(h + dh);
    let ratio = (d2 / d).max(1.0e-12);
    (-dh / ratio.ln()).clamp(1_000.0, 100_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sea_level_density_is_standard() {
        assert!((density(0.0) - 1.225).abs() < 1e-12);
        assert_eq!(density(-1000.0), 1.225);
        assert_eq!(density(2_000_000.0), 0.0);
    }

    #[test]
    fn density_decreases_monotonically() {
        let mut previous = density(0.0);
        let mut h = 0.0;
        while h < ATMOSPHERE_TOP {
            h += 1000.0;
            let d = density(h);
            assert!(d <= previous + 1e-18, "h={} d={} prev={}", h, d, previous);
            previous = d;
        }
    }

    #[test]
    fn pressure_ratio_is_bounded() {
        for h in [-1.0, 0.0, 5000.0, 50_000.0, 200_000.0, 2_000_000.0, f64::NAN] {
            let p = pressure_ratio(h);
            assert!(p.is_finite() && (0.0..=1.0).contains(&p));
        }
    }

    #[test]
    fn dynamic_pressure_matches_formula() {
        let q = dynamic_pressure(0.0, 100.0);
        assert!((q - 0.5 * 1.225 * 10_000.0).abs() < 1e-9);
        assert_eq!(dynamic_pressure(0.0, f64::NAN), 0.0);
    }

    #[test]
    fn speed_of_sound_is_plausible() {
        assert!((speed_of_sound(0.0) - 340.3).abs() < 1.0);
        assert!(speed_of_sound(11_000.0) < 320.0);
        assert_eq!(speed_of_sound(2_000_000.0), 0.0);
    }

    #[test]
    fn scale_height_is_positive() {
        assert!(scale_height(10_000.0) > 1000.0);
        assert_eq!(scale_height(2_000_000.0), f64::INFINITY);
    }
}
