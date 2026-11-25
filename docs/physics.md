# Physics model

Orbit uses a **two-dimensional, coplanar, prograde** model. This is a planning
and teaching simulator, not a flight-certified tool. All equations are
deterministic and documented so results can be audited.

## Frames

- The inertial frame is Earth-centred, with the Earth–Moon–Sun system in the
  `x–y` plane.
- Earth rotates about `+z` at `ω = 7.292115e-5 rad/s`.
- Launch occurs on the equator at longitude `λ`. The initial position is
  `(R_E, 0)` and inertial velocity is `ω × r`.
- The atmosphere co-rotates: `v_atm = ω × r`.

## Ascent

State: position, inertial velocity and current-stage propellant.

```
a = a_gravity + (T / m) d̂ + a_drag
```

- **Gravity** is the full Earth–Moon–Sun field during ascent, including the
  indirect acceleration of the Earth (see below).
- **Thrust** is `n_engines · T(p) · throttle`, where thrust and Isp interpolate
  linearly between sea-level and vacuum values using `p/p_0`.
- **Mass flow** is `ṁ = T / (Isp · g0)`.
- **Drag** is `0.5 ρ(h) v_rel² C_d A` opposite the relative velocity.
- **Atmosphere** uses fourteen logarithmic density anchors from sea level to
  1,000 km; pressure uses the same density ratio for engine interpolation.
- **Guidance** is a deterministic altitude-based pitch program. The vehicle
  starts vertical and pitches toward the horizon, ending when horizontal speed
  reaches local circular speed with limited radial motion.
- **Staging** occurs when the current stage reaches its reserve fraction. Dry
  mass and residual propellant are jettisoned and the next stage ignites.
- **Events** include liftoff, max-Q, stage separation, engine cutoff, impact and
  timeout.

The ascent ends when `v_h ≥ 0.995 v_circ` and `|v_r|` is small, producing a
near-circular parking orbit at roughly 700–1,100 km for the presets.

## Earth–Moon–Sun field

For an Earth-centred frame:

```
a = -μ_E r/r³
  + [-μ_M (r-r_M)/|r-r_M|³ - μ_M r_M/|r_M|³]
  + [-μ_S (r-r_S)/|r-r_S|³ - μ_S r_S/|r_S|³]
```

The bracketed indirect terms subtract the acceleration of the Earth, so a
spacecraft at rest at the geocentre experiences zero acceleration.

The analytic ephemeris moves the Moon and Sun on circular orbits:

```
r_M(t) = R_M [cos(θ_M0 + n_M t), sin(θ_M0 + n_M t)]
n_M = 2π / 27.32166 days
r_S(t) = R_S [cos(θ_S0 + n_S t), sin(θ_S0 + n_S t)]
n_S = 2π / 365.256 days
```

Mission epochs and initial phases are free parameters, which permits a
favourable lunar departure geometry.

## Integrators

- Fixed-step classical RK4 is used for ascent and finite burns.
- Dormand–Prince 5(4) with embedded error estimate is used for coasts and burn
  targeting. Error norm is RMS over `atol + rtol·max(|y|,|y5|)`.
- Coasts reduce the step ceiling near the Moon so perilune is resolved; closest
  approach is refined with cubic Hermite interpolation and golden-section
  search.
- The coupled conservation run integrates Earth, Moon, Sun and a test particle
  together with mutual gravity in a barycentric frame.

## Parking orbit

The ascent target is an elliptical insertion trajectory. A finite circularisation
burn at the current state removes residual radial velocity; up to six
corrections are applied. The result must have eccentricity below 0.05 before TLI
planning is allowed.

## Trans-lunar injection

1. Given parking radius `r0`, the Hohmann transfer semi-major axis is
   `a = (r0 + R_M)/2` and time of flight `t_f = π√(a³/μ_E)`.
2. The departure phase is chosen so the spacecraft and Moon meet after a
   half-revolution:
   `(n0 - n_M) τ = θ_M0 + n_M t_f - π - θ0 (mod 2π)`.
3. The parking orbit is coasted to the ignition time.
4. A finite prograde burn is applied.
5. The resulting n-body trajectory is propagated through the encounter. The
   burn magnitude is scanned over `0.90–1.20×` the impulsive Hohmann value; the
   scan minimises lunar miss distance, golden-section refines the minimum, and
   bisection solves for the requested perilune altitude.
6. The planner returns the residual in metres and the number of evaluations.

A solution is accepted only when the predicted perilune is within 1% of the
requested altitude. Failures are reported; no fallback burn is invented.

## Lunar capture

At perilune the required circular velocity about the Moon is

```
v_c = √(μ_M / r)  (prograde tangential)
```

A finite burn matches it. Because the burn is finite, the radius drifts by a
few kilometres. The capture routine repeats from the entry state with a small
aim correction, then performs a two-impulse altitude trim:

1. transfer ellipse between the current radius and the target radius;
2. coast half a period;
3. circularise at the target radius.

The final osculating lunar elements are reported. The acceptance test requires
both apsides within 1% of the target altitude.

## Energy and momentum

The mission ephemeris is time-dependent, so a simple two-body energy is not
conserved during a lunar transfer. To test conservation honestly, Orbit provides
a separate fully coupled four-body system (Earth, Moon, Sun and a massless
spacecraft) integrated together. Total energy and barycentric momentum are
computed before and after 30 days. The test requires relative energy error below
`1e-6` and momentum error below `1e-9`.

## Simplifications

- No 3D, inclination changes, nodal precession or out-of-plane burns.
- Circular analytic Moon/Sun ephemeris rather than JPL kernels.
- Constant drag coefficient, no Mach-dependent drag rise or heating.
- No wind, weather, Earth oblateness, tides, solar radiation pressure or
  relativity.
- Engine throttling is a simple dynamic-pressure limiter.
- Structural loads are reported only as max-Q and max-g, not stress.
