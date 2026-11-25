# Architecture

Orbit is a Cargo workspace with four crates and zero external dependencies. The
product is the Studio; the other crates make it testable and scriptable.

## Workspace

```
orbit-core
  src/math/vec2.rs       2D vector algebra
  src/math/integrator.rs RK4 + adaptive Dormand–Prince 5(4)
  src/math/root.rs       bisection, secant, Newton, golden section
  src/math/rng.rs        deterministic xorshift128+ for tests/fuzzing
  src/rocket/            engines, tanks, stages, rocket, checks, presets
  src/aero/              layered atmosphere, drag, stability
  src/constants.rs       physical and astronomical constants

orbit-flight
  src/ascent/            rotating Earth, guidance, staging, events
  src/nbody/             analytic ephemeris, gravity, propagation, full system
  src/plan/              vehicles, finite burns, TLI targeting, lunar capture
  src/mission/           orchestration and the 20-script catalogue
  src/diagnostics/       state diagnostics, CSV, text reports

orbit-studio
  src/json.rs            strict bounded parser/writer
  src/http.rs            HTTP/1.1 parsing and response writing
  src/server.rs          loopback routes, SSE, auth
  src/state.rs           bounded run store and subscribers
  src/model.rs           JSON ↔ rocket/mission conversion
  src/assets.rs          include_str! embedded UI
  src/main.rs            launcher

orbit-tools (development only)
  orbit-fuzz, orbit-bench, orbit-batch, orbit-validate
```

## Data flow

1. The browser loads embedded HTML/CSS/JS from the loopback server.
2. The Garage edits a rocket object and posts it to `/api/validate`; the server
   parses it, validates it and returns engineering issues plus a summary.
3. **Run mission** posts a rocket + mission configuration.
4. `orbit-studio` starts a worker thread, which calls
   `orbit_flight::mission::run_mission`.
5. The mission runs ascent → parking circularisation → TLI planning →
   n-body coast → perilune → capture → altitude correction.
6. Real mission events are emitted to SSE subscribers; the final result,
   CSV and report are stored in a bounded run record.
7. The browser fetches `/api/runs/:id`, draws plots from the samples and offers
   CSV/report downloads.

## Determinism

- No wall-clock time or randomness enters a trajectory.
- The RNG is only used by tests, fuzzing and randomised acceptance runs, and is
  explicitly seeded.
- The integrator is deterministic for a given platform and input.
- The analytic ephemeris is a pure function of time and phase.
- Targeters use fixed scan grids and deterministic bisection/golden-section
  rules. Evaluation counts and residuals are reported.

## Soundness boundaries

| Boundary | Limit |
|---|---|
| JSON document | 2 MiB |
| JSON nesting | 64 |
| JSON values | 100,000 |
| JSON string | 1 MiB |
| Rocket stages | 8 |
| Tanks per stage | 16 |
| Engines per stage | 64 |
| Coast samples | 20,000 (mission trims above 20,000) |
| Ascent samples | 100,000 hard cap |
| Integrator steps | 500,000 default, up to 2,000,000 for conservation |
| Retained Studio runs | 16 |
| SSE subscribers per run | 32 |
| SSE stored events | 2,000 |

Every public parser and numerical entry point rejects non-finite values. A
failed mission returns a status and a residual; it does not return a synthetic
success.

## Concurrency and security

- The server binds `127.0.0.1` only.
- A random 128-bit token is generated at launch and required for every
  `/api/*` request (query parameter or `X-Orbit-Token` header).
- Every connection is handled on a short-lived thread with read/write timeouts.
- Mission runs live in worker threads; results are shared through `Arc` and
  `Mutex`. Locks are never held while writing to a socket.
- Responses set `X-Content-Type-Options: nosniff`, `Referrer-Policy:
  no-referrer` and a strict CSP for HTML.
- Static assets are original and embedded; the UI makes no external requests.

## Test architecture

- Unit tests live beside modules.
- `crates/orbit-flight/tests/acceptance.rs` encodes PLAN/REQUIREMENTS criteria.
- `crates/orbit-studio/tests/http_api.rs` drives the real HTTP server.
- `crates/orbit-studio/tests/malformed.rs` exercises 2,000 malformed inputs.
- `tests/playwright_studio.py` drives the real browser UI.
- `crates/orbit-tools/src/bin/orbit-fuzz.rs` performs 100,000 random document
  parses plus structured mutations.
