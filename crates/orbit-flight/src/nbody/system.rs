//! Fully coupled four-body system used for conservation diagnostics.
//!
//! Unlike the mission ephemeris, Earth, Moon and Sun are integrated together
//! with mutual gravity, and the spacecraft is a massless test particle. Total
//! energy and barycentric momentum are therefore genuine conserved quantities
//! (up to integration error), which makes this the honest 30-day conservation
//! test for the project.

use orbit_core::constants::{
    AU, MOON_ORBIT_RADIUS, MU_EARTH, MU_MOON, MU_SUN, R_EARTH,
};
use orbit_core::math::{DormandPrince, Flow, Vec2};

const G: f64 = 6.674_30e-11;

/// A body with finite mass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MassiveBody {
    pub mu: f64,
    pub mass: f64,
    pub position: Vec2,
    pub velocity: Vec2,
}

impl MassiveBody {
    fn new(mu: f64, position: Vec2, velocity: Vec2) -> Self {
        Self {
            mu,
            mass: mu / G,
            position,
            velocity,
        }
    }
}

/// The integrated system.
#[derive(Debug, Clone, PartialEq)]
pub struct FullSystem {
    pub sun: MassiveBody,
    pub earth: MassiveBody,
    pub moon: MassiveBody,
    pub spacecraft: Vec2,
    pub spacecraft_velocity: Vec2,
}

impl FullSystem {
    /// Hierarchical circular initial conditions with a zero-momentum frame.
    pub fn initial() -> Self {
        let m_earth = MU_EARTH / G;
        let m_moon = MU_MOON / G;
        let m_earth_moon = m_earth + m_moon;
        let m_sun = MU_SUN / G;

        let r_moon = MOON_ORBIT_RADIUS;
        let v_rel = ((MU_EARTH + MU_MOON) / r_moon).sqrt();
        let earth_rel = Vec2::new(-r_moon * m_moon / m_earth_moon, 0.0);
        let moon_rel = Vec2::new(r_moon * m_earth / m_earth_moon, 0.0);
        let earth_rel_v = Vec2::new(0.0, -v_rel * m_moon / m_earth_moon);
        let moon_rel_v = Vec2::new(0.0, v_rel * m_earth / m_earth_moon);

        let v_helio = ((MU_SUN + MU_EARTH + MU_MOON) / AU).sqrt();
        let emb = Vec2::new(AU, 0.0);
        let emb_v = Vec2::new(0.0, v_helio);
        let sun_velocity = emb_v * (-m_earth_moon / m_sun);

        let earth = MassiveBody::new(MU_EARTH, emb + earth_rel, emb_v + earth_rel_v);
        let moon = MassiveBody::new(MU_MOON, emb + moon_rel, emb_v + moon_rel_v);
        let sun = MassiveBody::new(MU_SUN, Vec2::ZERO, sun_velocity);

        // A spacecraft in a 10,000 km altitude equatorial orbit around Earth.
        let r_sc = R_EARTH + 10_000_000.0;
        let sc_rel = Vec2::new(r_sc, 0.0);
        let sc_v = Vec2::new(0.0, (MU_EARTH / r_sc).sqrt());
        Self {
            sun,
            earth,
            moon,
            spacecraft: earth.position + sc_rel,
            spacecraft_velocity: earth.velocity + sc_v,
        }
    }

    pub fn bodies(&self) -> [MassiveBody; 3] {
        [self.sun, self.earth, self.moon]
    }

    fn set_bodies(&mut self, bodies: [MassiveBody; 3]) {
        self.sun = bodies[0];
        self.earth = bodies[1];
        self.moon = bodies[2];
    }

    fn to_state(&self) -> [f64; 16] {
        [
            self.sun.position.x,
            self.sun.position.y,
            self.sun.velocity.x,
            self.sun.velocity.y,
            self.earth.position.x,
            self.earth.position.y,
            self.earth.velocity.x,
            self.earth.velocity.y,
            self.moon.position.x,
            self.moon.position.y,
            self.moon.velocity.x,
            self.moon.velocity.y,
            self.spacecraft.x,
            self.spacecraft.y,
            self.spacecraft_velocity.x,
            self.spacecraft_velocity.y,
        ]
    }

    fn from_state(y: &[f64]) -> Option<Self> {
        if y.len() < 16 || y.iter().any(|v| !v.is_finite()) {
            return None;
        }
        let mut s = Self::initial();
        let mut bodies = s.bodies();
        for (i, body) in bodies.iter_mut().enumerate() {
            body.position = Vec2::new(y[i * 4], y[i * 4 + 1]);
            body.velocity = Vec2::new(y[i * 4 + 2], y[i * 4 + 3]);
        }
        s.set_bodies(bodies);
        s.spacecraft = Vec2::new(y[12], y[13]);
        s.spacecraft_velocity = Vec2::new(y[14], y[15]);
        Some(s)
    }

    fn derivative(y: &[f64], out: &mut [f64]) {
        let mut pos = [Vec2::ZERO; 4];
        let mut vel = [Vec2::ZERO; 4];
        for i in 0..4 {
            pos[i] = Vec2::new(y[i * 4], y[i * 4 + 1]);
            vel[i] = Vec2::new(y[i * 4 + 2], y[i * 4 + 3]);
        }
        let mus = [MU_SUN, MU_EARTH, MU_MOON, 0.0];
        let mut acc = [Vec2::ZERO; 4];
        for i in 0..4 {
            let mut a = Vec2::ZERO;
            for j in 0..3 {
                if i == j {
                    continue;
                }
                let d = pos[j] - pos[i];
                let r2 = d.norm_sq();
                if r2 > 1.0 {
                    a += d * (mus[j] / (r2 * r2.sqrt()));
                }
            }
            acc[i] = a;
        }
        for i in 0..4 {
            out[i * 4] = vel[i].x;
            out[i * 4 + 1] = vel[i].y;
            out[i * 4 + 2] = acc[i].x;
            out[i * 4 + 3] = acc[i].y;
        }
    }

    /// Total energy in joules (spacecraft treated as a unit test mass).
    pub fn energy(&self) -> f64 {
        let bodies = self.bodies();
        let mut energy = 0.0;
        for body in &bodies {
            energy += 0.5 * body.mass * body.velocity.norm_sq();
        }
        for i in 0..3 {
            for j in (i + 1)..3 {
                let d = (bodies[j].position - bodies[i].position).norm();
                if d > 0.0 {
                    energy -= G * bodies[i].mass * bodies[j].mass / d;
                }
            }
        }
        energy += 0.5 * self.spacecraft_velocity.norm_sq();
        for body in &bodies {
            let d = (self.spacecraft - body.position).norm();
            if d > 0.0 {
                energy -= body.mu / d;
            }
        }
        energy
    }

    /// Barycentric momentum of the massive bodies.
    pub fn momentum(&self) -> Vec2 {
        let mut p = Vec2::ZERO;
        for body in self.bodies() {
            p += body.velocity * body.mass;
        }
        p
    }

    /// Propagate the coupled system for `seconds`.
    pub fn propagate(&mut self, seconds: f64) -> Result<u32, String> {
        if !seconds.is_finite() || seconds <= 0.0 {
            return Err("invalid propagation duration".into());
        }
        let mut y = self.to_state();
        let controller = DormandPrince {
            rtol: 1.0e-12,
            atol: 1.0e-7,
            h_min: 1.0e-3,
            h_max: 3_600.0,
            max_steps: 2_000_000,
        };
        let report = controller
            .propagate(
                &|_t: f64, state: &[f64], out: &mut [f64]| Self::derivative(state, out),
                &mut y,
                0.0,
                seconds,
                60.0,
                &mut |_, _| Flow::Continue,
            )
            .map_err(|e| e.to_string())?;
        let next = Self::from_state(&y).ok_or_else(|| "non-finite state".to_string())?;
        *self = next;
        Ok(report.steps)
    }

    /// Closest distance from the spacecraft to a body.
    pub fn distance_to(&self, body: &MassiveBody) -> f64 {
        (self.spacecraft - body.position).norm()
    }
}

/// Relative energy drift over a 30-day coupled coast.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConservationReport {
    pub days: f64,
    pub energy_before: f64,
    pub energy_after: f64,
    pub relative_energy_error: f64,
    pub momentum_before: Vec2,
    pub momentum_after: Vec2,
    pub relative_momentum_error: f64,
    pub steps: u32,
}

/// Run the 30-day conservation experiment.
pub fn conservation_30_days() -> Result<ConservationReport, String> {
    let mut system = FullSystem::initial();
    let energy_before = system.energy();
    let momentum_before = system.momentum();
    let days = 30.0;
    let steps = system.propagate(days * 86_400.0)?;
    let energy_after = system.energy();
    let momentum_after = system.momentum();
    let scale = energy_before.abs().max(1.0);
    let pscale = system
        .bodies()
        .iter()
        .map(|b| b.mass * b.velocity.norm())
        .sum::<f64>()
        .max(1.0);
    Ok(ConservationReport {
        days,
        energy_before,
        energy_after,
        relative_energy_error: (energy_after - energy_before).abs() / scale,
        momentum_before,
        momentum_after,
        relative_momentum_error: (momentum_after - momentum_before).norm() / pscale,
        steps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_energy_is_finite_and_negative() {
        let s = FullSystem::initial();
        let e = s.energy();
        assert!(e.is_finite());
        assert!(e < 0.0);
        assert!(s.momentum().norm() < 1.0e22);
    }

    #[test]
    fn short_propagation_stays_finite() {
        let mut s = FullSystem::initial();
        let steps = s.propagate(0.5 * 86_400.0).unwrap();
        assert!(steps > 10);
        assert!(s.energy().is_finite());
        assert!(s.spacecraft.is_finite());
    }

    #[test]
    fn thirty_days_conserve_energy_to_one_part_per_million() {
        let report = conservation_30_days().unwrap();
        assert!(
            report.relative_energy_error < 1.0e-6,
            "energy drift {:.3e}",
            report.relative_energy_error
        );
        assert!(
            report.relative_momentum_error < 1.0e-9,
            "momentum drift {:.3e}",
            report.relative_momentum_error
        );
        assert!(report.steps > 100);
    }
}
