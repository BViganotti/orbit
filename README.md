# Orbit

**A browser rocket garage and mission planner.** Build a rocket from stages,
engines and propellant, fly a physically consistent two-dimensional ascent,
coast to the Moon under Earth–Moon–Sun gravity, and plan a capture into lunar
orbit. Everything runs locally; the Studio is compiled into a single Rust
binary with no external crates, fonts, assets or network calls.

```
cargo run --release -p orbit-studio
```

The launcher binds `127.0.0.1`, prints `ORBIT_URL=…`, and opens the browser with
a random access token. Closing the process closes the Studio.

## What it does

- **Rocket garage** — multi-stage vehicles, engine clusters, tank dry mass,
  propellant density, drag area, reserve propellant, mass properties, payload
  fraction, liftoff TWR and ideal delta-v.
- **Physics-first ascent** — rotating Earth frame, layered atmosphere, drag,
  pressure-dependent thrust and Isp, mass flow, staging, gravity-turn autopilot,
  max-Q tracking, event detection and bounded integration.
- **Full mission pipeline** — parking-orbit circularisation, finite trans-lunar
  injection, 5-day n-body coast, perilune targeting, finite lunar orbit
  insertion and a fine altitude correction.
- **Real targeting** — the TLI solver scans and bisects burn magnitude using
  actual n-body propagations. It reports the residual; it never fabricates a
  successful orbit.
- **Plots and exports** — ground track, altitude, speed, distance to the Moon,
  event log, CSV trajectory and a plain-text mission report.
- **Conservation lab** — a fully coupled Earth–Moon–Sun + test-particle
  integrator checks 30-day energy and momentum conservation.
- **20 scripted missions** — all reach their target lunar orbit within 1%.

## Quick start

Requirements: stable Rust (1.75+) and a browser. No Node, no npm, no external
crates.

```sh
cargo build --release
cargo run --release -p orbit-studio
```

Useful launcher flags:

```sh
cargo run --release -p orbit-studio -- --no-open --port 0
cargo run --release -p orbit-studio -- --token my-development-token-1234
```

`--port 0` asks the OS for a free port. The printed URL is the only way to reach
the authenticated API.

## Studio walkthrough

1. **Garage** — choose a preset, edit payload, stages, engines and tanks. Press
   **Validate** to run the server-side engineering checks.
2. **Mission** — choose one of the 20 catalogue scripts or edit altitude, epoch
   and lunar phase. **Run mission** streams real result events over SSE.
3. **Plots** — inspect the ground track, altitude, speed and lunar distance.
4. **Events** — read the full event log. Export CSV or the report from the
   Mission tab.
5. **Conservation lab** — run the coupled 30-day n-body check from the Garage.

## Verification

```sh
cargo test                 # unit + acceptance + HTTP + malformed input tests
cargo test --release
python3 tests/playwright_studio.py   # real browser smoke test
cargo run --release -p orbit-tools --bin orbit-fuzz
cargo run --release -p orbit-tools --bin orbit-bench -- --scale 1
cargo run --release -p orbit-tools --bin orbit-batch
```

Acceptance results from this repository:

- 20/20 scripted missions reach the target lunar altitude; all are within 1%.
- Tsiolkovsky staging checks match an independent computation within 0.1%.
- The 30-day coupled n-body coast conserves energy to better than 1e-6
  relative (the measured error is at floating-point resolution).
- 100 randomised missions complete without NaN, infinity or unbounded state.
- 1,000 malformed documents and 1,000 malformed rockets fail cleanly.
- The Playwright test edits a rocket, validates it, runs a mission, checks the
  plot pixels and exports CSV.

See `FINAL_REPORT.md` for measured numbers and `LIMITATIONS.md` for the model's
boundaries.

## Repository layout

```
crates/orbit-core      vectors, integrators, roots, RNG, rockets, atmosphere
crates/orbit-flight    ascent, n-body, targeting, capture, missions, diagnostics
crates/orbit-studio    strict JSON, HTTP/SSE server, embedded HTML/CSS/JS
crates/orbit-tools     fuzz, benchmark, batch and validation binaries
assets/                original UI, compiled into the binary
examples/              6 rocket files and 20 mission files
docs/                  architecture, physics, formats, API, testing
tests/                 Playwright browser test
fuzz/ bench/           harness docs, corpus and measured results
```

## Design constraints

- Rust standard library only. No `serde`, no HTTP crate, no plotting crate.
- Deterministic missions: identical inputs produce identical trajectories.
- Bounded parsing: 2 MiB input, 64 nesting levels, 100,000 values, 1 MiB
  strings.
- Bounded state: fixed sample caps, step budgets and retained-run limits.
- No panics on malformed user input; invalid rockets and missions return
  structured errors.
- The server binds loopback only and requires a per-launch bearer token.
- No CI/CD workflows; verification is local and reproducible.

## Documentation

- [Architecture](docs/architecture.md)
- [Physics model](docs/physics.md)
- [File and export formats](docs/formats.md)
- [HTTP API](docs/api.md)
- [Testing and benchmarks](docs/testing.md)
- [Limitations](LIMITATIONS.md)
- [Final report](FINAL_REPORT.md)
- [Changelog](CHANGELOG.md)

## License

MIT. See [LICENSE](LICENSE).
