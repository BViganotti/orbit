# Limitations

Orbit is a two-dimensional planning and teaching simulator. It is deliberately
sound about what it reports, but it is not a flight-dynamics package and not a
certification tool.

## Model limits

- **2D only.** All motion is coplanar and prograde. There is no inclination,
  node, argument of periapsis, out-of-plane burn or 3D attitude.
- **Simplified ephemeris.** The Moon and Sun move on circular coplanar orbits
  with fixed periods. There are no JPL kernels, eccentricities, lunar
  libration, Earth oblateness, tides, relativity or solar radiation pressure.
- **Atmosphere.** Fourteen logarithmic density anchors above a smooth
  interpolation. No wind, weather, gusts, Mach-dependent drag rise, heating or
  rarefied-flow transition.
- **Ascent guidance is an autopilot model.** It is a deterministic altitude and
  velocity program, not a validated flight computer. Structural loads are
  reported only as max-Q and max-g.
- **Finite burns use fixed inertial thrust directions.** A long TLI or capture
  burn is not steered continuously; the mission corrects the resulting orbit
  numerically. Burn residuals are reported.
- **Parking orbit altitude is not exact.** The ascent reaches a near-circular
  orbit roughly between 700 and 1,100 km depending on the vehicle; the mission
  uses the achieved radius rather than forcing a nominal altitude.
- **Lunar altitude trim.** Capture is followed by a small two-impulse trim. The
  final osculating apsides are reported; the acceptance threshold is 1% of the
  target altitude, which is 0.8–1.5 km for the catalogue.
- **No propulsion transients.** Ignition, shutdown, chilldown, residuals and
  throttling dynamics are not modelled. Engine performance is a linear
  sea-level/vacuum interpolation.
- **No structural or thermal model.** Tanks, engines and stages are point
  masses with fixed dry mass. There is no stress, buckling, sloshing, aeroelastic
  or thermal analysis.
- **No landing, descent or surface operations.** The mission ends in lunar
  orbit. Earth impact and lunar impact are detection events only.

## Numerical limits

- Adaptive integration is deterministic on a given platform but floating-point
  last bits can differ across architectures.
- Perilune targeting is sensitive: a change of a few metres per second in TLI
  can move the lunar miss by thousands of kilometres. The planner scans,
  minimises miss distance and bisects, but some vehicles or geometries have no
  solution and are reported as failures.
- The 30-day conservation test integrates a separate fully coupled four-body
  system. The mission ephemeris itself is time-dependent, so mission energy is
  not a conserved scalar and is not presented as one.
- The measured relative energy error in the conservation test is at or below
  double-precision resolution for the chosen state; the test threshold is 1e-6.
- Ascent and burn integration use fixed steps for reproducibility, not
  guaranteed error bounds.

## Product limits

- The Studio is local and single-user. There is no account system, persistence,
  database, multiplayer or remote access.
- Browser support is current Chromium/Firefox/Safari; the automated browser test
  covers Chromium via Playwright. Canvas plots are redrawn on resize.
- Mission results are held in memory; the last 16 runs are retained. Closing the
  process clears them. CSV and report downloads are the durable export path.
- No CI/CD workflows exist, by requirement. Verification is local.
- No coverage instrumentation is installed. Coverage is tracked as a module/test
  matrix in `docs/testing.md`; no coverage percentage is fabricated.

## Delivery notes

- If no git remote is available or push fails, `PUSH.md` contains the exact
  commands to publish the repository.
- Benchmarks are machine-dependent. `bench/results.md` records the measured run
  for this release rather than a synthetic target.
- The project has no external crate dependencies. Rust standard library only.
