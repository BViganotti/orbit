# Glossary

- **Apoapsis / apogee / apolune** — farthest point of an orbit from Earth /
  Earth / Moon. Orbit uses "apoapsis" generically.
- **Periapsis / perigee / perilune** — nearest point.
- **Ascending node** — not modelled; the simulation is coplanar.
- **Circularisation burn** — burn that removes radial velocity and matches local
  circular speed.
- **Delta-v (Δv)** — velocity change; here the integral of thrust acceleration.
- **Dormand–Prince 5(4)** — adaptive Runge–Kutta pair with fifth-order solution
  and fourth-order error estimate.
- **Eccentricity** — shape of a conic; 0 is circular, <1 elliptical, 1 parabolic,
  >1 hyperbolic.
- **Ephemeris** — time-dependent body positions. Orbit uses circular analytic
  orbits, not JPL kernels.
- **Finite burn** — burn with duration and mass flow, as opposed to an impulse.
- **Gravity turn** — pitching a rocket toward the horizon so gravity and thrust
  shape the trajectory.
- **Hohmann transfer** — minimum-energy two-impulse transfer between circular
  orbits.
- **Isp (specific impulse)** — thrust per unit weight flow; seconds.
- **Max-Q** — maximum dynamic pressure during ascent.
- **Osculating elements** — instantaneous two-body orbital elements at a state.
- **Parking orbit** — temporary Earth orbit before TLI.
- **SOI (sphere of influence)** — region where a body's gravity dominates;
  lunar SOI is about 66,180 km in this model.
- **Specific energy** — energy per unit mass, `v²/2 − μ/r`.
- **TLI** — trans-lunar injection.
- **TWR** — thrust-to-weight ratio.
- **Tsiolkovsky equation** — `Δv = Isp·g0·ln(m0/mf)`.
- **Two-body problem** — motion under a single inverse-square gravity field.
