//! Curated rocket presets used by the Studio, examples and tests.
//!
//! These are original fictional vehicles sized for the simplified two-body
//! ascent model. They are not models of real launch vehicles.

use crate::rocket::engine::Engine;
use crate::rocket::rocket::Rocket;
use crate::rocket::stage::{Propellant, Stage, Tank};

fn engine(
    name: &str,
    thrust_vac: f64,
    thrust_sl: f64,
    isp_vac: f64,
    isp_sl: f64,
    mass: f64,
) -> Engine {
    Engine::new(name, thrust_vac, thrust_sl, isp_vac, isp_sl, mass)
}

/// Light two-stage lunar trainer.
pub fn kestrel() -> Rocket {
    let s1 = Stage::new(
        "Kestrel booster",
        4200.0,
        engine("K-9", 1_700_000.0, 1_480_000.0, 298.0, 276.0, 560.0),
        3,
    )
    .with_tank(Tank::new(
        "Booster tank",
        238_000.0,
        3200.0,
        Propellant::rp1_lox(),
    ));
    let s2 = Stage::new(
        "Kestrel upper",
        1500.0,
        engine("K-2", 260_000.0, 205_000.0, 352.0, 300.0, 175.0),
        1,
    )
    .with_tank(Tank::new(
        "Upper tank",
        23_500.0,
        620.0,
        Propellant::hydrogen_lox(),
    ));
    let s3 = Stage::new(
        "Kestrel transfer",
        700.0,
        engine("K-1", 90_000.0, 60_000.0, 378.0, 330.0, 110.0),
        1,
    )
    .with_tank(Tank::new(
        "Transfer tank",
        16_000.0,
        350.0,
        Propellant::hydrogen_lox(),
    ));
    let mut r = Rocket::new("Kestrel", 900.0)
        .with_stage(s1)
        .with_stage(s2)
        .with_stage(s3);
    r.drag_area = 10.8;
    r.drag_coefficient = 0.32;
    r
}

/// Medium-lift workhorse.
pub fn heron() -> Rocket {
    let s1 = Stage::new(
        "Heron core",
        7800.0,
        engine("H-12", 2_400_000.0, 2_050_000.0, 305.0, 281.0, 820.0),
        4,
    )
    .with_tank(Tank::new(
        "Core tank",
        420_000.0,
        5600.0,
        Propellant::rp1_lox(),
    ));
    let s2 = Stage::new(
        "Heron upper",
        2400.0,
        engine("H-3", 420_000.0, 330_000.0, 356.0, 305.0, 240.0),
        1,
    )
    .with_tank(Tank::new(
        "Upper tank",
        48_000.0,
        1100.0,
        Propellant::hydrogen_lox(),
    ));
    let s3 = Stage::new(
        "Heron transfer",
        950.0,
        engine("H-1", 130_000.0, 90_000.0, 380.0, 332.0, 140.0),
        1,
    )
    .with_tank(Tank::new(
        "Transfer tank",
        24_000.0,
        480.0,
        Propellant::hydrogen_lox(),
    ));
    let mut r = Rocket::new("Heron", 2200.0)
        .with_stage(s1)
        .with_stage(s2)
        .with_stage(s3);
    r.drag_area = 16.5;
    r.drag_coefficient = 0.34;
    r
}

/// Heavy three-stage vehicle for high-energy lunar missions.
pub fn atlas() -> Rocket {
    let s1 = Stage::new(
        "Atlas core",
        11_000.0,
        engine("A-16", 3_200_000.0, 2_750_000.0, 308.0, 284.0, 1100.0),
        5,
    )
    .with_tank(Tank::new(
        "Core tank",
        630_000.0,
        8600.0,
        Propellant::rp1_lox(),
    ));
    let s2 = Stage::new(
        "Atlas second",
        3600.0,
        engine("A-4", 680_000.0, 520_000.0, 362.0, 310.0, 390.0),
        1,
    )
    .with_tank(Tank::new(
        "Second tank",
        105_000.0,
        2300.0,
        Propellant::hydrogen_lox(),
    ));
    let s3 = Stage::new(
        "Atlas transfer",
        1100.0,
        engine("A-1", 110_000.0, 75_000.0, 380.0, 330.0, 130.0),
        1,
    )
    .with_tank(Tank::new(
        "Transfer tank",
        16_500.0,
        380.0,
        Propellant::hydrogen_lox(),
    ));
    let mut r = Rocket::new("Atlas", 4000.0)
        .with_stage(s1)
        .with_stage(s2)
        .with_stage(s3);
    r.drag_area = 24.0;
    r.drag_coefficient = 0.35;
    r
}

/// Small methane upper-stage demonstrator.
pub fn sparrow() -> Rocket {
    let s1 = Stage::new(
        "Sparrow booster",
        3000.0,
        engine("S-7", 1_250_000.0, 1_090_000.0, 294.0, 272.0, 430.0),
        3,
    )
    .with_tank(Tank::new(
        "Booster tank",
        176_000.0,
        2400.0,
        Propellant::rp1_lox(),
    ));
    let s2 = Stage::new(
        "Sparrow upper",
        1150.0,
        engine("S-2", 190_000.0, 150_000.0, 348.0, 300.0, 140.0),
        1,
    )
    .with_tank(Tank::new(
        "Upper tank",
        18_200.0,
        460.0,
        Propellant::methane_lox(),
    ));
    let s3 = Stage::new(
        "Sparrow transfer",
        550.0,
        engine("S-1", 70_000.0, 45_000.0, 372.0, 325.0, 85.0),
        1,
    )
    .with_tank(Tank::new(
        "Transfer tank",
        11_000.0,
        250.0,
        Propellant::hydrogen_lox(),
    ));
    let mut r = Rocket::new("Sparrow", 550.0)
        .with_stage(s1)
        .with_stage(s2)
        .with_stage(s3);
    r.drag_area = 8.4;
    r.drag_coefficient = 0.3;
    r
}

/// Hypergolic tug with a compact upper stage.
pub fn falconet() -> Rocket {
    let s1 = Stage::new(
        "Falconet booster",
        5200.0,
        engine("F-10", 1_950_000.0, 1_680_000.0, 300.0, 278.0, 610.0),
        3,
    )
    .with_tank(Tank::new(
        "Booster tank",
        268_000.0,
        3600.0,
        Propellant::rp1_lox(),
    ));
    let s2 = Stage::new(
        "Falconet upper",
        1700.0,
        engine("F-2", 310_000.0, 250_000.0, 344.0, 296.0, 190.0),
        1,
    )
    .with_tank(Tank::new(
        "Upper tank",
        28_000.0,
        700.0,
        Propellant::hypergolic(),
    ));
    let s3 = Stage::new(
        "Falconet transfer",
        800.0,
        engine("F-1", 110_000.0, 75_000.0, 376.0, 330.0, 120.0),
        1,
    )
    .with_tank(Tank::new(
        "Transfer tank",
        16_000.0,
        350.0,
        Propellant::hydrogen_lox(),
    ));
    let mut r = Rocket::new("Falconet", 1200.0)
        .with_stage(s1)
        .with_stage(s2)
        .with_stage(s3);
    r.drag_area = 12.2;
    r.drag_coefficient = 0.33;
    r
}

/// Deep-space variant with a large third stage.
pub fn voyager() -> Rocket {
    let s1 = Stage::new(
        "Voyager core",
        12_000.0,
        engine("V-16", 3_250_000.0, 2_800_000.0, 309.0, 285.0, 1120.0),
        5,
    )
    .with_tank(Tank::new(
        "Core tank",
        680_000.0,
        9200.0,
        Propellant::rp1_lox(),
    ));
    let s2 = Stage::new(
        "Voyager second",
        3800.0,
        engine("V-4", 700_000.0, 540_000.0, 362.0, 310.0, 400.0),
        1,
    )
    .with_tank(Tank::new(
        "Second tank",
        108_000.0,
        2350.0,
        Propellant::hydrogen_lox(),
    ));
    let s3 = Stage::new(
        "Voyager transfer",
        1100.0,
        engine("V-1", 120_000.0, 80_000.0, 382.0, 334.0, 130.0),
        1,
    )
    .with_tank(Tank::new(
        "Transfer tank",
        19_000.0,
        400.0,
        Propellant::hydrogen_lox(),
    ));
    let mut r = Rocket::new("Voyager", 3500.0)
        .with_stage(s1)
        .with_stage(s2)
        .with_stage(s3);
    r.drag_area = 25.0;
    r.drag_coefficient = 0.35;
    r
}

pub fn all() -> Vec<Rocket> {
    vec![
        kestrel(),
        heron(),
        atlas(),
        sparrow(),
        falconet(),
        voyager(),
    ]
}

pub fn by_id(id: &str) -> Option<Rocket> {
    match id {
        "kestrel" => Some(kestrel()),
        "heron" => Some(heron()),
        "atlas" => Some(atlas()),
        "sparrow" => Some(sparrow()),
        "falconet" => Some(falconet()),
        "voyager" => Some(voyager()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_validates() {
        for r in all() {
            assert!(r.validate().is_ok(), "{}: {:?}", r.name, r.validate());
        }
    }

    #[test]
    fn presets_are_deterministic() {
        assert_eq!(kestrel(), kestrel());
        assert_eq!(atlas(), atlas());
    }

    #[test]
    fn lookup_and_unknown() {
        assert_eq!(by_id("heron").unwrap().name, "Heron");
        assert!(by_id("not-a-rocket").is_none());
    }

    #[test]
    fn delta_v_ranges_are_plausible() {
        for r in all() {
            let dv = r.total_delta_v();
            assert!((12_000.0..22_000.0).contains(&dv), "{} dv={}", r.name, dv);
        }
    }
}
