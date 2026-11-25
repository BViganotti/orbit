//! Tanks, propellants and stages.

use crate::rocket::engine::Engine;
use crate::G0;

/// A propellant with a bulk density used for tank volume checks.
#[derive(Debug, Clone, PartialEq)]
pub struct Propellant {
    pub name: String,
    /// Bulk density including tankage allowance, kg/m^3.
    pub density: f64,
}

impl Propellant {
    pub fn new(name: impl Into<String>, density: f64) -> Self {
        Self {
            name: name.into(),
            density,
        }
    }

    pub fn rp1_lox() -> Self {
        Self::new("RP-1/LOX", 1020.0)
    }

    pub fn methane_lox() -> Self {
        Self::new("CH4/LOX", 830.0)
    }

    pub fn hydrogen_lox() -> Self {
        Self::new("LH2/LOX", 363.0)
    }

    pub fn hypergolic() -> Self {
        Self::new("N2O4/MMH", 1190.0)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() || self.name.len() > 64 {
            return Err("invalid propellant name".into());
        }
        if !self.density.is_finite() || self.density <= 0.0 || self.density > 3000.0 {
            return Err("propellant density must be finite and in (0, 3000] kg/m^3".into());
        }
        Ok(())
    }
}

/// A propellant tank. `dry_mass` is tank structure and insulation.
#[derive(Debug, Clone, PartialEq)]
pub struct Tank {
    pub name: String,
    pub propellant_mass: f64,
    pub dry_mass: f64,
    pub propellant: Propellant,
}

impl Tank {
    pub fn new(
        name: impl Into<String>,
        propellant_mass: f64,
        dry_mass: f64,
        propellant: Propellant,
    ) -> Self {
        Self {
            name: name.into(),
            propellant_mass,
            dry_mass,
            propellant,
        }
    }

    pub fn wet_mass(&self) -> f64 {
        self.propellant_mass + self.dry_mass
    }

    pub fn volume(&self) -> f64 {
        if self.propellant.density > 0.0 {
            self.propellant_mass / self.propellant.density
        } else {
            f64::INFINITY
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        self.propellant.validate()?;
        if self.name.trim().is_empty() || self.name.len() > 64 {
            return Err("invalid tank name".into());
        }
        if !self.propellant_mass.is_finite()
            || !self.dry_mass.is_finite()
            || self.propellant_mass <= 0.0
            || self.dry_mass < 0.0
        {
            return Err("tank masses must be finite, propellant positive, dry non-negative".into());
        }
        if self.propellant_mass > 2.0e6 {
            return Err("single tank propellant mass exceeds 2,000 t".into());
        }
        if self.volume() > 5000.0 {
            return Err("single tank volume exceeds 5000 m^3".into());
        }
        Ok(())
    }
}

/// A single stack stage: structure, tanks and a cluster of identical engines.
#[derive(Debug, Clone, PartialEq)]
pub struct Stage {
    pub name: String,
    /// Structure excluding tanks and engines, kg.
    pub structure_mass: f64,
    pub tanks: Vec<Tank>,
    pub engine: Engine,
    pub engine_count: u32,
    /// Fraction of propellant that must remain at separation (0..=1).
    pub reserve_fraction: f64,
}

impl Stage {
    pub fn new(
        name: impl Into<String>,
        structure_mass: f64,
        engine: Engine,
        engine_count: u32,
    ) -> Self {
        Self {
            name: name.into(),
            structure_mass,
            tanks: Vec::new(),
            engine,
            engine_count,
            reserve_fraction: 0.0,
        }
    }

    pub fn with_tank(mut self, tank: Tank) -> Self {
        self.tanks.push(tank);
        self
    }

    pub fn propellant_mass(&self) -> f64 {
        self.tanks.iter().map(|t| t.propellant_mass).sum()
    }

    pub fn usable_propellant(&self) -> f64 {
        self.propellant_mass() * (1.0 - self.reserve_fraction)
    }

    pub fn tank_dry_mass(&self) -> f64 {
        self.tanks.iter().map(|t| t.dry_mass).sum()
    }

    pub fn engine_dry_mass(&self) -> f64 {
        self.engine.mass * f64::from(self.engine_count)
    }

    /// Structural mass without propellant.
    pub fn dry_mass(&self) -> f64 {
        self.structure_mass + self.tank_dry_mass() + self.engine_dry_mass()
    }

    pub fn wet_mass(&self) -> f64 {
        self.dry_mass() + self.propellant_mass()
    }

    pub fn thrust_vacuum(&self) -> f64 {
        self.engine.thrust_vacuum * f64::from(self.engine_count)
    }

    pub fn thrust_sea_level(&self) -> f64 {
        self.engine.thrust_sea_level * f64::from(self.engine_count)
    }

    pub fn vacuum_mass_flow(&self) -> f64 {
        self.engine.vacuum_mass_flow() * f64::from(self.engine_count)
    }

    /// Full-throttle vacuum burn time of the usable propellant, seconds.
    pub fn burn_time(&self) -> f64 {
        let mdot = self.vacuum_mass_flow();
        if mdot > 0.0 {
            self.usable_propellant() / mdot
        } else {
            f64::INFINITY
        }
    }

    /// Ideal delta-v for this stage given the mass above it at ignition.
    ///
    /// `mass_above` is the payload plus all upper stages, wet.
    pub fn delta_v(&self, mass_above: f64) -> f64 {
        let m0 = mass_above + self.wet_mass();
        let mf = mass_above + self.dry_mass() + self.propellant_mass() * self.reserve_fraction;
        if m0 <= 0.0 || mf <= 0.0 || mf >= m0 {
            return 0.0;
        }
        self.engine.isp_vacuum * G0 * (m0 / mf).ln()
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() || self.name.len() > 64 {
            return Err("invalid stage name".into());
        }
        if !self.structure_mass.is_finite() || self.structure_mass < 0.0 {
            return Err("stage structure mass must be finite and non-negative".into());
        }
        if self.engine_count == 0 || self.engine_count > 64 {
            return Err("stage engine count must be in [1, 64]".into());
        }
        if !self.reserve_fraction.is_finite() || !(0.0..0.5).contains(&self.reserve_fraction) {
            return Err("reserve fraction must be in [0, 0.5)".into());
        }
        self.engine.validate()?;
        if self.tanks.is_empty() {
            return Err("stage has no tanks".into());
        }
        if self.tanks.len() > 16 {
            return Err("stage has more than 16 tanks".into());
        }
        for tank in &self.tanks {
            tank.validate()?;
        }
        let prop = self.propellant_mass();
        if !prop.is_finite() || prop <= 0.0 {
            return Err("stage propellant mass must be positive".into());
        }
        if self.dry_mass() <= 0.0 {
            return Err("stage dry mass must be positive".into());
        }
        if self.wet_mass() > 5.0e6 {
            return Err("stage wet mass exceeds 5,000 t".into());
        }
        let twr = self.thrust_sea_level() / (self.wet_mass() * G0);
        if !twr.is_finite() {
            return Err("stage thrust-to-weight is not finite".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_stage() -> Stage {
        let engine = Engine::new("T", 1_000_000.0, 900_000.0, 300.0, 280.0, 500.0);
        Stage::new("S1", 5000.0, engine, 1)
            .with_tank(Tank::new("Tank", 100_000.0, 2000.0, Propellant::rp1_lox()))
    }

    #[test]
    fn masses_add_up() {
        let s = test_stage();
        assert_eq!(s.propellant_mass(), 100_000.0);
        assert_eq!(s.tank_dry_mass(), 2000.0);
        assert_eq!(s.engine_dry_mass(), 500.0);
        assert_eq!(s.dry_mass(), 7500.0);
        assert_eq!(s.wet_mass(), 107_500.0);
    }

    #[test]
    fn tsiolkovsky_matches_closed_form() {
        let s = test_stage();
        let dv = s.delta_v(5000.0);
        let expected = 300.0 * G0 * ((5000.0_f64 + 107_500.0) / (5000.0 + 7500.0)).ln();
        assert!((dv - expected).abs() < 1e-9);
    }

    #[test]
    fn burn_time_and_flow_are_consistent() {
        let s = test_stage();
        let mdot = s.vacuum_mass_flow();
        assert!((mdot - 1_000_000.0 / (300.0 * G0)).abs() < 1e-9);
        assert!((s.burn_time() * mdot - 100_000.0).abs() < 1e-6);
    }

    #[test]
    fn reserve_fraction_reduces_usable_propellant() {
        let mut s = test_stage();
        s.reserve_fraction = 0.1;
        assert!((s.usable_propellant() - 90_000.0).abs() < 1e-9);
        assert!(s.delta_v(5000.0) < test_stage().delta_v(5000.0));
    }

    #[test]
    fn validation_rejects_malformed_stage() {
        let mut s = test_stage();
        s.engine_count = 0;
        assert!(s.validate().is_err());
        let mut s = test_stage();
        s.tanks.clear();
        assert!(s.validate().is_err());
        let mut s = test_stage();
        s.structure_mass = f64::NAN;
        assert!(s.validate().is_err());
        let mut s = test_stage();
        s.tanks[0].propellant_mass = -1.0;
        assert!(s.validate().is_err());
    }

    #[test]
    fn propellant_presets_are_valid() {
        for p in [
            Propellant::rp1_lox(),
            Propellant::methane_lox(),
            Propellant::hydrogen_lox(),
            Propellant::hypergolic(),
        ] {
            assert!(p.validate().is_ok());
        }
    }
}
