## MODIFIED Requirements

### Requirement: ATMO-001 Atmosphere LUTs match the CPU reference
The GPU's atmosphere (the sky-view table 192 x 108, the aerial-perspective volume 32 x 32 x 32, the per-pixel orbit march, the per-pixel camera march (ATMO-021), the sky-view and aerial-perspective lookups of the composite, the terrain's atmosphere lighting) SHALL match the f64 reference of `planet_frame::look::atmosphere` (ATMO-005 to ATMO-009) within `|GPU - reference| <= 1 % x |reference| + 1e-4` per channel (the absolute term dominates at twilight, intentionally), the reference evaluated on the f32-rounded inputs the GPU receives (`Atmosphere::rounded_f32`, `FrameGeometry::rounded_f32`, `StaticLuts::rounded_f32`) with the same texel-to-direction mapping, step counts and sample placement, so that the bound measures arithmetic and not discretisation (ATMO-007). The static tables SHALL be read with `textureLoad` and a manual bilinear interpolation. Class: presentation data (ADR 0035). The bound SHALL hold at camera altitudes 1 m, 2 km and 20 km with the sun at 0.8, 0.05 and -0.08 cosine to the zenith (sunset), for the orbit march at 20,000 km including the rays within 1e-4, 1e-5 and 1e-6 rad of the atmosphere's and the ground's limb (the discriminants there are the factored forms `(r mu - q)(r mu + q)` and `(r mu - rho)(r mu + rho)` with `r`, `rho` and `q = sqrt(c_top)` sent as a high and a low f32 and `r mu` split with `fma`, so the tangent keeps its digits), for the composite (the sky view and aerial-perspective lookups at 1 m and 2 km, the camera march of each pixel with geometry at 8.5 km, 20 km, 60 km and 97.5 km, and the orbit march of each pixel; the limb pixels are covered per ray), for the sun disc's coverage across its rim (within 0.03 from f32 vectors) and for the terrain lighting. The planet's radius reaches the shaders only as a value in the uniform block decided in f64 (CON-11 v1.4, ADR 0043 and ADR 0050 point 6): every ray quantity is formed by the altitude-relative primitives of ATMO-006 from `h`, `c`, `c_top` and `r` supplied by the CPU, never as `r^2 - R^2` or `r - R` of f32 planet-sized numbers; the aerial perspective's rays end at the depth.

Verify: B
Status: active
Source: report §15 (M5), Key Finding 7, ADR 0040 points 3-5, ADR 0043

#### Scenario: Differential
- **WHEN** the GPU's products and the CPU reference are computed for the same parameters
- **THEN** the maximum difference is within the bound (the scenarios below give the cases)

#### Scenario: Sky view and aerial perspective
- **WHEN** the GPU's tables are read back at 1 m, 2 km and 20 km with three suns and compared with the reference
- **THEN** every compared texel is within the bound (measured worst 0.09 of the bound for the sky view, 0.08 for aerial perspective on the software adapter)

#### Scenario: Orbit
- **WHEN** rays from 20,000 km, from the nadir to past the limb and within 1e-6 rad of it, are marched on the GPU with and without geometry in front
- **THEN** radiance and transmittance are within the bound (measured worst 0.92 of the bound on the software adapter, whose `fma` is not fused, at the ground's limb minus 1e-6 rad; 0.02 on the RTX 3080 Ti)

#### Scenario: Composite
- **WHEN** a frame with terrain on one half and sky on the other is composited at 2 km, and a sky frame from 20,000 km
- **THEN** every pixel equals the reference applied to the GPU's own tables within the bound (measured worst 0.13 inside, 0.08 from orbit)

### Requirement: ATMO-002 Aerial perspective on all geometry
Aerial perspective SHALL be applied to all opaque geometry, and atmosphere parameters SHALL be physical so that other planets are a parameter change: the composite reads the one depth buffer the terrain wrote, applies the aerial-perspective volume (a camera inside the atmosphere up to 8 km altitude), the per-pixel camera march (inside it above 8 km, ATMO-021) or the per-pixel orbit march (outside it) to every pixel with geometry, and the sky to every pixel without. The terrain's lit radiance SHALL be its albedo times the atmosphere's lighting (the transmitted, shadowed sun on Lambert's cosine plus the multiple-scattering ambient), so that `lit` equals `albedo x lighting` as in REND-026.

Verify: B, C
Status: active
Source: report §8, ADR 0040, ADR 0042 point 2

#### Scenario: Distant terrain
- **WHEN** the ground view is rendered with and without the atmosphere, and the same colour is seen at 1 km and at 100 km
- **THEN** the sky is lit where there was black, the terrain differs, and distant pixels are bluer and brighter in the blue channel than near ones

#### Scenario: Both tile formats
- **WHEN** the orbit, 20 km and 1 m views are drawn from compact tiles (the only tile format since ADR 0062)
- **THEN** the terrain covers the same pixels with and without the atmosphere and every value is finite and inside the clamp

### Requirement: ATMO-007 Discretisation bounds of the tables and the marches
The tables and the marches SHALL be within their declared discretisation bound of the converged integral (the same model with many steps), as a bound separate from the arithmetic bound of the GPU passes (ATMO-001), which is measured on the same discretisation: the transmittance table within 3 % relative + 1e-4 absolute (on transmittance), the multiple-scattering table between its texels within 5 % + 1e-3 (the absolute term reflects 32 samples across the twilight terminator, where the ambient term is a few percent of the sky), and the sky-view march (32 quadratic steps), the aerial-perspective march (`min(2 (slice + 1), 64)` uniform steps) and the orbit march (40 uniform steps from the entry point) within 5 % + 1e-4, at camera altitudes 1 m, 2 km and 20 km (the orbit march at 200 km and 20,000 km) with the sun high, low and below the horizon. The camera march (8 steps while the sun's cosine to the up is at least 0.1 and 12 below it, dense towards the geometry, from the camera to the geometry, ATMO-021) SHALL be within 5 % + 1e-4 per channel of the converged march (2,048 steps) of every ray of the centre column that ends on the ground, at 8 km, 14 km, 20 km, 40 km, 60 km, 97.5 km and 99.5 km, with the sun at 0.7, 0.2, 0.1, 0.05, -0.05 and -0.08, towards the sun and away from it (measured worst 0.89 of the bound). The aerial-perspective volume read between its texels (trilinear over 32 slices and 32 rows, the transmittance averaged over the channels) is within 6 % of the converged march of the same rays up to 8 km altitude with the sun at 0.7 (it is not within 5 % with a low sun: 17 to 18 % at 2 km with the sun at 0.2 or 0.05), and above 8 km it is not used for geometry, because it is not within the bound there. Class: presentation data.

Verify: A
Status: active
Source: ADR 0040 points 3 and 4

#### Scenario: Measured against the bound
- **WHEN** the tables and the three marches are compared with the converged integrals (8,192 steps for transmittance, 2,048 for the sky view, 4,096 for aerial perspective, 8,192 for the orbit march)
- **THEN** the worst measured excess over the bound is 0.75 (transmittance table), 0.41 (multiple-scattering table), 0.23 (sky view), 0.03 (aerial perspective) and 0.70 (orbit) of the bound, and the test fails above 1

#### Scenario: Between the texels of the volume and the camera march
- **WHEN** the centre column of rays that end on the ground (land of radiance 0.010, 0.012, 0.006) is hazed by the camera march and by the trilinear volume, at 1 m, 2 km, 5 km and 8 km (the volume, sun 0.7), at 14 km, 20 km, 60 km, 97.5 km and 99.5 km (both, sun 0.7) and at the altitudes of the camera march above (the camera march, every sun), and each is compared with the converged march
- **THEN** the volume's worst error is 5.6 % (2 km) up to 8 km, and 10.2 % (14 km), 5.1 % (20 km), 23 % (60 km), 21 % (97.5 km) and 18 % (99.5 km) above; the camera march is within the bound everywhere (0.89 of it at worst, at 97.5 km with the sun at 0.1)

### Requirement: ATMO-021 The aerial-perspective range follows the horizon, and the camera marches to far geometry
The distance of the last slice of the aerial-perspective volume SHALL be a per-frame value, `max(256 km, sqrt(c))` with `c = h (2R + h)` for a camera inside the atmosphere (the tangent distance, the distance to the horizon), and 256 km for a camera outside it (where the volume is not used); the slices SHALL remain quadratic, slice `k` at `range ((k + 1) / 32)^2`, and the march SHALL take the same step counts. The CPU SHALL compute the range in f64 (`FrameGeometry::ap_range`) and the GPU SHALL read it from the word that held the constant range, so the slice distances, the froxel march, the lookup and the quarter-range lookup of sky pixels all use it; below about 5 km altitude the range SHALL be exactly 256 km and every frame there SHALL be as before. Terrain farther than 256 km along its ray SHALL be hazed to its own distance as the orbit march hazes it from outside (by the volume up to 8 km altitude, where the range is 256 km, and by the camera march above it): for the same terrain pixel the composite with the camera 1 km inside the top and 1 km outside SHALL differ by at most 5 % per channel (measured worst 3.5 % with the camera march, the difference of the two cameras' own haze; 7.2 % when the volume was read at the discretisation of 32 slices over a 1,130 km range; the clamp to 256 km gave 95 %); terrain above the reference sphere beyond the horizon (a plateau past the tangent point) is not covered by the bounds: the march takes its steps towards the geometry's end, not at the tangent point, and is up to 6 % off by day and about 24 to 39 % off in twilight for such rays. Above 8,000 m camera altitude (`CAMERA_MARCH_MIN_ALTITUDE_M`, the shader's `CAM_MARCH_ALT`) inside the atmosphere a pixel with geometry SHALL NOT read the volume: it SHALL be hazed by a march from the camera to the geometry of `CAMERA_MARCH_STEPS_DAY` = 8 steps while the sun's cosine to the up is at least `CAMERA_MARCH_TWILIGHT_MU` = 0.1 and `CAMERA_MARCH_STEPS_TWILIGHT` = 12 below it, the step boundaries at `s_end (1 - (1 - i / n)^2)` (dense towards the geometry, where the air is thickest), cut where the ray leaves the atmosphere, with the transmittance per channel as the orbit march has it (`FrameGeometry::camera_ray_cosines`, the shader's `atm_camera_ray`); the cost over the volume lookup is about 1.8 ms by day and 2.6 ms in twilight at 1440p with half of the pixels geometry (indicative wall times); at 8,000 m and below, and for sky pixels at every altitude, nothing changes. The volume is still built every frame. The altitude, the step counts and the sun's cosine in the shader SHALL be the reference's. At the altitude of 8 km the haze changes by the error of the volume below it (8 % at the sun 0.7, up to 19 % in twilight, measured against the march).

Verify: A, B, C
Status: active
Source: SPIKE-17 bug 1, SPIKE-20, ATMO-002, ATMO-006, ATMO-007

#### Scenario: Range
(The range is the volume's and the sky pixels' quarter-range lookup's; geometry above 8 km does not read it.)
- **WHEN** the frame geometry is made at altitudes 0, 1 m, 2 km, 5 km, 20 km, 1 km below the top and 1 km above it
- **THEN** the range is 256 km for the first four, the distance to the horizon at 20 km and below the top (about 505 km and 1,130 km), and 256 km outside; the last slice is at the range and the GPU word holds it

#### Scenario: Across the top
- **WHEN** far terrain (ground beyond 1.2 x 256 km) is composited with the camera 1 km inside and 1 km outside the top, at the same pose
- **THEN** each channel differs by at most 5 %

#### Scenario: The camera march
- **WHEN** terrain 120 km, 300 km and 600 km along the view axis is composited at 8.5 km, 60 km and 97.5 km altitude on the GPU
- **THEN** every pixel is within the bound of ATMO-001 of the reference's camera march, and the shader declares the reference's altitude, step counts and twilight cosine

#### Scenario: The march bound
- **WHEN** the aerial-perspective march is compared with the converged march at altitudes 1 m, 2 km, 20 km, 60 km and 99 km
- **THEN** it is within the discretisation bound of ATMO-007 at each
