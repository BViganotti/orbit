# Formats

## JSON limits

Orbit's parser is deliberately strict and bounded:

| Limit | Value |
|---|---:|
| Document size | 2 MiB |
| Nesting depth | 64 |
| Total values | 100,000 |
| String length | 1 MiB |
| Number | finite `f64` only |

Rejected syntax includes trailing data, leading zeroes, missing fraction or
exponent digits, unpaired surrogates, control characters in strings and
non-finite numbers. Duplicate object keys are accepted and the last value wins.

## Rocket schema

```json
{
  "name": "Atlas",
  "payload_mass": 4000,
  "drag_area": 24.0,
  "drag_coefficient": 0.35,
  "stages": [
    {
      "name": "Atlas core",
      "structure_mass": 11000,
      "engine_count": 5,
      "reserve_fraction": 0.0,
      "engine": {
        "name": "A-16",
        "thrust_vacuum": 3200000,
        "thrust_sea_level": 2750000,
        "isp_vacuum": 308,
        "isp_sea_level": 284,
        "mass": 1100,
        "min_throttle": 0.4,
        "gimbal_deg": 5
      },
      "tanks": [
        {
          "name": "Core tank",
          "propellant_mass": 630000,
          "dry_mass": 8600,
          "propellant": { "name": "RP-1/LOX", "density": 1020 }
        }
      ]
    }
  ]
}
```

### Bounds

| Field | Range |
|---|---|
| `name` | 1..=80 bytes |
| stages | 1..=8 |
| `payload_mass` | 0..=1,000,000 kg |
| `drag_area` | 0.1..=1,000 m² |
| `drag_coefficient` | 0.02..=5 |
| structure mass | 0..=1,000,000 kg |
| engine count | 1..=64 |
| engine thrust | 1..=100,000,000 N |
| Isp | 1..=2,000 s |
| engine mass | 0..=100,000 kg |
| tanks | 1..=16 per stage |
| tank propellant | 1..=2,000,000 kg |
| tank dry mass | 0..=1,000,000 kg |
| propellant density | 1..=3,000 kg/m³ |
| reserve fraction | 0..=0.49 |
| liftoff mass | (0, 10,000,000] kg |
| total delta-v | (0, 30,000] m/s |

The server ignores an optional `summary` field, so the files exported by the
Studio can be posted back unchanged.

## Mission request

`POST /api/mission/run` and `/api/demo` accept:

```json
{
  "name": "Atlas at 100 km",
  "rocket": { "...": "rocket object" },
  "lunar_altitude_m": 100000,
  "parking_altitude_m": 400000,
  "epoch_s": 86400,
  "moon_phase_deg": 40,
  "sun_phase_deg": 220,
  "tolerance": 0.01
}
```

Instead of `rocket`, `rocket_id` may name a built-in preset. `script_id` loads a
catalogue script first; explicit fields override it.

| Field | Range | Default |
|---|---|---|
| lunar altitude | 10,000..=1,000,000 m | 100,000 |
| parking target | 80,000..=2,000,000 m | 400,000 |
| epoch | finite seconds | 0 |
| phases | finite degrees | 0 / 180 |
| tolerance | 0.001..=0.2 | 0.01 |

## Mission result

The result JSON contains:

- `status`, `success`, `message`, `rocket_name`
- `summary` — mass, payload fraction, delta-v, TWR, burn time, stages
- `metrics` — mission duration, delta-v by phase, final mass, target and errors
- `transfer` — parking coast, TLI delta-v/time, perilune, residual, evaluations
- `lunar` — apsides, eccentricity, period, capture delta-v, iterations
- `events` — time, phase, kind, detail
- `samples` — `[t, altitude, speed, moon_distance, phase, mass, x, y]`
  with phase `0=ascent`, `1=parking`, `2=transfer`, `3=lunar`
- `samples` are rounded for transport; the CSV carries three decimals.

Unavailable numeric values (for example an ascent failure with no lunar orbit)
are serialized as `null`, not zero.

## CSV

```
time_s,phase,altitude_m,speed_m_s,moon_distance_m,mass_kg
86400.000,ascent,0.000,464.581,380615684.569,788500.000
```

- Time is absolute mission time (epoch + elapsed), seconds.
- Altitude and distance are metres.
- Speed is metres per second.
- Mass is kilograms.
- Non-finite values would be written as `nan`, but the simulator rejects them
  before export.

## Report

A plain-text report contains vehicle summary, delta-v budget, transfer details,
lunar orbit, error residuals and the complete event log. It ends with an
explicit `RESULT: SUCCESS` or `RESULT: FAILED - …` line.
