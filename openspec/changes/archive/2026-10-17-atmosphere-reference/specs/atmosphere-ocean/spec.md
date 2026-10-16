## ADDED Requirements

### Requirement: ATMO-005 Static atmosphere tables on the CPU in f64
The static tables of the atmosphere (ADR 0040 point 2) SHALL be computed on the CPU in f64 with `libm` by a GPU-free module (`planet_frame::look::atmosphere`, CON-02, CON-03) from the look's atmosphere parameters (LOOK-001) and the body radius of the world definition: the **transmittance table** (256 x 64 texels of optical depth, RGB, from a point to the top of the atmosphere along a ray; the transmittance is `exp(-tau)`, and interpolating `tau` and exponentiating after is the lookup) and the **multiple-scattering table** (32 x 32 texels of Hillaire's `psi_ms`, RGB, the second-and-higher-order isotropic in-scattered radiance including the ground bounce, over `1 - f_ms`). Texel `i` of an axis of `N` texels SHALL sit at `i / (N - 1)` and lookups SHALL use that mapping with a manual bilinear interpolation. The tables SHALL be presentation data (ADR 0035): never cached, never keyed, never read by a CPU system other than the references and plan builders, and bit-exact across runs, thread counts (`generate_with_threads`) and operating systems (CON-14). Radiance is in units of the top-of-atmosphere solar illuminance (illuminance 1; the sun's disc has radiance `1 / (pi r^2)`). The GPU block the passes read is fixed here and pinned by a test: `Atmosphere::static_words` (6 `vec4<f32>`: Rayleigh coefficients and scale height; Mie scattering and scale height; Mie absorption and g; ozone absorption and peak altitude; ground albedo and ozone half width; body radius, top height, horizon distance at the top, sun angular radius) then `FrameGeometry::words` (10 `vec4<f32>`: `h r c c_top`; `rho mu_h theta_h beta`; up and `mu_sun`; sun and its sine; the azimuth axes `e1` and `e2`; the ray frame `right`, `up`, `forward` and `near`; pre-exposure factor, inside flag, aerial-perspective range, debug view as bits). The per-frame products of the GPU passes are specified by ATMO-001, ATMO-002 and ATMO-010.

| Attribute | Class | Bound |
|---|---|---|
| transmittance table (optical depth) | presentation | 512 midpoint steps per texel in f64; against the converged integral within ATMO-007 |
| multiple-scattering table | presentation | 8 x 8 directions, 40 midpoint steps; against direct recomputation between texels within ATMO-007 |
| frame geometry (`h`, `r`, `c`, `c_top`, horizon, up, sun cosines, ray frame) | presentation | decided in f64 from the plan, rounded once to f32 (6e-8 relative) |

Verify: A
Status: active
Source: ADR 0040 points 2 and 3, ADR 0035, CON-14

#### Scenario: Known answers
- **WHEN** the Earth tables are generated for a body of radius 6,371 km
- **THEN** their hash equals the recorded known answer, the zenith transmittance at sea level is (0.9404, 0.8676, 0.7623) within 1e-5 and the horizon's is red-shifted

#### Scenario: Thread independence
- **WHEN** the tables are generated on 1, 2, 3, 7 and 64 threads
- **THEN** every table equals the others bit for bit

#### Scenario: Another body and a vacuum
- **WHEN** the tables are generated for another radius and thickness, and for an atmosphere whose coefficients are all zero
- **THEN** the first differs from the Earth's, and the vacuum has zero optical depth, finite multiple scattering and no sky light above the horizon

### Requirement: ATMO-006 Altitude-relative ray geometry at every altitude
Every ray quantity of the atmosphere (ground distance, exit through the top, altitude and sun cosine of a sample, the horizon, the LUT coordinates) SHALL be formed from the altitude-relative primitives of ADR 0040 point 5 at every altitude, in one module, and never as a difference of planet-sized numbers: `c = h (2R + h)`, `c_top = (h - H)(2R + h + H)`, the ground distance `c / (-r mu + sqrt((r mu)^2 - c))`, the exit `-c_top / (r mu + sqrt(D))` for `mu >= 0` and `-r mu + sqrt(D)` for `mu < 0`, the sample altitude `(c + s (s + 2 r mu)) / (r_s + R)`. From outside the atmosphere a ray SHALL be marched from its entry point (altitude `H`, `r mu = -sqrt(D_top)`), with the entry distance `c_top / (-r mu + sqrt(D_top))`. The sky-view mapping SHALL concentrate rows at the horizon (`theta = theta_h (1 - (1 - 2v)^2)` above it) and columns at the sun, with the azimuth taken as `atan2(|dir . e2|, dir . e1)` of the horizontal axes towards the sun, so that texels adjacent to the horizon and to the sun resolve it at 1 m altitude as at 20 km. The aerial-perspective rays end at the geometry's depth or the top of the atmosphere, never at the reference sphere (terrain below it gets its haze); the multiple-scattering factor is capped at 1000 (a non-absorbing optically thick medium); the planet's penumbra is the sun's angular radius in cosine terms at the horizon (`w R / r`); the sun disc's coverage comes from the chord between the view and sun vectors. Class: presentation data. Bound: the primitives equal the naive f64 forms within 1e-8 relative at 1 m, 10 m, 2 km, 20 km and 90 km; in f32 the naive `r^2 - R^2` is off by more than a quarter at its worst between 0.5 m and 20 m while the primitive is within 1e-6; the texel-to-direction mapping inverts within 1e-6 (SPIKE-09 and the GPU passes' bound tests ATMO-001 carry the f32 evidence).

Verify: A
Status: active
Source: ADR 0040 point 5, ADR 0043, CON-11, SPIKE-09

#### Scenario: Primitives against the naive forms in f64
- **WHEN** the ground distance, the exit distance and the sample altitude are formed at 1 m to 90 km altitude for rays from the nadir to the zenith including the horizon
- **THEN** they equal the naive f64 forms within 1e-8 relative

#### Scenario: Why the primitives
- **WHEN** `r^2 - R^2` is formed in f32 for altitudes between 0.5 m and 20 m
- **THEN** the naive form is off by more than 25 % somewhere and the altitude-relative form is within 1e-6 everywhere

#### Scenario: The sky-view mapping
- **WHEN** every fifth row and seventh column of the sky-view table is mapped to a direction and back at 1 m, 2 km and 20 km with the sun high, low and below the horizon
- **THEN** the coordinates come back within 1e-6, the last row above the horizon does not reach the ground and the first below it does, both within 2e-3 of the horizon's cosine

#### Scenario: The regimes meet at the top of the atmosphere
- **WHEN** the same direction is seen from 1 km below and 1 km above the top of the atmosphere
- **THEN** the sky-view texel and the orbit march agree within 8 % + 3e-4

### Requirement: ATMO-007 Discretisation bounds of the tables and the marches
The tables and the marches SHALL be within their declared discretisation bound of the converged integral (the same model with many steps), as a bound separate from the arithmetic bound of the GPU passes (ATMO-001), which is measured on the same discretisation: the transmittance table within 3 % relative + 1e-4 absolute (on transmittance), the multiple-scattering table between its texels within 5 % + 1e-3 (the absolute term reflects 32 samples across the twilight terminator, where the ambient term is a few percent of the sky), and the sky-view march (32 quadratic steps), the aerial-perspective march (`min(2 (slice + 1), 64)` uniform steps) and the orbit march (40 uniform steps from the entry point) within 5 % + 1e-4, at camera altitudes 1 m, 2 km and 20 km (the orbit march at 200 km and 20,000 km) with the sun high, low and below the horizon. Class: presentation data.

Verify: A
Status: active
Source: ADR 0040 points 3 and 4

#### Scenario: Measured against the bound
- **WHEN** the tables and the three marches are compared with the converged integrals (8,192 steps for transmittance, 2,048 for the sky view, 4,096 for aerial perspective, 8,192 for the orbit march)
- **THEN** the worst measured excess over the bound is 0.75 (transmittance table), 0.41 (multiple-scattering table), 0.23 (sky view), 0.03 (aerial perspective) and 0.70 (orbit) of the bound, and the test fails above 1

### Requirement: ATMO-008 Physical reference cases
The reference SHALL reproduce closed-form cases: the vertical optical depth of one exponential species (`sigma H (1 - exp(-top/H))`) and of the ozone tent (`sigma_peak x half width`); the phase functions integrating to 1 over the sphere and the Cornette-Shanks function with `g = 0` equal to Rayleigh's; the terminator's penumbra (0 below the horizon by more than the sun's radius, 1 above, one half on the horizon, the horizon dipping with altitude); the sun's disc carrying illuminance 1; and the radiance of a thousand-times-thinner Rayleigh sky looking at the zenith equal to `phase x sigma x column` within 1 % per channel with the blue-to-red ratio of the coefficients. Class: presentation data.

Verify: A
Status: active
Source: ADR 0040 point 4, ADR 0035

#### Scenario: Optical depth
- **WHEN** the vertical optical depth is integrated for a Rayleigh-only and an ozone-only atmosphere
- **THEN** it equals the closed form within 1e-6 relative (Rayleigh) and 1e-4 (the ozone tent)

#### Scenario: A thin sky
- **WHEN** the zenith radiance is marched in an atmosphere a thousand times thinner with no ground albedo
- **THEN** it equals the single-scattering closed form within 1 % per channel

### Requirement: ATMO-009 The plan carries the atmosphere
A lit plan SHALL carry an atmosphere only when the view asks for one (`LitPlan::with_atmosphere`), as the look's coefficients on the body's radius from the world definition and a view (`composite`, or one of the debug views of the tables and products), so that the renderer needs nothing else from the look; its text form SHALL round-trip exactly, a plan without one SHALL keep its text and hash, and a plan whose radius or atmosphere is out of range SHALL be refused naming the value (LOOK-003). Class: presentation data. Bound: the text carries the view, the radius and the 22 values as exact f64 bit patterns (the round trip is bit-exact); the GPU receives their f32 rounding in the words of ATMO-005.

Verify: A
Status: active
Source: ADR 0040, ADR 0042 point 3, CON-03

#### Scenario: Round trip
- **WHEN** a plan with the Earth atmosphere and another with a debug view is written to text and read back
- **THEN** both equal the originals, the text of a plan without an atmosphere contains no atmosphere token, and a truncated token list is refused
