## ADDED Requirements

### Requirement: ATMO-021 The aerial-perspective range follows the horizon
The distance of the last slice of the aerial-perspective volume SHALL be a per-frame value, `max(256 km, sqrt(c))` with `c = h (2R + h)` for a camera inside the atmosphere (the tangent distance, the distance to the horizon), and 256 km for a camera outside it (where the volume is not used); the slices SHALL remain quadratic, slice `k` at `range ((k + 1) / 32)^2`, and the march SHALL take the same step counts. The CPU SHALL compute the range in f64 (`FrameGeometry::ap_range`) and the GPU SHALL read it from the word that held the constant range, so the slice distances, the froxel march, the lookup and the quarter-range lookup of sky pixels all use it; below about 5 km altitude the range SHALL be exactly 256 km and every frame there SHALL be as before. Terrain farther than 256 km along its ray SHALL be hazed by the volume (to its own distance) as the orbit march hazes it from outside: for the same terrain pixel the composite with the camera 1 km inside the top and 1 km outside SHALL differ by at most 10 % per channel (measured worst 7.2 % at the discretisation of 32 slices over a 1,130 km range; the clamp to 256 km gave 95 %); terrain above the reference sphere beyond the horizon (a plateau past the tangent point) reads the last slice and is not covered.

Verify: A, C
Status: active
Source: SPIKE-17 bug 1, ATMO-002, ATMO-006, ATMO-007

#### Scenario: Range
- **WHEN** the frame geometry is made at altitudes 0, 1 m, 2 km, 5 km, 20 km, 1 km below the top and 1 km above it
- **THEN** the range is 256 km for the first four, the distance to the horizon at 20 km and below the top (about 505 km and 1,130 km), and 256 km outside; the last slice is at the range and the GPU word holds it

#### Scenario: Across the top
- **WHEN** far terrain (ground beyond 1.2 x 256 km) is composited with the camera 1 km inside and 1 km outside the top, at the same pose
- **THEN** each channel differs by at most 10 %

#### Scenario: The march bound
- **WHEN** the aerial-perspective march is compared with the converged march at altitudes 1 m, 2 km, 20 km, 60 km and 99 km
- **THEN** it is within the discretisation bound of ATMO-007 at each
