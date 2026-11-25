//! A liquid rocket engine with sea-level and vacuum performance.

use crate::G0;

/// One engine type. Thrust is in newtons, specific impulse in seconds and mass
/// in kilograms.
#[derive(Debug, Clone, PartialEq)]
pub struct Engine {
    pub name: String,
    pub thrust_vacuum: f64,
    pub thrust_sea_level: f64,
    pub isp_vacuum: f64,
    pub isp_sea_level: f64,
    pub mass: f64,
    pub min_throttle: f64,
    pub gimbal_deg: f64,
}

impl Engine {
    /// Construct an engine with conservative defaults.
    pub fn new(
        name: impl Into<String>,
        thrust_vacuum: f64,
        thrust_sea_level: f64,
        isp_vacuum: f64,
        isp_sea_level: f64,
        mass: f64,
    ) -> Self {
        Self {
            name: name.into(),
            thrust_vacuum,
            thrust_sea_level,
            isp_vacuum,
            isp_sea_level,
            mass,
            min_throttle: 0.4,
            gimbal_deg: 5.0,
        }
    }

    /// Validate all performance figures. Returns a human-readable reason.
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("engine name is empty".into());
        }
        if self.name.len() > 64 {
            return Err("engine name is longer than 64 bytes".into());
        }
        let values = [
            self.thrust_vacuum,
            self.thrust_sea_level,
            self.isp_vacuum,
            self.isp_sea_level,
            self.mass,
            self.min_throttle,
            self.gimbal_deg,
        ];
        if values.iter().any(|v| !v.is_finite()) {
            return Err("engine contains a non-finite value".into());
        }
        if self.thrust_vacuum <= 0.0 || self.thrust_sea_level <= 0.0 {
            return Err("engine thrust must be positive".into());
        }
        if self.isp_vacuum <= 0.0 || self.isp_sea_level <= 0.0 {
            return Err("engine Isp must be positive".into());
        }
        if self.thrust_sea_level > self.thrust_vacuum * 2.0 {
            return Err("sea-level thrust exceeds twice vacuum thrust".into());
        }
        if self.isp_vacuum > 2000.0 {
            return Err("vacuum Isp exceeds 2000 s (outside this model)".into());
        }
        if self.mass < 0.0 {
            return Err("engine mass must not be negative".into());
        }
        if !(0.0..=1.0).contains(&self.min_throttle) || self.min_throttle <= 0.0 {
            return Err("minimum throttle must be in (0, 1]".into());
        }
        if !(0.0..=30.0).contains(&self.gimbal_deg) {
            return Err("gimbal angle must be in [0, 30] degrees".into());
        }
        Ok(())
    }

    /// Thrust at a given ambient pressure ratio `p / p_sea_level`.
    ///
    /// The interpolation is linear between sea-level and vacuum values, which is
    /// the standard first-order nozzle model used by this simulator.
    pub fn thrust_at(&self, pressure_ratio: f64) -> f64 {
        let p = pressure_ratio.clamp(0.0, 1.0);
        self.thrust_sea_level + (self.thrust_vacuum - self.thrust_sea_level) * (1.0 - p)
    }

    /// Specific impulse at a given ambient pressure ratio (seconds).
    pub fn isp_at(&self, pressure_ratio: f64) -> f64 {
        let p = pressure_ratio.clamp(0.0, 1.0);
        self.isp_sea_level + (self.isp_vacuum - self.isp_sea_level) * (1.0 - p)
    }

    /// Propellant mass flow (kg/s) at a pressure ratio and throttle setting.
    pub fn mass_flow(&self, pressure_ratio: f64, throttle: f64) -> f64 {
        let t = throttle.clamp(0.0, 1.0);
        let thrust = self.thrust_at(pressure_ratio) * t;
        let isp = self.isp_at(pressure_ratio).max(1.0);
        thrust / (isp * G0)
    }

    /// Vacuum mass flow, useful for stage bookkeeping.
    pub fn vacuum_mass_flow(&self) -> f64 {
        self.thrust_vacuum / (self.isp_vacuum * G0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn merlin() -> Engine {
        Engine::new("M-1", 914_000.0, 845_000.0, 311.0, 282.0, 470.0)
    }

    #[test]
    fn performance_interpolates_with_pressure() {
        let e = merlin();
        assert!((e.thrust_at(1.0) - 845_000.0).abs() < 1e-9);
        assert!((e.thrust_at(0.0) - 914_000.0).abs() < 1e-9);
        assert!((e.isp_at(1.0) - 282.0).abs() < 1e-9);
        assert!((e.isp_at(0.0) - 311.0).abs() < 1e-9);
        let mid = e.thrust_at(0.5);
        assert!(mid > 845_000.0 && mid < 914_000.0);
    }

    #[test]
    fn mass_flow_matches_thrust_over_isp() {
        let e = merlin();
        let mdot = e.mass_flow(0.0, 1.0);
        assert!((mdot - 914_000.0 / (311.0 * G0)).abs() < 1e-9);
        assert!((e.mass_flow(0.0, 0.5) - mdot * 0.5).abs() < 1e-9);
    }

    #[test]
    fn validation_rejects_bad_values() {
        assert!(merlin().validate().is_ok());
        let mut e = merlin();
        e.thrust_vacuum = f64::NAN;
        assert!(e.validate().is_err());
        let mut e = merlin();
        e.isp_vacuum = 0.0;
        assert!(e.validate().is_err());
        let mut e = merlin();
        e.min_throttle = 0.0;
        assert!(e.validate().is_err());
        let mut e = merlin();
        e.name = String::new();
        assert!(e.validate().is_err());
    }
}
