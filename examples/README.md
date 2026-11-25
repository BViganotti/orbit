# Orbit examples

Two kinds of example live here:

- `rockets/*.json` — complete rocket definitions accepted by the Studio and by
  `orbit-validate`.
- `missions/*.json` — the twenty scripted lunar missions used by the acceptance
  tests and the Studio catalogue.

## Running

The catalogue is built into the server. In the Studio, open **Mission**, pick a
script and press **Run mission**.

From the command line (tooling only; the product is the Studio):

```sh
cargo run --release -p orbit-tools --bin orbit-batch
cargo run --release -p orbit-tools --bin orbit-validate -- examples/rockets/atlas.json
```

## Mission example format

```json
{
  "mission": "lunar-atlas-100",
  "description": "Heavy three-stage mission with margin.",
  "rocket_id": "atlas",
  "lunar_altitude_km": 100,
  "epoch_days": 1,
  "moon_phase_deg": 40,
  "sun_phase_deg": 220,
  "parking_altitude_km": 400,
  "tolerance": 0.01
}
```

`rocket_id` refers to a built-in preset. To use a custom vehicle, pass the rocket
object from the Studio or extend the catalogue in
`crates/orbit-flight/src/mission/scripts.rs`.

## Rocket format

A rocket contains `name`, `payload_mass`, `drag_area`, `drag_coefficient` and
`stages`. Each stage has a structure mass, an engine cluster and one or more
tanks. Every number must be finite and inside the bounds documented in
`docs/formats.md`. All example files validate and are exercised by
`crates/orbit-studio/tests/examples.rs`.
