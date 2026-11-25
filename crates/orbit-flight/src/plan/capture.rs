//! Lunar orbit insertion.
//!
//! At perilune the planner burns to match the local circular velocity about the
//! Moon. Because burns are finite the result is corrected iteratively, and the
//! residual eccentricity is reported rather than hidden.

use orbit_core::constants::{MU_MOON, R_MOON};
use orbit_core::math::Vec2;
use orbit_core::rocket::Rocket;

use crate::nbody::gravity::{elements, Elements};
use crate::nbody::propagator::{coast, CoastConfig, State};
use crate::nbody::Ephemeris;

use super::burn::{finite_burn, BurnConfig, BurnStatus};
use super::vehicle::Vehicle;

/// Result of lunar orbit insertion and correction.
#[derive(Debug, Clone, PartialEq)]
pub struct LunarOrbit {
    pub state: State,
    pub t: f64,
    pub elements: Elements,
    pub periapsis_altitude: f64,
    pub apoapsis_altitude: f64,
    pub spent_delta_v: f64,
    pub iterations: u32,
    pub status: BurnStatus,
    /// Periapsis altitude of the osculating lunar orbit at entry.
    pub entry_periapsis_altitude: f64,
    /// Apoapsis altitude of the osculating lunar orbit at entry.
    pub entry_apoapsis_altitude: f64,
}

/// Circularise about the Moon at the current perilune.
///
/// A finite insertion burn changes the radius slightly, so the routine repeats
/// the insertion from the entry state with a small aim correction and returns
/// the best result actually achieved.
pub fn insert_lunar_orbit(
    eph: &Ephemeris,
    rocket: &Rocket,
    vehicle: &mut Vehicle,
    t_entry: f64,
    state_entry: State,
    target_altitude: f64,
    config: &BurnConfig,
) -> Result<LunarOrbit, String> {
    eph.validate()?;
    rocket.validate()?;
    vehicle.validate(rocket)?;
    if !state_entry.is_finite() || !t_entry.is_finite() {
        return Err("invalid lunar insertion state".into());
    }
    if !target_altitude.is_finite() || target_altitude <= 0.0 {
        return Err("invalid lunar target altitude".into());
    }
    let entry_vehicle = *vehicle;
    let entry_moon = eph.state(crate::nbody::Body::Moon, t_entry);
    let entry_rel = state_entry.position - entry_moon.position;
    let entry_rel_v = state_entry.velocity - entry_moon.velocity;
    let entry_elements = elements(entry_rel, entry_rel_v, MU_MOON);
    let mut aim_offset = 0.0f64;
    let mut best: Option<(LunarOrbit, Vehicle, f64)> = None;
    for _attempt in 0..4 {
        let mut state = state_entry;
        let mut t = t_entry;
        let mut v = entry_vehicle;
        let mut spent = 0.0;
        let mut iterations = 0u32;
        let mut status = BurnStatus::Complete;
        for _ in 0..6 {
            iterations += 1;
            let moon = eph.state(crate::nbody::Body::Moon, t);
            let rel = state.position - moon.position;
            let rel_v = state.velocity - moon.velocity;
            let r = rel.norm();
            if !r.is_finite() || r <= R_MOON {
                return Err("lunar insertion state is inside the Moon".into());
            }
            let aim_radius = (r + aim_offset).max(R_MOON + 1_000.0);
            let desired = rel.normalized_or_x().perp() * (MU_MOON / aim_radius).sqrt();
            let delta = desired - rel_v;
            let dv = delta.norm();
            if !dv.is_finite() {
                return Err("lunar insertion delta-v is not finite".into());
            }
            if dv < 0.02 {
                break;
            }
            let outcome = finite_burn(eph, rocket, &mut v, t, state, delta / dv, dv, config)?;
            state = outcome.state;
            t = outcome.t;
            spent += outcome.delta_v;
            status = outcome.status;
            if !matches!(status, BurnStatus::Complete) {
                break;
            }
        }
        let moon = eph.state(crate::nbody::Body::Moon, t);
        let rel = state.position - moon.position;
        let rel_v = state.velocity - moon.velocity;
        let el = match elements(rel, rel_v, MU_MOON) {
            Some(e) => e,
            None => return Err("could not compute lunar elements".into()),
        };
        let peri = el.periapsis - R_MOON;
        let apo = el.apoapsis - R_MOON;
        let score = (peri - target_altitude).abs() + (apo - target_altitude).abs();
        let outcome = LunarOrbit {
            state,
            t,
            elements: el,
            periapsis_altitude: peri,
            apoapsis_altitude: apo,
            spent_delta_v: spent,
            iterations,
            status,
            entry_periapsis_altitude: entry_elements
                .map(|e| e.periapsis - R_MOON)
                .unwrap_or(f64::NAN),
            entry_apoapsis_altitude: entry_elements
                .map(|e| e.apoapsis - R_MOON)
                .unwrap_or(f64::NAN),
        };
        let better = best.as_ref().map_or(true, |(_, _, s)| score < *s);
        if better {
            best = Some((outcome, v, score));
        }
        if score <= 0.002 * target_altitude.max(1.0) {
            break;
        }
        let mean = 0.5 * (peri + apo);
        // If the achieved orbit is too high, aim lower on the next attempt.
        aim_offset -= mean - target_altitude;
        if aim_offset.abs() > 0.2 * target_altitude {
            break;
        }
    }
    match best {
        Some((outcome, v, _)) => {
            *vehicle = v;
            match adjust_lunar_altitude(
                eph,
                rocket,
                vehicle,
                outcome.t,
                outcome.state,
                target_altitude,
                config,
            ) {
                Ok(mut adjusted) => {
                    adjusted.spent_delta_v += outcome.spent_delta_v;
                    adjusted.iterations += outcome.iterations;
                    adjusted.entry_periapsis_altitude = outcome.entry_periapsis_altitude;
                    adjusted.entry_apoapsis_altitude = outcome.entry_apoapsis_altitude;
                    Ok(adjusted)
                }
                Err(_) => Ok(outcome),
            }
        }
        None => Err("lunar insertion produced no result".into()),
    }
}

/// Fine-tune a near-circular lunar orbit to the requested altitude using a
/// small two-impulse transfer. This removes the systematic radius bias of a
/// finite insertion burn without pretending the burn was impulsive.
fn adjust_lunar_altitude(
    eph: &Ephemeris,
    rocket: &Rocket,
    vehicle: &mut Vehicle,
    t_entry: f64,
    state_entry: State,
    target_altitude: f64,
    config: &BurnConfig,
) -> Result<LunarOrbit, String> {
    let target_radius = R_MOON + target_altitude;
    let entry_moon = eph.state(crate::nbody::Body::Moon, t_entry);
    let entry_rel = state_entry.position - entry_moon.position;
    let entry_rel_v = state_entry.velocity - entry_moon.velocity;
    let entry_elements = elements(entry_rel, entry_rel_v, MU_MOON);
    let mut t = t_entry;
    let mut state = state_entry;
    let mut spent = 0.0;
    let mut iterations = 0u32;
    let mut status = BurnStatus::Complete;
    for _ in 0..3 {
        iterations += 1;
        let moon = eph.state(crate::nbody::Body::Moon, t);
        let rel = state.position - moon.position;
        let rel_v = state.velocity - moon.velocity;
        let r1 = rel.norm();
        let elements_now = match elements(rel, rel_v, MU_MOON) {
            Some(e) => e,
            None => return Err("could not compute lunar elements".into()),
        };
        let peri_err = elements_now.periapsis - target_radius;
        let apo_err = elements_now.apoapsis - target_radius;
        if peri_err.abs() <= 0.001 * target_altitude.max(1.0)
            && apo_err.abs() <= 0.001 * target_altitude.max(1.0)
        {
            break;
        }
        if elements_now.eccentricity > 0.05 {
            break;
        }
        let a = 0.5 * (r1 + target_radius);
        if !a.is_finite() || a <= R_MOON {
            break;
        }
        let v_circ1 = (MU_MOON / r1).sqrt();
        let v_trans1 = (MU_MOON * (2.0 / r1 - 1.0 / a)).sqrt();
        let dv1 = v_trans1 - v_circ1;
        if dv1.abs() > 0.001 {
            let dir = if dv1 >= 0.0 {
                rel.normalized_or_x().perp()
            } else {
                -rel.normalized_or_x().perp()
            };
            let out = finite_burn(eph, rocket, vehicle, t, state, dir, dv1.abs(), config)?;
            state = out.state;
            t = out.t;
            spent += out.delta_v;
            status = out.status;
            if !matches!(status, BurnStatus::Complete) {
                break;
            }
        }
        // Coast half a transfer period to the opposite apsis.
        let half = std::f64::consts::PI * (a * a * a / MU_MOON).sqrt();
        let coasted = coast(
            eph,
            state,
            t,
            t + half,
            &CoastConfig {
                sample_interval: half.max(1.0),
                max_step: 90.0,
                rtol: 1.0e-10,
                atol: 1.0e-4,
                ..CoastConfig::default()
            },
        )
        .map_err(|e| e.to_string())?;
        state = coasted.state;
        t = coasted.t;
        let moon = eph.state(crate::nbody::Body::Moon, t);
        let rel = state.position - moon.position;
        let v_circ2 = (MU_MOON / target_radius).sqrt();
        let v_trans2 = (MU_MOON * (2.0 / target_radius - 1.0 / a)).sqrt();
        let dv2 = v_circ2 - v_trans2;
        if dv2.abs() > 0.001 {
            let dir = if dv2 >= 0.0 {
                rel.normalized_or_x().perp()
            } else {
                -rel.normalized_or_x().perp()
            };
            let out = finite_burn(eph, rocket, vehicle, t, state, dir, dv2.abs(), config)?;
            state = out.state;
            t = out.t;
            spent += out.delta_v;
            status = out.status;
            if !matches!(status, BurnStatus::Complete) {
                break;
            }
        }
    }
    let moon = eph.state(crate::nbody::Body::Moon, t);
    let rel = state.position - moon.position;
    let rel_v = state.velocity - moon.velocity;
    let final_elements = elements(rel, rel_v, MU_MOON)
        .ok_or_else(|| String::from("could not compute lunar elements"))?;
    Ok(LunarOrbit {
        state,
        t,
        elements: final_elements,
        periapsis_altitude: final_elements.periapsis - R_MOON,
        apoapsis_altitude: final_elements.apoapsis - R_MOON,
        spent_delta_v: spent,
        iterations,
        status,
        entry_periapsis_altitude: entry_elements
            .map(|e| e.periapsis - R_MOON)
            .unwrap_or(f64::NAN),
        entry_apoapsis_altitude: entry_elements
            .map(|e| e.apoapsis - R_MOON)
            .unwrap_or(f64::NAN),
    })
}

/// Desired circular velocity vector about the Moon.
pub fn lunar_circular_velocity(state: State, moon_position: Vec2, moon_velocity: Vec2) -> Vec2 {
    let rel = state.position - moon_position;
    let r = rel.norm();
    if r <= 0.0 || !r.is_finite() {
        return Vec2::ZERO;
    }
    rel.normalized_or_x().perp() * (MU_MOON / r).sqrt() + moon_velocity
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nbody::Body;
    use orbit_core::constants::MOON_ORBIT_RADIUS;
    use orbit_core::rocket::presets;

    fn near_moon_state(alt: f64, factor: f64) -> (State, f64) {
        let eph = Ephemeris::default();
        let t = 0.0;
        let moon = eph.state(Body::Moon, t);
        let r = R_MOON + alt;
        let rel = Vec2::new(r, 0.0);
        let v = (MU_MOON / r).sqrt() * factor;
        (
            State::new(moon.position + rel, moon.velocity + Vec2::new(0.0, v)),
            t,
        )
    }

    #[test]
    fn circular_insertion_needs_little_delta_v() {
        let eph = Ephemeris::default();
        let rocket = presets::atlas();
        let (state, t) = near_moon_state(100_000.0, 1.0);
        let mut v = Vehicle::new(2, rocket.stages[2].propellant_mass(), 20_000.0);
        let out = insert_lunar_orbit(
            &eph,
            &rocket,
            &mut v,
            t,
            state,
            100_000.0,
            &BurnConfig::default(),
        )
        .unwrap();
        assert!(out.spent_delta_v < 20.0);
        assert!(out.periapsis_altitude > 90_000.0);
        assert!(out.apoapsis_altitude < 400_000.0);
        assert_eq!(out.status, BurnStatus::Complete);
    }

    #[test]
    fn insertion_corrects_elliptical_state() {
        let eph = Ephemeris::default();
        let rocket = presets::atlas();
        let (state, t) = near_moon_state(100_000.0, 0.8);
        let mut v = Vehicle::new(2, rocket.stages[2].propellant_mass(), 20_000.0);
        let out = insert_lunar_orbit(
            &eph,
            &rocket,
            &mut v,
            t,
            state,
            100_000.0,
            &BurnConfig::default(),
        )
        .unwrap();
        assert!(out.spent_delta_v > 100.0);
        assert!(out.elements.eccentricity < 0.05);
        assert!(out.t > 0.0);
        assert!(MOON_ORBIT_RADIUS > R_MOON);
    }

    #[test]
    fn insertion_trims_to_a_higher_target_altitude() {
        let eph = Ephemeris::default();
        let rocket = presets::atlas();
        let (state, t) = near_moon_state(150_000.0, 0.92);
        let mut v = Vehicle::new(2, rocket.stages[2].propellant_mass(), 20_000.0);
        let out = insert_lunar_orbit(
            &eph,
            &rocket,
            &mut v,
            t,
            state,
            150_000.0,
            &BurnConfig::default(),
        )
        .unwrap();
        assert!((out.periapsis_altitude - 150_000.0).abs() < 1_500.0);
        assert!((out.apoapsis_altitude - 150_000.0).abs() < 1_500.0);
        assert!(out.elements.eccentricity < 0.002);
    }

    #[test]
    fn circular_velocity_is_tangential() {
        let eph = Ephemeris::default();
        let moon = eph.state(Body::Moon, 0.0);
        let r = R_MOON + 100_000.0;
        let s = State::new(moon.position + Vec2::new(r, 0.0), moon.velocity);
        let v = lunar_circular_velocity(s, moon.position, moon.velocity);
        assert!(v.is_finite());
        let rel = v - moon.velocity;
        assert!((rel.norm() - (MU_MOON / r).sqrt()).abs() < 1e-9);
        assert!(rel.dot(Vec2::new(1.0, 0.0)).abs() < 1e-6);
    }
}
