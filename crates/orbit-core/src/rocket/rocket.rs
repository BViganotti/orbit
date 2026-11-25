//! The assembled multi-stage rocket.

use crate::rocket::stage::Stage;
use crate::G0;

/// A rocket: a payload, an ordered stack of stages (index 0 fires first) and a
/// single drag reference area.
#[derive(Debug, Clone, PartialEq)]
pub struct Rocket {
    pub name: String,
    pub stages: Vec<Stage>,
    pub payload_mass: f64,
    /// Reference area for drag, m^2.
    pub drag_area: f64,
    /// Drag coefficient in the continuum regime.
    pub drag_coefficient: f64,
}

/// Aggregate numbers shown in the garage and used in mission reports.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RocketSummary {
    pub liftoff_mass: f64,
    pub dry_mass: f64,
    pub propellant_mass: f64,
    pub payload_fraction: f64,
    pub total_delta_v: f64,
    pub liftoff_twr: f64,
    pub vacuum_twr_first_stage: f64,
    pub total_burn_time: f64,
    pub stage_count: usize,
}

impl Rocket {
    pub fn new(name: impl Into<String>, payload_mass: f64) -> Self {
        Self {
            name: name.into(),
            stages: Vec::new(),
            payload_mass,
            drag_area: 12.0,
            drag_coefficient: 0.35,
        }
    }

    pub fn with_stage(mut self, stage: Stage) -> Self {
        self.stages.push(stage);
        self
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    /// Wet mass of every stage above (after) stage `index`.
    pub fn mass_above(&self, index: usize) -> f64 {
        self.payload_mass
            + self
                .stages
                .iter()
                .skip(index + 1)
                .map(|s| s.wet_mass())
                .sum::<f64>()
    }

    pub fn ignition_mass(&self, index: usize) -> f64 {
        self.mass_above(index) + self.stages[index].wet_mass()
    }

    pub fn post_stage_mass(&self, index: usize) -> f64 {
        let stage = &self.stages[index];
        self.mass_above(index) + stage.dry_mass() + stage.propellant_mass() * stage.reserve_fraction
    }

    pub fn liftoff_mass(&self) -> f64 {
        self.payload_mass + self.stages.iter().map(|s| s.wet_mass()).sum::<f64>()
    }

    pub fn dry_mass(&self) -> f64 {
        self.payload_mass + self.stages.iter().map(|s| s.dry_mass()).sum::<f64>()
    }

    pub fn propellant_mass(&self) -> f64 {
        self.stages.iter().map(|s| s.propellant_mass()).sum()
    }

    /// Ideal delta-v summed over stages, using vacuum Isp.
    pub fn total_delta_v(&self) -> f64 {
        self.stages
            .iter()
            .enumerate()
            .map(|(i, s)| s.delta_v(self.mass_above(i)))
            .sum()
    }

    pub fn stage_delta_v(&self, index: usize) -> f64 {
        self.stages
            .get(index)
            .map(|s| s.delta_v(self.mass_above(index)))
            .unwrap_or(0.0)
    }

    pub fn liftoff_twr(&self) -> f64 {
        let mass = self.liftoff_mass();
        if mass <= 0.0 || self.stages.is_empty() {
            0.0
        } else {
            self.stages[0].thrust_sea_level() / (mass * G0)
        }
    }

    pub fn vacuum_twr_first_stage(&self) -> f64 {
        let mass = self.liftoff_mass();
        if mass <= 0.0 || self.stages.is_empty() {
            0.0
        } else {
            self.stages[0].thrust_vacuum() / (mass * G0)
        }
    }

    pub fn total_burn_time(&self) -> f64 {
        self.stages.iter().map(|s| s.burn_time()).sum()
    }

    pub fn summary(&self) -> RocketSummary {
        let liftoff = self.liftoff_mass();
        RocketSummary {
            liftoff_mass: liftoff,
            dry_mass: self.dry_mass(),
            propellant_mass: self.propellant_mass(),
            payload_fraction: if liftoff > 0.0 {
                self.payload_mass / liftoff
            } else {
                0.0
            },
            total_delta_v: self.total_delta_v(),
            liftoff_twr: self.liftoff_twr(),
            vacuum_twr_first_stage: self.vacuum_twr_first_stage(),
            total_burn_time: self.total_burn_time(),
            stage_count: self.stages.len(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() || self.name.len() > 80 {
            return Err("rocket name must be 1..=80 bytes".into());
        }
        if self.stages.is_empty() {
            return Err("rocket has no stages".into());
        }
        if self.stages.len() > 8 {
            return Err("rocket has more than 8 stages".into());
        }
        if !self.payload_mass.is_finite() || self.payload_mass < 0.0 {
            return Err("payload mass must be finite and non-negative".into());
        }
        if self.payload_mass > 1.0e6 {
            return Err("payload mass exceeds 1,000 t".into());
        }
        if !self.drag_area.is_finite() || self.drag_area <= 0.0 || self.drag_area > 1000.0 {
            return Err("drag area must be finite and in (0, 1000] m^2".into());
        }
        if !self.drag_coefficient.is_finite()
            || self.drag_coefficient < 0.02
            || self.drag_coefficient > 5.0
        {
            return Err("drag coefficient must be in [0.02, 5.0]".into());
        }
        for (i, stage) in self.stages.iter().enumerate() {
            stage
                .validate()
                .map_err(|e| format!("stage {}: {}", i + 1, e))?;
        }
        let mass = self.liftoff_mass();
        if !mass.is_finite() || mass <= 0.0 || mass > 1.0e7 {
            return Err("liftoff mass must be finite and in (0, 10,000 t]".into());
        }
        let dv = self.total_delta_v();
        if !dv.is_finite() || dv <= 0.0 || dv > 30_000.0 {
            return Err("total delta-v outside (0, 30,000] m/s".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rocket::engine::Engine;
    use crate::rocket::stage::{Propellant, Tank};

    fn two_stage() -> Rocket {
        let e1 = Engine::new("E1", 2_000_000.0, 1_800_000.0, 290.0, 270.0, 500.0);
        let e2 = Engine::new("E2", 300_000.0, 250_000.0, 350.0, 300.0, 200.0);
        let s1 = Stage::new("S1", 6000.0, e1, 2).with_tank(Tank::new(
            "T1",
            200_000.0,
            4000.0,
            Propellant::rp1_lox(),
        ));
        let s2 = Stage::new("S2", 1500.0, e2, 1).with_tank(Tank::new(
            "T2",
            30_000.0,
            800.0,
            Propellant::hydrogen_lox(),
        ));
        Rocket::new("Test", 2000.0).with_stage(s1).with_stage(s2)
    }

    #[test]
    fn masses_and_staging_are_consistent() {
        let r = two_stage();
        let s1 = &r.stages[0];
        let s2 = &r.stages[1];
        assert_eq!(r.mass_above(0), 2000.0 + s2.wet_mass());
        assert_eq!(r.ignition_mass(0), r.liftoff_mass());
        assert_eq!(r.post_stage_mass(0), 2000.0 + s2.wet_mass() + s1.dry_mass());
        assert!((r.dry_mass() - (2000.0 + s1.dry_mass() + s2.dry_mass())).abs() < 1e-9);
    }

    #[test]
    fn total_delta_v_is_sum_of_stages() {
        let r = two_stage();
        let sum = r.stage_delta_v(0) + r.stage_delta_v(1);
        assert!((r.total_delta_v() - sum).abs() < 1e-9);
        assert!(sum > 9000.0);
    }

    #[test]
    fn summary_fields_are_finite() {
        let s = two_stage().summary();
        assert!(s.liftoff_twr > 1.0);
        assert!(s.payload_fraction > 0.0 && s.payload_fraction < 1.0);
        assert!(s.total_burn_time > 0.0);
        assert_eq!(s.stage_count, 2);
    }

    #[test]
    fn validation_rejects_malformed_rockets() {
        let mut r = two_stage();
        r.stages.clear();
        assert!(r.validate().is_err());
        let mut r = two_stage();
        r.drag_area = 0.0;
        assert!(r.validate().is_err());
        let mut r = two_stage();
        r.drag_coefficient = f64::INFINITY;
        assert!(r.validate().is_err());
        let mut r = two_stage();
        r.name.clear();
        assert!(r.validate().is_err());
        let mut r = two_stage();
        r.payload_mass = -1.0;
        assert!(r.validate().is_err());
    }

    #[test]
    fn stage_delta_v_uses_remaining_stack() {
        let r = two_stage();
        let dv1 = r.stage_delta_v(0);
        let dv2 = r.stage_delta_v(1);
        // Upper stage sees only the payload above it and therefore has a high
        // mass ratio.
        assert!(dv2 > 3000.0);
        assert!(dv1 > 3000.0);
    }
}
