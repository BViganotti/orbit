//! Validate every checked-in example file.

use std::path::PathBuf;

use orbit_core::rocket::presets;
use orbit_flight::mission::MissionConfig;
use orbit_studio::json;
use orbit_studio::model;

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn json_files(dir: PathBuf) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("examples directory")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    files.sort();
    files
}

#[test]
fn all_example_rockets_parse_and_validate() {
    let files = json_files(examples_dir().join("rockets"));
    assert_eq!(files.len(), 6);
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        let value = json::parse(&text).unwrap();
        let rocket = model::parse_rocket(&value).unwrap_or_else(|e| panic!("{:?}: {e}", file));
        assert!(presets::by_id(&rocket.name.to_lowercase()).is_some());
        assert!(!rocket.stages.is_empty());
    }
}

#[test]
fn all_twenty_mission_examples_are_valid() {
    let files = json_files(examples_dir().join("missions"));
    assert_eq!(files.len(), 20);
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        let value = json::parse(&text).unwrap();
        let rocket_id = value.string("rocket_id").expect("rocket_id");
        assert!(presets::by_id(rocket_id).is_some(), "{:?}", file);
        let mut config = MissionConfig::default();
        config.name = value.string("mission").unwrap().to_string();
        config.lunar_altitude = value.number("lunar_altitude_km").unwrap() * 1000.0;
        config.epoch = value.number("epoch_days").unwrap() * 86_400.0;
        config.moon_phase = value.number("moon_phase_deg").unwrap().to_radians();
        config.sun_phase = value.number("sun_phase_deg").unwrap().to_radians();
        config.ascent.target_altitude = value.number("parking_altitude_km").unwrap() * 1000.0;
        config.tolerance = value.number("tolerance").unwrap();
        config.validate().unwrap_or_else(|e| panic!("{:?}: {e}", file));
    }
}
