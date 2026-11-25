# Changelog

All notable changes to Orbit are recorded here. The project uses a single
`0.1.0` release line; dates are ISO 8601.

## [0.1.0] — 2026-11-25

### Added

- Cargo workspace with four std-only crates: `orbit-core`, `orbit-flight`,
  `orbit-studio`, `orbit-tools`.
- Two-dimensional vector algebra, fixed RK4 and adaptive Dormand–Prince 5(4),
  bisection/secant/Newton/golden-section solvers, deterministic xorshift RNG.
- Rocket model: engines with pressure-dependent thrust and Isp, tanks with
  propellant density, multi-stage stacks, reserve propellant, mass properties,
  Tsiolkovsky delta-v, liftoff TWR and engineering checks.
- Layered exponential atmosphere, drag, dynamic pressure, speed of sound and
  static-stability classification.
- Rotating-Earth ascent with gravity turn, staging, drag, max-Q tracking,
  event detection and remaining delta-v accounting.
- Analytic Earth–Moon–Sun ephemeris with documented circular orbits and
  indirect-frame acceleration terms.
- Event-aware n-body propagator with adaptive error control, Hermite closest
  approach and lunar SOI detection.
- Fully coupled four-body integrator and 30-day energy/momentum conservation
  experiment.
- Finite burns, automatic staging, parking circularisation, TLI phase solving,
  scan/golden-section/bisection perilune targeting, lunar insertion and
  two-impulse altitude trim.
- Deterministic mission runner with four phases, bounded samples, event log,
  delta-v accounting and honest failure statuses.
- Twenty scripted lunar missions across six presets, target altitudes and
  epochs.
- Diagnostics: osculating elements, Earth/Moon energy and momentum, CSV export,
  text mission report and headline summary.
- Studio: strict bounded JSON parser/writer, HTTP/1.1 server, token auth, SSE
  run stream, bounded run store, embedded original HTML/CSS/JS, rocket editor,
  validation panel, mission runner, canvas plots, event table, CSV/report
  downloads and a conservation lab.
- Tooling: 100,000-input fuzz harness, benchmark harness, batch mission runner
  and offline rocket validator.
- Six example rockets, twenty example mission files and documentation for
  architecture, physics, formats, API and testing.
- Acceptance tests for Tsiolkovsky, conservation, ascent, 100 randomised
  missions, 2,000 malformed inputs, HTTP API and a Playwright browser flow.

### Fixed during development

- Dormand–Prince workspace now supports state vectors longer than eight
  elements (needed for the four-body system).
- Mission duration is reported as elapsed time rather than absolute epoch time.
- Unavailable residuals serialize as `null` instead of a misleading zero.
- Plot time axes are relative to the departure epoch.
- Voyager ascent and capture were tuned after failing to reach a near-circular
  parking orbit.

### Security

- Server binds only to `127.0.0.1`.
- Random 128-bit bearer token required for all API routes.
- Strict CSP, no-sniff and no-referrer headers.
- Hard limits on JSON, HTTP requests, integrations, samples and retained runs.

### Notes

- No external crates, vendored code, CDNs, external fonts or runtime downloads.
- No CI/CD workflows, by project requirement.
