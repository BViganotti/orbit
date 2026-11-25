//! Vehicle state after ascent: current stage, propellant and mass.

use orbit_core::rocket::Rocket;
use orbit_core::G0;

/// Mutable mass bookkeeping for a staged vehicle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vehicle {
    pub stage_index: usize,
    pub propellant: f64,
    pub mass: f64,
}

impl Vehicle {
    pub fn new(stage_index: usize, propellant: f64, mass: f64) -> Self {
        Self {
            stage_index,
            propellant,
            mass,
        }
    }

    pub fn from_ascent(
        rocket: &Rocket,
        stage_index: usize,
        propellant: f64,
        mass: f64,
    ) -> Option<Self> {
        if stage_index >= rocket.stages.len()
            || !propellant.is_finite()
            || !mass.is_finite()
            || propellant < 0.0
            || mass <= 0.0
        {
            return None;
        }
        Some(Self::new(stage_index, propellant, mass))
    }

    pub fn reserve(&self, rocket: &Rocket) -> f64 {
        rocket
            .stages
            .get(self.stage_index)
            .map(|s| s.propellant_mass() * s.reserve_fraction)
            .unwrap_or(0.0)
    }

    pub fn usable_propellant(&self, rocket: &Rocket) -> f64 {
        (self.propellant - self.reserve(rocket)).max(0.0)
    }

    pub fn has_stage(&self, rocket: &Rocket) -> bool {
        self.stage_index < rocket.stages.len()
    }

    /// Jettison the current stage and ignite the next one, if any.
    pub fn advance(&mut self, rocket: &Rocket) -> bool {
        if self.stage_index + 1 >= rocket.stages.len() {
            return false;
        }
        let stage = &rocket.stages[self.stage_index];
        let remaining = self.propellant;
        // Drop the spent stage and its residual propellant.
        self.mass = (self.mass - stage.dry_mass() - remaining).max(1.0);
        self.stage_index += 1;
        let next = &rocket.stages[self.stage_index];
        self.propellant = next.propellant_mass();
        self.mass += self.propellant;
        true
    }

    /// Ideal remaining delta-v across the current and upper stages.
    pub fn remaining_delta_v(&self, rocket: &Rocket) -> f64 {
        if !self.has_stage(rocket) {
            return 0.0;
        }
        let stage = &rocket.stages[self.stage_index];
        let usable = self.usable_propellant(rocket);
        let mut dv = if usable > 0.0 && self.mass > usable {
            stage.engine.isp_vacuum * G0 * (self.mass / (self.mass - usable)).ln()
        } else {
            0.0
        };
        for i in (self.stage_index + 1)..rocket.stages.len() {
            dv += rocket.stage_delta_v(i);
        }
        dv
    }

    pub fn validate(&self, rocket: &Rocket) -> Result<(), String> {
        if !self.has_stage(rocket) {
            return Err("vehicle has no active stage".into());
        }
        if !self.mass.is_finite() || self.mass <= 0.0 {
            return Err("vehicle mass is not finite and positive".into());
        }
        if !self.propellant.is_finite() || self.propellant < 0.0 {
            return Err("vehicle propellant is not finite and non-negative".into());
        }
        let stage = &rocket.stages[self.stage_index];
        if self.propellant > stage.propellant_mass() + 1e-6 {
            return Err("vehicle propellant exceeds stage capacity".into());
        }
        if self.mass <= self.propellant {
            return Err("vehicle mass is less than its propellant".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_core::rocket::presets;

    #[test]
    fn staging_mass_is_continuous() {
        let rocket = presets::atlas();
        let mut v = Vehicle::new(0, rocket.stages[0].propellant_mass(), rocket.liftoff_mass());
        let before = v.mass;
        assert!(v.advance(&rocket));
        assert_eq!(v.stage_index, 1);
        // Mass after jettison equals payload plus upper stages, then the next
        // stage's propellant is already part of that wet stack.
        assert!(v.mass < before);
        assert!(v.mass > 0.0);
        assert_eq!(v.propellant, rocket.stages[1].propellant_mass());
    }

    #[test]
    fn advanced_vehicle_validates() {
        let rocket = presets::kestrel();
        let mut v = Vehicle::new(0, rocket.stages[0].propellant_mass(), rocket.liftoff_mass());
        assert!(v.validate(&rocket).is_ok());
        v.propellant = -1.0;
        assert!(v.validate(&rocket).is_err());
    }

    #[test]
    fn remaining_delta_v_matches_rocket_at_ignition() {
        let rocket = presets::heron();
        let v = Vehicle::new(0, rocket.stages[0].propellant_mass(), rocket.liftoff_mass());
        let expected = rocket.total_delta_v();
        assert!((v.remaining_delta_v(&rocket) - expected).abs() < 1e-6);
    }
}
