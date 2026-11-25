//! The twenty-mission scripted catalogue used by the acceptance tests and the
//! Studio. Every script is a complete, deterministic mission specification.

use crate::ascent::AscentConfig;

use super::MissionConfig;

/// One catalogue entry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MissionScript {
    pub id: &'static str,
    pub name: &'static str,
    pub rocket_id: &'static str,
    pub description: &'static str,
    /// Target lunar circular altitude, km.
    pub lunar_altitude_km: f64,
    /// Mission epoch, days.
    pub epoch_days: f64,
    /// Lunar phase at epoch, degrees.
    pub moon_phase_deg: f64,
    /// Sun phase at epoch, degrees.
    pub sun_phase_deg: f64,
}

pub const SCRIPTS: [MissionScript; 20] = [
    MissionScript {
        id: "lunar-kestrel-100",
        name: "Kestrel at 100 km",
        rocket_id: "kestrel",
        description: "Baseline light launcher to a 100 km lunar orbit.",
        lunar_altitude_km: 100.0,
        epoch_days: 0.0,
        moon_phase_deg: 0.0,
        sun_phase_deg: 180.0,
    },
    MissionScript {
        id: "lunar-heron-100",
        name: "Heron at 100 km",
        rocket_id: "heron",
        description: "Medium launcher, full lunar insertion.",
        lunar_altitude_km: 100.0,
        epoch_days: 0.5,
        moon_phase_deg: 20.0,
        sun_phase_deg: 200.0,
    },
    MissionScript {
        id: "lunar-atlas-100",
        name: "Atlas at 100 km",
        rocket_id: "atlas",
        description: "Heavy three-stage mission with margin.",
        lunar_altitude_km: 100.0,
        epoch_days: 1.0,
        moon_phase_deg: 40.0,
        sun_phase_deg: 220.0,
    },
    MissionScript {
        id: "lunar-sparrow-100",
        name: "Sparrow at 100 km",
        rocket_id: "sparrow",
        description: "Methane upper stage demonstration.",
        lunar_altitude_km: 100.0,
        epoch_days: 1.5,
        moon_phase_deg: 60.0,
        sun_phase_deg: 240.0,
    },
    MissionScript {
        id: "lunar-falconet-100",
        name: "Falconet at 100 km",
        rocket_id: "falconet",
        description: "Hypergolic transfer stage mission.",
        lunar_altitude_km: 100.0,
        epoch_days: 2.0,
        moon_phase_deg: 80.0,
        sun_phase_deg: 260.0,
    },
    MissionScript {
        id: "lunar-voyager-100",
        name: "Voyager at 100 km",
        rocket_id: "voyager",
        description: "Deep-space variant with a large third stage.",
        lunar_altitude_km: 100.0,
        epoch_days: 2.5,
        moon_phase_deg: 100.0,
        sun_phase_deg: 280.0,
    },
    MissionScript {
        id: "lunar-kestrel-80",
        name: "Kestrel low orbit",
        rocket_id: "kestrel",
        description: "Tight 80 km lunar orbit.",
        lunar_altitude_km: 80.0,
        epoch_days: 3.0,
        moon_phase_deg: 120.0,
        sun_phase_deg: 300.0,
    },
    MissionScript {
        id: "lunar-heron-120",
        name: "Heron high orbit",
        rocket_id: "heron",
        description: "120 km lunar orbit.",
        lunar_altitude_km: 120.0,
        epoch_days: 3.5,
        moon_phase_deg: 140.0,
        sun_phase_deg: 320.0,
    },
    MissionScript {
        id: "lunar-atlas-150",
        name: "Atlas far orbit",
        rocket_id: "atlas",
        description: "150 km lunar orbit for margin tests.",
        lunar_altitude_km: 150.0,
        epoch_days: 4.0,
        moon_phase_deg: 160.0,
        sun_phase_deg: 340.0,
    },
    MissionScript {
        id: "lunar-sparrow-80",
        name: "Sparrow low orbit",
        rocket_id: "sparrow",
        description: "Low lunar orbit with the small launcher.",
        lunar_altitude_km: 80.0,
        epoch_days: 4.5,
        moon_phase_deg: 180.0,
        sun_phase_deg: 0.0,
    },
    MissionScript {
        id: "lunar-falconet-120",
        name: "Falconet medium orbit",
        rocket_id: "falconet",
        description: "120 km lunar orbit.",
        lunar_altitude_km: 120.0,
        epoch_days: 5.0,
        moon_phase_deg: 200.0,
        sun_phase_deg: 20.0,
    },
    MissionScript {
        id: "lunar-voyager-150",
        name: "Voyager far orbit",
        rocket_id: "voyager",
        description: "150 km lunar orbit with the deep-space stack.",
        lunar_altitude_km: 150.0,
        epoch_days: 5.5,
        moon_phase_deg: 220.0,
        sun_phase_deg: 40.0,
    },
    MissionScript {
        id: "lunar-kestrel-120",
        name: "Kestrel phase C",
        rocket_id: "kestrel",
        description: "Phase-varied 120 km mission.",
        lunar_altitude_km: 120.0,
        epoch_days: 6.0,
        moon_phase_deg: 240.0,
        sun_phase_deg: 60.0,
    },
    MissionScript {
        id: "lunar-heron-150",
        name: "Heron phase D",
        rocket_id: "heron",
        description: "Phase-varied 150 km mission.",
        lunar_altitude_km: 150.0,
        epoch_days: 6.5,
        moon_phase_deg: 260.0,
        sun_phase_deg: 80.0,
    },
    MissionScript {
        id: "lunar-atlas-80",
        name: "Atlas low orbit",
        rocket_id: "atlas",
        description: "Phase-varied 80 km mission.",
        lunar_altitude_km: 80.0,
        epoch_days: 7.0,
        moon_phase_deg: 280.0,
        sun_phase_deg: 100.0,
    },
    MissionScript {
        id: "lunar-sparrow-120",
        name: "Sparrow phase F",
        rocket_id: "sparrow",
        description: "Phase-varied 120 km mission.",
        lunar_altitude_km: 120.0,
        epoch_days: 7.5,
        moon_phase_deg: 300.0,
        sun_phase_deg: 120.0,
    },
    MissionScript {
        id: "lunar-falconet-150",
        name: "Falconet phase G",
        rocket_id: "falconet",
        description: "Phase-varied 150 km mission.",
        lunar_altitude_km: 150.0,
        epoch_days: 8.0,
        moon_phase_deg: 320.0,
        sun_phase_deg: 140.0,
    },
    MissionScript {
        id: "lunar-voyager-80",
        name: "Voyager low orbit",
        rocket_id: "voyager",
        description: "Phase-varied 80 km mission.",
        lunar_altitude_km: 80.0,
        epoch_days: 8.5,
        moon_phase_deg: 340.0,
        sun_phase_deg: 160.0,
    },
    MissionScript {
        id: "lunar-atlas-120",
        name: "Atlas phase I",
        rocket_id: "atlas",
        description: "Late-epoch 120 km mission.",
        lunar_altitude_km: 120.0,
        epoch_days: 9.0,
        moon_phase_deg: 15.0,
        sun_phase_deg: 195.0,
    },
    MissionScript {
        id: "lunar-voyager-120",
        name: "Voyager phase J",
        rocket_id: "voyager",
        description: "Late-epoch 120 km mission.",
        lunar_altitude_km: 120.0,
        epoch_days: 9.5,
        moon_phase_deg: 45.0,
        sun_phase_deg: 225.0,
    },
];

/// Build a runnable configuration from a script.
pub fn config(script: &MissionScript) -> MissionConfig {
    MissionConfig {
        name: script.name.to_string(),
        epoch: script.epoch_days * orbit_core::constants::DAY,
        moon_phase: script.moon_phase_deg.to_radians(),
        sun_phase: script.sun_phase_deg.to_radians(),
        lunar_altitude: script.lunar_altitude_km * 1000.0,
        tolerance: 0.01,
        ascent: AscentConfig {
            target_altitude: 400_000.0,
            ..AscentConfig::default()
        },
        ..MissionConfig::default()
    }
}

pub fn by_id(id: &str) -> Option<MissionScript> {
    SCRIPTS.iter().copied().find(|s| s.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_core::rocket::presets;

    #[test]
    fn catalogue_is_complete_and_unique() {
        assert_eq!(SCRIPTS.len(), 20);
        for (i, script) in SCRIPTS.iter().enumerate() {
            assert!(presets::by_id(script.rocket_id).is_some(), "{}", script.id);
            assert!(script.lunar_altitude_km >= 80.0);
            assert!(config(script).validate().is_ok());
            for other in SCRIPTS.iter().skip(i + 1) {
                assert_ne!(script.id, other.id);
            }
        }
    }

    #[test]
    fn ids_resolve() {
        assert_eq!(by_id("lunar-atlas-100").unwrap().rocket_id, "atlas");
        assert!(by_id("nope").is_none());
    }
}
