//! Malformed-input hardening: 1,000 documents and 1,000 rocket objects must
//! fail cleanly without panicking.

use std::panic::{catch_unwind, AssertUnwindSafe};

use orbit_core::math::Rng;
use orbit_core::rocket::presets;
use orbit_studio::json::{self, Value};
use orbit_studio::model;

fn mutate(text: &str, rng: &mut Rng, variant: usize) -> String {
    let bytes = text.as_bytes();
    match variant % 5 {
        0 => {
            let cut = 1 + (rng.next_u64() as usize % text.len().saturating_sub(1).max(1));
            text[..cut.min(bytes.len())].to_string()
        }
        1 => format!("{text}\u{0000}"),
        2 => format!("{text} trailing"),
        3 => {
            let mut copy = text.to_string();
            let idx = rng.next_u64() as usize % copy.len().max(1);
            copy.insert(idx.min(copy.len()), '\u{0000}');
            copy
        }
        _ => format!("!{text}"),
    }
}

#[test]
fn one_thousand_malformed_json_documents_fail_cleanly() {
    let base = r#"{"name":"probe","stages":[{"name":"s","structure_mass":1000,"engine_count":1,"engine":{"name":"e","thrust_vacuum":100000,"thrust_sea_level":80000,"isp_vacuum":300,"isp_sea_level":280,"mass":100},"tanks":[{"name":"t","propellant_mass":10000,"dry_mass":100,"propellant":{"name":"p","density":1000}}]}]}"#;
    let mut rng = Rng::new(0xbad_1_0000);
    for i in 0..1000 {
        let text = mutate(base, &mut rng, i);
        let outcome = catch_unwind(AssertUnwindSafe(|| json::parse(&text)));
        match outcome {
            Ok(Err(_)) => {}
            Ok(Ok(_)) => panic!("malformed document {i} parsed: {text:?}"),
            Err(_) => panic!("panic on malformed document {i}"),
        }
    }
}

#[test]
fn one_thousand_malformed_rocket_objects_fail_cleanly() {
    let valid = model::rocket_to_json(&presets::atlas());
    let mut rng = Rng::new(0xbad_0b1ec7);
    let mut rejected = 0;
    for i in 0..1000 {
        let mut value = valid.clone();
        if let Value::Object(entries) = &mut value {
            match i % 5 {
                0 => {
                    entries.retain(|(k, _)| k != "stages");
                }
                1 => {
                    entries.retain(|(k, _)| k != "name");
                }
                2 => entries.push(("payload_mass".into(), json::number(-1.0))),
                3 => {
                    let bogus = Value::Array(vec![Value::Null; 9]);
                    entries.retain(|(k, _)| k != "stages");
                    entries.push(("stages".into(), bogus));
                }
                _ => {
                    let required = ["name", "payload_mass", "drag_area", "stages"];
                    let key = required[i % required.len()];
                    for entry in entries.iter_mut() {
                        if entry.0 == key {
                            entry.1 = Value::Null;
                        }
                    }
                }
            }
        }
        let outcome = catch_unwind(AssertUnwindSafe(|| model::parse_rocket(&value)));
        match outcome {
            Ok(Err(_)) => rejected += 1,
            Ok(Ok(_)) => panic!("malformed rocket {i} was accepted"),
            Err(_) => panic!("panic on malformed rocket {i}"),
        }
    }
    assert_eq!(rejected, 1000);
}

#[test]
fn huge_and_deep_documents_are_rejected() {
    let huge = "x".repeat(json::MAX_INPUT + 1);
    assert!(json::parse(&huge).is_err());
    let deep = "[".repeat(json::MAX_DEPTH + 1) + &"]".repeat(json::MAX_DEPTH + 1);
    assert!(json::parse(&deep).is_err());
    let long_string = format!("\"{}\"", "a".repeat(json::MAX_STRING + 1));
    assert!(json::parse(&long_string).is_err());
}
