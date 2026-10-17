## MODIFIED Requirements

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

## ADDED Requirements

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
Flicker SHALL be measured as the share of water pixels that change coverage between consecutive frames of a coastal flight after reprojection with the f64 camera poses (ADR 0041 point 6): each water pixel of one frame is a ray that meets the water sphere at a point, the point is projected into the other frame, and the pixel counts as changed when that frame shows no water anywhere in the 3 x 3 pixels around the position; a dry pixel counts as a hole when the other frame shows water all around the same point and no terrain nearer the camera hides it; both directions are summed; pixels within four pixels' angle of the horizon, points that left the frame and points that terrain nearer the second camera hides (parallax) are not counted. The metric SHALL be measured on a calm sea (wave height 0): the intended wave motion changes which displaced surface point a grazing pixel sees, and is judged by the bounds of ATMO-013 and ATMO-014. The budget is 0.1 % of the covered pixels (ADR 0041 point 6, confirmed by the first run: 0.0000 % of 305,216 and of 459,725 covered pixels on the two flights of ATMO-004). The exact-class share (a band of the class view narrower than a pixel comes and goes by sampling) is reported, not budgeted. Class: presentation data.

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
