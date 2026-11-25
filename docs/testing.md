# Testing and benchmarks

## Commands

```sh
cargo test
cargo test --release
cargo build --release
cargo run --release -p orbit-tools --bin orbit-fuzz
cargo run --release -p orbit-tools --bin orbit-bench -- --scale 1
cargo run --release -p orbit-tools --bin orbit-batch
python3 tests/playwright_studio.py
```

`make verify` runs the Rust suite; `make browser` rebuilds and runs Playwright.

## Acceptance mapping

| Requirement | Test |
|---|---|
| 20 missions within 1% of target altitude | `mission::tests::twenty_scripted_missions_reach_lunar_orbit`, `orbit-batch` |
| Tsiolkovsky within 0.1% | `acceptance::tsiolkovsky_matches_independent_computation_within_0_1_percent` |
| 30-day energy within 1e-6 | `acceptance::thirty_day_full_nbody_energy_conservation`, `nbody::system::tests` |
| 100 randomised missions never NaN/unbounded | `acceptance::one_hundred_randomized_missions_never_produce_nan_or_unbounded_state` |
| 1,000 malformed inputs fail cleanly | `crates/orbit-studio/tests/malformed.rs` |
| Studio edit → run → plots | `tests/playwright_studio.py` |
| Build zero warnings | `cargo build --release` (workspace denies nothing; verified manually) |
| No external crates / no CI | workspace manifests and repository audit |

## Determinism

- Every mission is a pure function of rocket and `MissionConfig`.
- `mission::tests::mission_is_deterministic` compares status, total time and
  lunar eccentricity across two runs.
- The fuzz harness is seeded (`0x0f0_1234`); the acceptance fuzz uses
  `0x0b17_2026`.
- The analytic ephemeris has no system-clock dependency.
- The only platform-dependent result is floating-point last-bit behavior.

## Unit and integration coverage by module

| Module | Covered by |
|---|---|
| `Vec2`, RNG | arithmetic, normalization, rotation, bounds, determinism |
| RK4 / Dormand–Prince | decay, oscillator energy, early stop, NaN rejection |
| Roots | bisection, secant, Newton, golden section, bad brackets |
| Engines, tanks, stages | interpolation, mass flow, Tsiolkovsky, reserve, validation |
| Rocket | staging masses, delta-v sum, summary, malformed rockets |
| Atmosphere/drag | sea level, monotonic density, pressure bounds, drag direction |
| Ascent | config bounds, ascent success/failure, remaining delta-v, determinism |
| Gravity/elements | circular, elliptical, hyperbolic, degenerate states, SOI |
| Propagation | circular coast, impact, sample bounds, bad intervals, Hermite |
| Full system | initial energy, short propagation, 30-day conservation |
| Finite burns | velocity change, circularisation, bad inputs, staging |
| TLI | phase/tof, Hohmann delta-v, non-circular rejection, 20 end-to-end plans |
| Capture | circular and elliptical insertion, correction, tangential velocity |
| Mission | validation, demo success, determinism, failure honesty, 20 scripts |
| Diagnostics | state elements, report, CSV, headline |
| JSON | round-trip, malformed corpus, depth/size, surrogates, numbers |
| HTTP | request parsing, percent decoding, rejected methods, status |
| Server | token checks, static assets, real mission over HTTP, SSE |
| UI | Playwright edit/validate/run/plots/CSV |

There is no coverage instrumentation installed in the repository (no external
crates and no `llvm-cov` dependency). Coverage is therefore tracked as the
module/test matrix above plus explicit acceptance tests. This is stated honestly
in `FINAL_REPORT.md`.

## Malformed input

`crates/orbit-studio/tests/malformed.rs` runs:

- 1,000 truncated/corrupted JSON documents, all rejected;
- 1,000 structurally invalid rocket objects, all rejected;
- oversized document, over-deep nesting and over-long string cases.

Each is wrapped in `catch_unwind`; any panic fails the test. The fuzz binary
scales this to 100,000 random documents and adds random ascent configurations.

## Benchmarks

`orbit-bench` uses `std::time::Instant` and `std::hint::black_box`. It reports
ms/op and ops/s for JSON, ascent, full mission, TLI planning, 30-day coupled
n-body conservation, integrator steps, RNG and rocket parsing. Results are
machine-dependent; `bench/results.md` records the run performed for this
release. Benchmarks are not asserted in tests because timing is environment
dependent.

## Browser test

The Playwright test launches the real release binary with `--no-open --port 0`,
reads `ORBIT_URL`, then:

1. waits for the loopback health check;
2. selects the Atlas preset;
3. renames it and edits payload mass;
4. validates it through the server;
5. runs `lunar-atlas-100` and waits for a successful lunar orbit;
6. checks the event table and non-empty canvas pixels;
7. downloads the CSV and asserts its header and row count;
8. saves a screenshot to `tests/studio.png`.

The test requires Python Playwright and a Chromium browser; it is not a Cargo
test because it depends on a browser binary outside the Rust toolchain.

## Known runtime

On the development machine used for this release:

- `cargo test` (debug) is dominated by the 100 randomised missions (~47 s).
- `cargo test --release` is faster but still runs all analytical checks.
- `orbit-batch` runs 20 missions in about 5.7 s in release.
- `orbit-fuzz` processes 100,000 documents plus mutations in under a second.
