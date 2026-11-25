# Engineering decisions

## Rust standard library only

No crates keeps the build reproducible and the binary auditable. The cost is
that JSON, HTTP, plotting and benchmarking are implemented in-repo. Each is kept
small and bounded, with explicit limits and tests.

## Two dimensions

A coplanar model keeps targeting well-posed and the state small. It still
exercises real gravity, finite burns, phasing and capture. A 3D model would add
inclination and node targeting but would not change the product shape.

## Analytic ephemeris for missions, coupled system for conservation

Mission planning needs a cheap, differentiable-in-time Moon position and a free
epoch. A circular analytic ephemeris provides that. A time-dependent field does
not conserve a simple energy, so conservation is tested in a separate fully
coupled four-body integrator rather than misusing mission energy.

## Scan, minimise, then bisect

Lunar perilune is extremely sensitive to TLI magnitude. A naive sign-change scan
misses the narrow encounter. The planner scans `0.90–1.20×` Hohmann, minimises
miss distance with golden section, then bisects the target altitude. It reports
the residual and evaluation count.

## Finite burns, iterative correction

Burns integrate mass flow and stage depletion. Because they are not impulses,
the delivered orbit differs slightly from the request. Parking circularisation,
TLI replanning and lunar insertion all correct iteratively. The final osculating
elements are the source of truth.

## Two-burn ascent

The autopilot ends on an elliptical insertion trajectory, then circularises. A
continuous attempt to force a circular orbit by thrust alone over-energised the
vehicle and produced escape trajectories for the lighter stacks. The two-burn
split is physically standard and made every preset robust.

## No fake success

Targeter failures become `TransferFailed` with a residual. Ascent failures
become `AscentFailed`. A lunar orbit outside tolerance becomes `OrbitMiss`. The
UI does not hide these states.

## SSE for results

Mission runs are short enough that result events are computed before they are
streamed, but SSE gives the browser a single ordered channel for progress,
events and completion. Events are real mission events, not a timer animation.

## No CI

The requirement forbids CI workflows. Verification is scripted in
`scripts/verify.sh`, `fuzz/run.sh`, `bench/run.sh` and the Playwright test.

## Rejected alternatives

- **External JSON/HTTP/plotting crates:** violates the std-only rule.
- **Lambert targeting only:** less transparent than the scan/bisect residual;
  kept as a possible future cross-check.
- **Real ephemeris files:** out of v1 scope and would add large assets.
- **3D:** out of scope; would complicate the planner without improving the
  demonstration.
- **In-browser simulation:** the core must be testable headlessly and compile
  into the binary, so physics stays in Rust.
