## ADDED Requirements

### Requirement: VIEW-012 Surface picking
The editor SHALL turn a cursor position into the point of the planet surface under it and the item under that point (`planet_frame::pick`, GPU-free, f64 on the CPU, `libm` only; ADR 0001, CON-11, CON-14). The ray starts at the camera in `PlanetFixed` and runs through the pixel (`cursor_ray`: pixel centres at `+ 0.5`, the camera's own `ray_frame`). It meets the height field `r(d) = R + h(d)`, not the drawn tiles, so the answer does not depend on the level of detail or on which tiles have arrived. The march runs inside the shell `[R - max_height, R + max_height]` by sphere tracing with a declared slope bound (`PickLimits::max_slope`, default 2) and bisects the bracketing step to `tolerance_m` (default 1e-4 m). A ray that never enters the shell, leaves it without touching the surface, or starts below the surface has no hit; a camera inside the shell picks from its own position. The hit carries the point, the unit direction from the centre, the height at that direction and the distance along the ray. An item is a footprint (id, unit direction, radius along the surface): the item under a hit is the footprint that contains it (great-circle distance by `atan2` of the cross and dot products, metres on `R`; the rim belongs to the footprint) with the smallest distance in units of its own radius, then the smaller radius, then the lower id. The result is a pure function of its inputs, bit for bit.

Verify: A
Status: active
Source: owner (M6 step U2), ADR 0001, SPIKE-14, VIEW-011

#### Scenario: Rays meet the sphere where the analytic answer says
- **WHEN** pixels across the viewport are picked over a bare sphere of 6,371 km radius from orbit, from 50 km and from 300 m up
- **THEN** every hit is within 2 mm of the quadratic's solution and the centre pixel from orbit hits at the distance `R`

#### Scenario: Rays meet a hill where a dense march does
- **WHEN** a ray descends onto a 3 km hill with a 0.25 m reference march
- **THEN** the picked distance is within 0.3 m of the reference, the reported height is the field's height at the hit direction, and the point is on the surface to 1 mm

#### Scenario: Misses and starts
- **WHEN** the ray looks past the limb, away from the planet, or from a camera below the surface, and when a camera 100 m above the surface inside the shell looks down at an angle
- **THEN** the first three give no hit and the last picks a hit between 100 m and 200 m away

#### Scenario: The hit projects back to the cursor
- **WHEN** a hit is projected with the camera's f64 projection
- **THEN** it lands on the cursor pixel within 1e-6 pixels, and two calls give the same bits

#### Scenario: Footprints
- **WHEN** footprints nest, overlap with equal keys, or lie off the hit
- **THEN** the nearest in its own radius wins (a small item near its centre inside a large one), equal keys go to the smaller radius then the lower id, a hit outside every footprint gives none, and small arcs of 1 mm to 2,000 km measure exact to 1e-12 relative
