# atmosphere-ocean Specification

## Purpose
The atmosphere, aerial perspective, HDR exposure and ocean/shoreline rendering: look-up tables checked against a CPU reference, scattering applied to all geometry, and the sea surface. Built in M5 under ADR 0040 (atmosphere), ADR 0041 (water level and ocean), ADR 0038 (HDR and exposure) and ADR 0042 (frame structure); it follows the renderer's rules (CON-03, CON-11 v1.4, CON-18: every visual feature has a debug view and a tier-C test). The atmosphere's CPU reference and static tables are ATMO-005 to ATMO-009 (GPU-free `planet_frame::look::atmosphere`); the GPU passes come with the requirements that cite them.

## Requirements

### Requirement: ATMO-001 Atmosphere LUTs match the CPU reference
The GPU's atmosphere (the sky-view table 192 x 108, the aerial-perspective volume 32 x 32 x 32, the per-pixel orbit march, the sky-view and aerial-perspective lookups of the composite, the terrain's atmosphere lighting) SHALL match the f64 reference of `planet_frame::look::atmosphere` (ATMO-005 to ATMO-009) within `|GPU - reference| <= 1 % x |reference| + 1e-4` per channel (the absolute term dominates at twilight, intentionally), the reference evaluated on the f32-rounded inputs the GPU receives (`Atmosphere::rounded_f32`, `FrameGeometry::rounded_f32`, `StaticLuts::rounded_f32`) with the same texel-to-direction mapping, step counts and sample placement, so that the bound measures arithmetic and not discretisation (ATMO-007). The static tables SHALL be read with `textureLoad` and a manual bilinear interpolation. Class: presentation data (ADR 0035). The bound SHALL hold at camera altitudes 1 m, 2 km and 20 km with the sun at 0.8, 0.05 and -0.08 cosine to the zenith (sunset), for the orbit march at 20,000 km including the rays within 1e-4, 1e-5 and 1e-6 rad of the atmosphere's and the ground's limb (the discriminants there are the factored forms `(r mu - q)(r mu + q)` and `(r mu - rho)(r mu + rho)` with `r`, `rho` and `q = sqrt(c_top)` sent as a high and a low f32 and `r mu` split with `fma`, so the tangent keeps its digits), for the composite (the sky view and aerial-perspective lookups at 1 m, 2 km and 20 km and the orbit march of each pixel; the limb pixels are covered per ray), for the sun disc's coverage across its rim (within 0.03 from f32 vectors) and for the terrain lighting. The planet's radius reaches the shaders only as a value in the uniform block decided in f64 (CON-11 v1.4, ADR 0043 and ADR 0050 point 6): every ray quantity is formed by the altitude-relative primitives of ATMO-006 from `h`, `c`, `c_top` and `r` supplied by the CPU, never as `r^2 - R^2` or `r - R` of f32 planet-sized numbers; the aerial perspective's rays end at the depth.

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

### Requirement: ATMO-003 HDR and exposure
The pipeline SHALL be linear HDR with physical sun units and automatic exposure covering at least ten orders of magnitude of luminance.

Verify: D
Status: planned
Review: contact sheet of ground, limb and sunset views approved by the owner
Source: report §8

#### Scenario: Day to night
- **WHEN** the sun sets in a scripted view
- **THEN** exposure adapts without clipping or banding visible on the contact sheet

### Requirement: ATMO-004 Ocean and shoreline
The water SHALL render as the terrain's own grid drawn again at the water level (ADR 0041 point 4): a water vertex is the terrain vertex's tile-local position plus `up x (W - h_morph)` plus the wave displacement, `h_morph` the height the terrain shader morphs to, so that water and terrain agree across LOD, with the terrain's skirts closing the cracks; the pass draws where `W - h_morph > 0` and its class per pixel (wet film, foam, shallow, mid, deep water) comes from that interpolated vertical depth, never from a depth test between two meshes (ATMO-015). The shoreline SHALL NOT flicker as the terrain's level of detail changes or the camera moves (ATMO-017), in the lit path with the atmosphere and without it, in both tile formats. Every water view has a debug view (CON-18): the class view is a set of discrete bands (the golden `water_class_*` views), the wave and normal views are inspection views.

Class and bound (ADR 0035): the water level W, the water body and the liquid kind are fields (GEN-023, item `ocean-bake-and-water-level`); the surface (waves, absorption, glint, foam, emission) is presentation data (ATMO-013 to ATMO-015).

Verify: C
Status: active
Source: report §14, §17 item 13, ADR 0041 points 4 to 6, ADR 0042 point 2, ADR 0053

#### Scenario: The class view follows the depth under the water
- **WHEN** the class view of a coastal scene is drawn at 2 km and at 3 m in both tile formats and every water pixel's class is compared with the class of the CPU's depth at the point its ray meets the water
- **THEN** every water pixel is a palette colour, none is off the oracle by more than the mesh's own interpolation near a band edge (measured: 0 of 19,669 and 0 of 21,759), the two formats differ on at most 0.5 % of the frame (measured 0), and the classes along the middle column only grow towards the sea

#### Scenario: Coastal flight
- **WHEN** a scripted coastal flight (a 6 m glide towards the coast and a descent from 300 m to 4 m, over a beach of 1:50) is drawn in the class view with the level of detail changing under it
- **THEN** the share of water pixels that change coverage between consecutive frames after reprojection with the f64 poses stays within the budget of ATMO-017 (measured 0.0000 % on both flights)

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

### Requirement: ATMO-012 Water tiles are chosen by a per-tile CPU query at plan time
The nodes of a frame that carry water SHALL be chosen on the CPU at plan time (CON-03) by asking the water field W (GEN-023) for the `(4 + 1)^2` sample directions of each node's tile, memoised per tile (`WaterTileCache`): a tile carries water when any sample reports water, and its water is its most frequent sample's body (ties: the lower body id), so a tile holds one level and one liquid; nothing is stored per tile and `MESH_VERSION` and the compact slot pins are unchanged (ADR 0051 point 6, option 1). The plan SHALL carry, per water node, the terrain node's index, the level (rounded once to f32), the liquid slot and the waves' reduced phases, and the liquids' appearance (at most four per frame); an unknown liquid, more than four liquids and a level that is not a finite height within 1e5 m SHALL be refused naming the id, the numbers or the tile. A plan without water SHALL keep its text and hash; the text form of one with water SHALL round-trip exactly. Known limit of option 1: a dry basin below the level inside a tile that also holds sea is drawn flooded, and a lake smaller than the sample spacing of a coarse tile can be missed at that tile's level. Class: the selection is plan data (no GPU twin); the level, the body and the kind are fields read from W.

Cost, measured ad hoc on an in-memory bake (not a performance verdict, CON-20): 3.6 us per new tile (0.14 us per field query, 25 queries per tile) for the 7,536 tiles of the scripted views at bake sizes 256 and 1024, 0.01 us per tile for a warm plan; the sea's blocks of a stored bake are the ones the terrain's heights read, through the same block cache.

Verify: A
Status: active
Source: ADR 0041 point 8, ADR 0051 point 6, CON-03, CON-04

#### Scenario: Selection
- **WHEN** a plan of a coastal view is made over the reference sea
- **THEN** the water nodes are exactly the nodes with a wet sample, the first plan asks 25 queries per tile and a second plan asks none and is identical, and a later time moves the phases and nothing else

#### Scenario: Two bodies, a missing liquid
- **WHEN** a tile's samples hold two bodies, a liquid the table does not define, five liquids, or a level of 6,000 km
- **THEN** the tile takes the more frequent body (a tie the lower id), and the others are refused naming the liquid id, the numbers or the tile

#### Scenario: Text form
- **WHEN** a plan with water is written to text and read back
- **THEN** it equals the original, a plan without water has no water line, and a damaged line is refused

### Requirement: ATMO-013 Gerstner waves with phases reduced on the CPU
The waves SHALL be plane waves in the planet-fixed frame (ADR 0041 point 5): each liquid has up to four, with the wave vectors `k_i = 2 pi / lambda_i x D_i` (the directions the vertices of a regular tetrahedron, wavelengths 1, 0.62, 0.38 and 0.23 of the liquid's, amplitudes 1, 0.55, 0.3 and 0.15 of its wave height, dispersion `omega = sqrt(g k) / (1 + viscosity)`); the CPU SHALL reduce `(k_i . O + phi_i - omega_i t) mod 2 pi` in f64 per tile origin `O` and plan time `t` (the injected clock, CON-04) and the shader SHALL add only `k_i . local`, `local` the tile-local position of the surface point, so a planet-sized number meets a wave vector nowhere in a shader (CON-11). Two tiles that hold one point therefore agree about its phase, across face edges too (no per-tile tangent projection). A wave's weight is the product of three continuous factors: its tangent share (a smoothstep from 0.25 to 0.6 of the length of its direction's part along the surface, so a wave pointing out of the surface has none), its resolution (a smoothstep from four to eight vertices per wavelength at the vertex's own spacing, the node's sample spacing times one plus the vertex's morph factor, so a wave the grid cannot hold fades out, continuously across LOD borders and also over deep water where the grid is spaced for the seabed's distance) and the shore (a smoothstep from 0 to four times the sum of the amplitudes of the depth, so the surface never dips below the terrain where the water is thin: the vertical displacement is at most 0.57 of the depth). Liquids with no wave height (lava) have no waves. Class: presentation data (ADR 0035; no GPU twin, never stored or keyed). Bound: displacement and slope against the f64 reference on the same f32-rounded inputs and CPU-reduced phases within `1 mm + 1e-3 x (sum of the amplitudes)` per component (ATMO-014); the largest `|k . local|` of a wave with weight over the scripted views from orbit to 1 m is asserted below 250 rad (measured 17 rad) against a limit of 1,000, and by construction it is at most about 1.3 cells (so the f32 argument keeps its digits).

Verify: A
Status: active
Source: ADR 0041 point 5, ADR 0043, CON-11, CON-14

#### Scenario: Known answers
- **WHEN** the tile phase of a wave of `k = 0.1` rad/m at a planet-sized origin is reduced at t = 0, 1.5 s and 1e6 s
- **THEN** it equals the 50-digit value within 1e-8 rad, lies in `[0, 2 pi)`, and repeats after a thousand periods within 1e-6

#### Scenario: Seams
- **WHEN** a point of the shared border of two tiles, also across a face edge, is evaluated from each tile's origin with f32-rounded local numbers
- **THEN** displacement agrees within the bound and slope within 1e-3

#### Scenario: Weights and the shore
- **WHEN** the weights are taken at spacings around four to eight vertices per wavelength, under an up that is a wave's own direction, and at depths from 1 mm to 40 m
- **THEN** they are continuous, a wave on a grid of fewer than four vertices per wavelength or out of the surface has none, and the vertical displacement never exceeds half the depth

### Requirement: ATMO-014 The GPU's waves and surface match the references
The shader's wave function (`water_waves`: displacement and slope) SHALL match `planet_frame::look::water::wave_sample` within `1 mm + 1e-3 x (sum of the amplitudes)` per displacement component and 2e-4 per slope component at camera altitudes 1 m, 10 m, 100 m, 2 km and 20 km with tile origins anywhere on the planet and times up to 3e6 s, and the surface function (`water_surface`: the seabed seen through the liquid's extinction and its own colour, the Fresnel-weighted sky, the GGX sun glint, the emission, the shore fade and the foam) SHALL match `surface_radiance` within `max(1 % |v|, 1e-4)` per channel, the references evaluated on the f32-rounded inputs the GPU receives. In a walk of 1 mm a frame over 200 frames at 1 m altitude no step of either output SHALL differ from the reference's step by more than twice the bound (CON-11 v1.4, ADR 0043). Class: presentation data. Measured on the software adapter: worst displacement error 0.012 of the bound, worst slope 0.020 of its bound, worst surface error 0.001 of its bound (900 probes, 83 of them bright: glint and lava).

Verify: B
Status: active
Source: ADR 0041 point 5, ADR 0043, ADR 0035

#### Scenario: Waves
- **WHEN** 2,000 wave probes (400 at each altitude, depths from 2 mm to 400 m) are run on the GPU
- **THEN** displacement and slope are within the bounds, and probes at t = 1e6 s and 3e6 s on the far side of the planet are within the displacement bound

#### Scenario: Surface
- **WHEN** 900 surface probes (half of them looking along the sun's mirror direction, a fifth of a lava liquid with emission) are run on the GPU
- **THEN** every channel is within the bound

#### Scenario: A slow walk
- **WHEN** 200 frames of a camera moving 1 mm a frame at 1 m altitude are probed
- **THEN** no step differs from the reference's step by more than twice the bound

### Requirement: ATMO-015 The shoreline comes from the vertical depth
The class of a water pixel and the blend band SHALL come from the depth `W - h_morph` interpolated across the triangle (ADR 0041 point 6): the surface layer (reflection, glint, emission) fades in from the shore (`smoothstep(0, shore_fade, depth)`), the foam band is `foam_strength x smoothstep(0, 0.15 w, depth) x (1 - smoothstep(0.15 w, w, depth))` modulated by the swell's phase between 0.6 and 1, and the seabed shows through at the waterline (the pixel at depth 0 is the seabed's); a fragment at depth 0 or less is discarded. The discrete class is a function of the depth alone (film below the shore fade, foam below the foam width, shallow below 2 m, mid below 10 m, deep). Class: presentation data. Bound: the functions have no step larger than 0.03 per millimetre of depth; the class of every water pixel is the class of the CPU's depth within the mesh's own interpolation (ATMO-004).

Verify: A, C
Status: active
Source: ADR 0041 point 6

#### Scenario: Functions
- **WHEN** the cover, the foam and the class are evaluated on a scan of depths from 0 to 4 m
- **THEN** both are zero at the waterline, continuous, and the classes only grow with depth

#### Scenario: The surface
- **WHEN** the surface radiance is taken at the waterline, in 200 m of water, with a white seabed at 3 m, at grazing and steep views and along and away from the mirror direction, and for a lava liquid
- **THEN** it is the seabed at the waterline, independent of the seabed in deep water, redder-poor at 3 m (red absorbed first), brighter at a grazing view, at least ten times brighter at the glint, and the lava glows red where the water of the same depth is dark

### Requirement: ATMO-016 The water pass in the render graph and the frame targets
The lit frame graph SHALL, for a frame with water, run the copy of the scene target and the water pass after the terrain and before the composite (with an atmosphere) or the exposure passes (without), the water pass reading the copy, the scene target, the one depth buffer and the sky-view table, and drawing over the scene target and the depth so that the aerial perspective applies to water as to terrain (ADR 0042 point 2); a frame without water SHALL compile and draw as before. The copy (an HDR-sized `Rgba16Float`, 29,491,200 B at 1440p) SHALL be a frame target counted by the graph and equal to what the renderer allocates, the frame with the atmosphere and water at 1440p 103,926,032 B inside the share of 181,010,432 B (ADR 0048); no pipeline, target or bind group SHALL be created in a frame after `enable_water`; a plan with water on a renderer without it SHALL be refused naming `enable_water`, and so SHALL a water part without a lit part, a node the plan does not have and a phase outside `[0, 2 pi]`. The water shaders SHALL pass the backend's validation and the planet-scale scan (REND-005), and their vertex stages SHALL NOT read the atmosphere's block. The frustum cut of the node selection SHALL keep the tiles whose water the view sees: a field may name a water level (`HeightField::water_cull_level`, the viewer's world does) and a tile whose terrain lies below it is culled by a volume that reaches up to it, only for the frustum test, so a camera low over deep water does not lose the water above the seabed it does not see (a measured defect before the change: a band of unwatered seabed along the bottom of the frame).

Verify: A, C
Status: active
Source: ADR 0042 points 2 and 5, ADR 0048 point 1, ADR 0041 point 4, REND-005

#### Scenario: The frame graph
- **WHEN** the lit frame with water is compiled with and without an atmosphere
- **THEN** the passes run in the order terrain, copy, water, composite (or exposure), a water pass without its copy fails the compile naming it, and the frames without water are the graphs of before

#### Scenario: Nothing per frame
- **WHEN** frames with water are drawn after `enable_water`
- **THEN** no object is created, the graph is compiled once, the pass list names the copy and the water, and the renderer's targets equal the graph's count

#### Scenario: Water above culled seabed
- **WHEN** a camera 6 m over water 6 m deep looks 6 degrees down
- **THEN** the water covers the frame down to its bottom edge

### Requirement: ATMO-017 Coastal flicker metric
Flicker SHALL be measured as the share of water pixels that change coverage between consecutive frames of a coastal flight after reprojection with the f64 camera poses (ADR 0041 point 6): each water pixel of one frame is a ray that meets the water sphere at a point, the point is projected into the other frame, and the pixel counts as changed when that frame shows no water anywhere in the 3 x 3 pixels around the position; a dry pixel counts as a hole when the other frame shows water all around the same point and no terrain nearer the camera hides it; both directions are summed; pixels within four pixels' angle of the horizon, points that left the frame and points that terrain nearer the second camera hides (parallax) are not counted. The metric SHALL be measured on a calm sea (wave height 0): the intended wave motion changes which displaced surface point a grazing pixel sees, and is judged by the bounds of ATMO-013 and ATMO-014. The budget is 0.1 % of the covered pixels (ADR 0041 point 6, confirmed by the first run: 0.0000 % of 305,216 and of 459,725 covered pixels on the two flights of ATMO-004). The exact-class share (a band of the class view narrower than a pixel comes and goes by sampling) is reported, not budgeted, and so is the lit shore's flash (`lit_shore_flash`: the share of shore-band pixels whose lit luminance leaves the mean of the neighbouring frames' by more than 25 %, measured on the water's contribution, lit over terrain alone, with and without the `fwidth` filter of the shore bands, `water.shore_filter`; measured on the two flights: glide 6.5 % without and 16.2 % with the filter, descent 41.0 % without and 7.0 % with; reported, not budgeted, ADR 0053 point 2; the terrain's own shading at those pixels flashes 6.3 % and 36.8 %; `water.shore_filter` defaults to 1; the owner judges the shore in motion at the M5 gate). The coverage budget is the normative one. Class: presentation data.

Verify: A, C
Status: active
Source: ADR 0041 point 6, ADR 0043, CON-18

#### Scenario: The metric on synthetic frames
- **WHEN** two frames of a synthetic coast are compared after a real camera move, with one water pixel in twenty dropped, with the class of those pixels changed, with a camera turned away, and with no water
- **THEN** the clean move is within the budget, the dropped pixels are far above it, the changed classes show only in the exact count, points that left the frame are counted separately, and a frame with no water in both has no share

#### Scenario: Flights
- **WHEN** the two coastal flights of ATMO-004 are drawn with the level of detail changing under them
- **THEN** the shares are within the budget

### Requirement: ATMO-018 A second producer through the same water pass
A synthetic lake (a basin with its own level, body id and kind, produced by a test-only `WaterSource` ahead of the sea) SHALL be drawn by the same pass as the sea with no change to it (ADR 0041 point 7, the owner's M5 constraint 5): the plan holds the lake's tiles at the lake's level with the lava liquid, no pipeline or target is made for the second liquid, the lava's emission lights the HDR target (the lake is at least five times brighter than the sunlit land around it and red), a liquid with no waves has one level in the wave view, and the class view shows the basin's shoreline in its own level (goldens `water_class_lava_lake`). Class: presentation data; the lake's level and kind are fields read from W.

Verify: C
Status: active
Source: ADR 0041 point 7, owner's M5 constraints 3 to 5

#### Scenario: The lava lake
- **WHEN** a lake of lava in a bowl of the terrain is drawn with the sea's plan machinery in both tile formats
- **THEN** the lake's nodes carry its level and the lava slot, nothing new is created, its pixels glow red, the wave view is one level on it, and the class view has at least three bands
