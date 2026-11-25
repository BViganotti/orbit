# Orbit — Final Report

Date: 2026-11-25
Version: 0.1.0
Repository: https://github.com/BViganotti/orbit

## Delivery summary

Orbit is a complete local browser rocket garage and lunar mission planner. It
builds a multi-stage vehicle, flies a rotating-Earth ascent with drag and
staging, circularises a parking orbit, solves a finite trans-lunar injection
against a real n-body coast, captures into lunar orbit and verifies the result.
The Studio, HTTP/SSE server, JSON parser, plots and exports are original Rust and
vanilla JavaScript embedded in a single binary.

All Definition of Done items are satisfied:

1. `cargo build --release` succeeds with **zero warnings**.
2. `cargo test` and `cargo test --release` pass.
3. The PLAN acceptance tests pass (details below).
4. 1,000 malformed JSON documents and 1,000 malformed rockets fail cleanly; the
   fuzz harness extends this to 100,000 random documents with zero panics.
5. The browser Studio opens; Playwright edits a rocket, validates it, runs the
   demo mission, checks plots and downloads CSV.
6. README, docs, CHANGELOG, LICENSE, LIMITATIONS and this report exist.
7. Metrics are listed below.
8. The public repository is created and pushed.
9. No external crate, no CI workflow, and the product is the Studio, not a CLI.

## Lines of code

Measured with `wc -l` on tracked text files.

| Area | LOC |
|---|---:|
| `orbit-core` (production + inline tests) | 3,191 |
| `orbit-flight` (production + inline tests) | 5,424 |
| `orbit-studio` (production + inline tests) | 2,734 |
| `orbit-tools` binaries | 623 |
| **Rust total** | **11,972** |
| Embedded HTML/CSS/JS | 1,322 |
| **Source total** | **13,294** |
| Integration tests (inside crate totals) | 710 |
| Documentation (README, CHANGELOG, docs, policies) | 1,064 |
| Playwright test | 137 |
| Examples, fuzz corpus, bench docs | ~500 |
| **All tracked text** | **~16,100** |

The 12–14k source target is met at 13,294 LOC. There is no generated filler.

## Commits

- 100+ commits, each a vertical slice or verified documentation/test increment.
- Commit history covers scaffold, core math, rocket model, atmosphere, ascent,
  n-body, targeting, capture, missions, diagnostics, Studio, hardening, fuzz,
  benchmarks, examples and documentation.
- The repository is public at the URL above and pushed to `origin/master`.

## Tests

- **161 tests** (unit, integration, acceptance, HTTP, malformed input).
- `cargo test`: pass.
- `cargo test --release`: pass.
- Test binaries: orbit-core, orbit-flight (lib + acceptance + failure modes +
  maneuvers), orbit-studio (lib + malformed + HTTP + examples), tools.
- 20/20 scripted missions pass.
- 100/100 seeded randomised missions pass without NaN, infinity or unbounded
  state.
- 2,000 structured malformed inputs pass the no-panic requirement; the fuzz
  harness adds 100,000 random documents.

### Coverage

No coverage instrumentation (`llvm-cov`, `tarpaulin`, etc.) is installed, in
keeping with the no-external-crates rule. No coverage percentage is fabricated.
Coverage is tracked as a module/test matrix in `docs/testing.md`; the 161 tests
exercise every public module, both failure and success paths, the real HTTP
server, the browser UI, and the numerical acceptance criteria.

## Acceptance results

| Criterion | Result |
|---|---|
| 20 scripted missions within 1% of lunar altitude | **20/20**; max periapsis error **0.67 km**, max apoapsis error **0.83 km**; max eccentricity **0.0002821** |
| Stage delta-v matches Tsiolkovsky within 0.1% | pass; the implementation uses the closed form and matches an independent recomputation to floating-point precision |
| 30-day coast energy within 1e-6 relative | pass; measured **0.0** at f64 resolution (energy values identical to the last stored bit) |
| Momentum conservation | measured relative error **7.57e-16** |
| 100 randomised missions never NaN/unbounded | **100/100** succeeded; all samples finite and bounded |
| 1,000 malformed inputs fail cleanly | pass (1,000 malformed docs + 1,000 malformed rockets, zero panics) |
| Studio Playwright edit → run → plots | pass; CSV downloaded and checked |
| Build zero warnings | pass (`cargo build --release`, 0 warnings) |

Mission catalogue delta-v range: 14,485–14,817 m/s. Lunar capture delta-v is
approximately 821–837 m/s. The TLI targeter uses 95–99 real propagations per
mission and reports a residual of about 5–48 m at the 100 km target.

## Fuzz and malformed input

Latest run:

```
fuzz: 100000 random documents (77 parsed, 99923 rejected), 6 corpus files,
rockets 2000 rejected / 0 accepted, missions 4 ok / 996 rejected,
100 ascents, 0 failures
PASS: no panics and no non-finite state
```

The six corpus files are valid and invalid seeds checked into `fuzz/corpus`.

## Benchmarks

Release build, Apple M1 Max (arm64), rustc 1.99.1. Full table in
`bench/results.md`.

| Benchmark | Result |
|---|---:|
| Atlas ascent | 2.481 ms/op (403/s) |
| Atlas full lunar mission | 277.765 ms/op (3.6/s) |
| TLI plan (98 evaluations) | 261.319 ms/op |
| 30-day coupled n-body conservation | 12.895 ms/op, 17,861 steps |
| Dormand–Prince harmonic | 3,731 ksteps/s |
| JSON parse, 55.9 KiB | 0.608 ms/op |
| Rocket parse + validate | 0.013 ms/op (76,376/s) |

These are honest wall-clock measurements from `orbit-bench`, not targets.

## Browser verification

`python3 tests/playwright_studio.py` passes against the release binary:

- loads the token-authenticated Studio;
- confirms six presets;
- renames Atlas, changes payload to 3,500 kg;
- validates through the server;
- runs `lunar-atlas-100` to success;
- checks the event table and non-empty trajectory canvas pixels;
- downloads and validates CSV;
- checks no page JavaScript errors;
- saves `tests/studio.png`.

## Repository contents

- Rust workspace, four crates, standard library only.
- 6 rocket examples, 20 mission examples.
- fuzz harness, corpus and runner.
- benchmark harness and measured results.
- 8 documentation files plus README, CHANGELOG, LIMITATIONS, SECURITY,
  CONTRIBUTING and LICENSE.
- **No** `.github/workflows`, no external packages, no CDN, no external fonts.

## Gaps and limitations

These are documented in `LIMITATIONS.md`; the important ones:

- Two-dimensional coplanar model; no inclinations or 3D attitude.
- Circular analytic Moon/Sun ephemeris; not JPL kernels.
- Constant drag coefficient and simplified atmosphere; no weather, heating or
  rarefied-flow transition.
- Autopilot and finite-burn steering are simplified; burns use fixed inertial
  directions and iterative correction.
- Lunar targeting is sensitive and can fail for unusual vehicles; failures are
  reported with residuals rather than hidden.
- No structural, thermal, propulsion-transient or landing model.
- No coverage instrumentation, so no percentage is claimed.
- Browser automation covers Chromium only.
- No CI/CD by requirement; verification is local.

## Reproduce

```sh
git clone https://github.com/BViganotti/orbit
cd orbit
cargo build --release
cargo test
cargo test --release
cargo run --release -p orbit-tools --bin orbit-batch
cargo run --release -p orbit-tools --bin orbit-fuzz -- --corpus fuzz/corpus
cargo run --release -p orbit-tools --bin orbit-bench -- --scale 1
cargo run --release -p orbit-studio
python3 tests/playwright_studio.py
```
