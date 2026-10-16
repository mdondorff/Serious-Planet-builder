## MODIFIED Requirements

### Requirement: ATMO-001 Atmosphere LUTs match the CPU reference
The GPU's atmosphere (the sky-view table 192 x 108, the aerial-perspective volume 32 x 32 x 32, the per-pixel orbit march, the sky-view and aerial-perspective lookups of the composite, the terrain's atmosphere lighting) SHALL match the f64 reference of `planet_frame::look::atmosphere` (ATMO-005 to ATMO-009) within `|GPU - reference| <= 1 % x |reference| + 1e-4` per channel (the absolute term dominates at twilight, intentionally), the reference evaluated on the f32-rounded inputs the GPU receives (`Atmosphere::rounded_f32`, `FrameGeometry::rounded_f32`, `StaticLuts::rounded_f32`) with the same texel-to-direction mapping, step counts and sample placement, so that the bound measures arithmetic and not discretisation (ATMO-007). The static tables SHALL be read with `textureLoad` and a manual bilinear interpolation. Class: presentation data (ADR 0035). The bound SHALL hold at camera altitudes 1 m, 2 km and 20 km with the sun at 0.8, 0.05 and -0.08 cosine to the zenith (sunset), for the orbit march at 20,000 km including the rays within 1e-4, 1e-5 and 1e-6 rad of the atmosphere's and the ground's limb (the discriminants there are the factored forms `(r mu - q)(r mu + q)` and `(r mu - rho)(r mu + rho)` with `r`, `rho` and `q = sqrt(c_top)` sent as a high and a low f32 and `r mu` split with `fma`, so the tangent keeps its digits), for the composite (the sky view and aerial-perspective lookups at 1 m, 2 km and 20 km and the orbit march of each pixel; the limb pixels are covered per ray), for the sun disc's coverage across its rim (within 0.03 from f32 vectors) and for the terrain lighting. The planet's radius reaches the shaders only as a value in the uniform block decided in f64 (CON-11 v1.4, ADR 0043 and ADR 0050 point 6): every ray quantity is formed by the altitude-relative primitives of ATMO-006 from `h`, `c`, `c_top` and `r` supplied by the CPU, never as `r^2 - R^2` or `r - R` of f32 planet-sized numbers; the aerial perspective's rays end at the depth.

Verify: B
Status: active
Source: report §15 (M5), Key Finding 7, ADR 0040 points 3-5, ADR 0043

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
Aerial perspective SHALL be applied to all opaque geometry, and atmosphere parameters SHALL be physical so that other planets are a parameter change: the composite reads the one depth buffer the terrain wrote, applies the aerial-perspective volume (a camera inside the atmosphere) or the per-pixel orbit march (outside it) to every pixel with geometry, and the sky to every pixel without. The terrain's lit radiance SHALL be its albedo times the atmosphere's lighting (the transmitted, shadowed sun on Lambert's cosine plus the multiple-scattering ambient), so that `lit` equals `albedo x lighting` as in REND-026.

Verify: B, C
Status: active
Source: report §8, ADR 0040, ADR 0042 point 2

#### Scenario: Distant terrain
- **WHEN** the ground view is rendered with and without the atmosphere, and the same colour is seen at 1 km and at 100 km
- **THEN** the sky is lit where there was black, the terrain differs, and distant pixels are bluer and brighter in the blue channel than near ones

#### Scenario: Both tile formats
- **WHEN** the orbit, 20 km and 1 m views are drawn in the vertex and compact formats
- **THEN** the terrain covers the same pixels with and without the atmosphere and every value is finite and inside the clamp

## ADDED Requirements

### Requirement: ATMO-010 Temporal stability at 1 m altitude
The atmosphere SHALL be stable under camera motion at 1 m altitude (CON-11 v1.4, ADR 0043): with the camera rising from 1.000 m in 1 mm steps over 200 frames, with the sun high, on the horizon and just below it, no sampled sky-view texel of the horizon band (32 texels) and no sampled aerial-perspective texel along the ground (5) SHALL change between two frames by more than the reference's change plus the bound of ATMO-001. The planet's shadow SHALL be a smoothstep across the sun's angular radius (ADR 0050 point 2), so that no sample jumps. Class: presentation data.

Verify: B
Status: active
Source: ADR 0043, ADR 0040 point 5, SPIKE-09

#### Scenario: A slow walk
- **WHEN** 200 frames of a camera rising 1 mm per frame from 1 m are produced on the GPU and the horizon-band texels and aerial-perspective texels are compared frame to frame against the reference
- **THEN** no step exceeds the bound (measured: 0 steps of 199 frame pairs for each of three suns, worst 0.08 of the bound)

### Requirement: ATMO-011 Atmosphere in the render graph and the lit path
The lit frame graph SHALL, with an atmosphere, run the static tables' upload, the sky-view table and the aerial-perspective volume (compute), the terrain into a scene target and the one depth buffer, the sky and aerial-perspective composite into the lit path's HDR target, then the exposure and tone-map passes, compiled when the frame shape changes (size, exposure mode, atmosphere); the tables SHALL be uploaded when the body's tables change and not per frame; no pipeline, target or table SHALL be created in a frame after `enable_atmosphere`; the frame targets (a second HDR-sized scene target, the two tables, the two products) SHALL be counted by the graph and equal what the renderer allocates, inside the frame-target share (ADR 0048). A plan without an atmosphere SHALL draw what it drew before, and the debug path SHALL be untouched. Every atmosphere visual SHALL have debug views (CON-18): the transmittance and multiple-scattering tables as discrete bands (CPU-computed, identical on every adapter: these two have goldens), and, in eight discrete levels but without goldens (continuous GPU products differ across adapters by a few pixels at band edges: inspection views), the sky-view table, the aerial-perspective volume at the scene's depth and the transmittance along each view ray. Class: presentation data.

Verify: A, C
Status: active
Source: ADR 0042 points 1-2 and 5, ADR 0048 point 1, ADR 0040 point 9, CON-18

#### Scenario: The frame graph
- **WHEN** the lit frame with an atmosphere is compiled for adaptive and fixed exposure
- **THEN** the passes run in the order tables, sky view, aerial perspective, terrain, composite, histogram, adaptation, tone map (the histogram culled when fixed), a composite missing an input fails the compile naming it, and the extra targets are 29,491,200 + 165,888 + 262,144 + 262,144 + 16,384 bytes at 1440p inside the share

#### Scenario: Nothing per frame
- **WHEN** frames with and without the atmosphere and with a debug view are drawn after `enable_atmosphere`
- **THEN** no object is created, the tables are uploaded once, the graph is compiled once per change of shape, the renderer's targets equal the graph's count, and a plan with an atmosphere on a renderer without one is refused naming `enable_atmosphere`

#### Scenario: Lit is albedo times lighting
- **WHEN** the lit, albedo and lighting views of the same terrain frame are drawn with an atmosphere (the scene target before the composite) in both tile formats
- **THEN** every terrain pixel of the lit view equals the albedo view times pi times the lighting view over the pre-exposure within 3e-3

#### Scenario: Goldens
- **WHEN** the orbit, 20 km, 1 m and sunset views and the two table views are drawn at a fixed exposure
- **THEN** they equal the adapters' goldens, or are pending for the golden review policy (ADR 0037)
