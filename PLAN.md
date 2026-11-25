# Orbit — Implementation Plan

Status: ready for one-shot implementation.

## Mission

Orbit is a browser rocket garage and mission planner: build a rocket from stages,
engines and propellant, fly a physically consistent ascent, coast to the Moon
under n-body gravity, and plan maneuvers. It is a simulation and planning tool,
not a game and not a database.

## Product shape

- `orbit-core`: rocket/stage model, mass, engines, aerodynamics, integrators.
- `orbit-flight`: ascent, n-body gravity, Moon ephemeris, maneuver planning.
- `orbit-studio`: local loopback HTTP/SSE Studio with embedded assets.
- `examples/`: 20 scripted missions.
- `tests/`, `fuzz/`, `bench/`.

The user-facing product is the Studio and the embeddable library. A CLI may exist
only for tests, benchmarks and batch mission runs.

## Scope

### In v1
- Rocket editor: stages, dry mass, propellant, engines, thrust, Isp, drag area.
- Tsiolkovsky/staging checks.
- 2D ascent in a rotating Earth frame; atmosphere/drag; gravity turn scripts.
- n-body Earth–Moon–Sun flight with an analytic or simplified ephemeris.
- Maneuver planner: prograde/retrograde/normal burns, target orbit insertion.
- Adaptive RK4/Dormand-Prince integration with error control.
- Energy/momentum diagnostics.
- 2D trajectory plots, altitude/velocity plots, stage timeline.
- Mission scripts and deterministic replay.
- Studio: rocket editor, mission runner, plots, event log.
- Export mission report and trajectory CSV.

### Out of v1
- 3D rendering, real ephemeris files, finite-element structures, atmospheric
  chemistry, launch-site weather, multiplayer.
- Databases, CRDTs, crash analysis, observability.

## Architecture

```
orbit/
  Cargo.toml
  crates/
    orbit-core/
      src/rocket/       # stages, tanks, engines, mass
      src/aero/         # atmosphere, drag, stability
      src/math/         # vectors, integrators, root finding
    orbit-flight/
      src/ascent/       # Earth frame, gravity turn, staging
      src/nbody/        # Earth/Moon/Sun, ephemeris
      src/plan/         # maneuvers, targeter
      src/diagnostics/  # energy, momentum, reports
    orbit-studio/
      src/server.rs
      src/json.rs
      src/assets.rs
  assets/
  examples/
  tests/ fuzz/ bench/
```

Rust std only. Own JSON, HTTP/SSE, plot renderer and UI assets.

## LOC budget (target 12–14k)

| Module | LOC |
|---|---:|
| Rocket/stage/mass | 1,400 |
| Atmosphere/aero | 1,000 |
| Integrators/math | 1,200 |
| Ascent/staging | 1,800 |
| n-body/ephemeris | 1,800 |
| Maneuver planner | 1,600 |
| Diagnostics/CSV | 800 |
| Studio server/API | 1,200 |
| UI/plots | 1,800 |
| Tests/fuzz/bench | 1,400 |

## Milestones and commit ranges

### P0 Scaffold (commits 1–8)
Cargo workspace, test harness, README, LICENSE, Makefile, no CI, no crates.

### P1 Rocket and math (commits 9–20)
Stage/tank/engine model → mass properties → Tsiolkovsky checks → vectors →
integrators → root finding → unit tests.

### P2 Ascent (commits 21–34)
Rotating Earth → gravity/drag → thrust/mass flow → staging → gravity turn →
event detection → ascent tests → energy checks.

### P3 n-body and Moon (commits 35–48)
Earth/Moon/Sun gravity → simplified ephemeris → frame transforms → coast →
impact/orbit detection → 30-day energy conservation.

### P4 Planner (commits 49–62)
Burn model → target orbit → prograde/retrograde/normal → differential correction
→ 20 scripted missions → success metrics.

### P5 Studio (commits 63–76)
HTTP/JSON/SSE → embedded assets → rocket editor → mission runner → plots →
event log → CSV export.

### P6 Hardening (commits 77–92)
100 randomized missions → NaN/overflow fuzz → determinism → benchmarks → docs →
FINAL_REPORT.

## Testing

- 20 scripted rockets reach lunar orbit within 1% of target altitude.
- Stage delta-v matches Tsiolkovsky within 0.1%.
- 30-day coasts conserve energy to 1e-6 (relative).
- 100 randomized missions never produce NaN or unbounded state.
- Malformed rocket files fail cleanly.
- Studio Playwright: build rocket, run mission, inspect plots.

## Acceptance criteria

- Build zero warnings; tests pass.
- 20 missions within 1% target.
- Tsiolkovsky 0.1%, energy 1e-6.
- No NaN in 100 randomized missions.
- Studio runs the full flow.
- No external crates; no CI; no CLI-only product.

## Risks and fallbacks

- Real ephemeris is complex: use a documented simplified model; validate energy
  and known distances.
- Maneuver targeting can fail: report residual errors and fall back to a
  scripted burn sequence; never fake success.
- 2D only: document the plane and assumptions.
