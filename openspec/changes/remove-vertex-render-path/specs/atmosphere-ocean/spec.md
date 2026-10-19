## MODIFIED Requirements

### Requirement: ATMO-002 Aerial perspective on all geometry
Aerial perspective SHALL be applied to all opaque geometry, and atmosphere parameters SHALL be physical so that other planets are a parameter change: the composite reads the one depth buffer the terrain wrote, applies the aerial-perspective volume (a camera inside the atmosphere) or the per-pixel orbit march (outside it) to every pixel with geometry, and the sky to every pixel without. The terrain's lit radiance SHALL be its albedo times the atmosphere's lighting (the transmitted, shadowed sun on Lambert's cosine plus the multiple-scattering ambient), so that `lit` equals `albedo x lighting` as in REND-026.

Verify: B, C
Status: active
Source: report §8, ADR 0040, ADR 0042 point 2

#### Scenario: Distant terrain
- **WHEN** the ground view is rendered with and without the atmosphere, and the same colour is seen at 1 km and at 100 km
- **THEN** the sky is lit where there was black, the terrain differs, and distant pixels are bluer and brighter in the blue channel than near ones

#### Scenario: Both tile formats
- **WHEN** the orbit, 20 km and 1 m views are drawn from compact tiles (the only tile format since ADR 0062)
- **THEN** the terrain covers the same pixels with and without the atmosphere and every value is finite and inside the clamp

### Requirement: ATMO-004 Ocean and shoreline
The water SHALL render as the terrain's own grid drawn again at the water level (ADR 0041 point 4): a water vertex is the terrain vertex's tile-local position plus `up x (W - h_morph)` plus the wave displacement, `h_morph` the height the terrain shader morphs to, so that water and terrain agree across LOD, with the terrain's skirts closing the cracks; the pass draws where `W - h_morph > 0` and its class per pixel (wet film, foam, shallow, mid, deep water) comes from that interpolated vertical depth, never from a depth test between two meshes (ATMO-015). The shoreline SHALL NOT flicker as the terrain's level of detail changes or the camera moves (ATMO-017), in the lit path with the atmosphere and without it. Every water view has a debug view (CON-18): the class view is a set of discrete bands (the golden `water_class_*` views), the wave and normal views are inspection views.

Class and bound (ADR 0035): the water level W, the water body and the liquid kind are fields (GEN-023, item `ocean-bake-and-water-level`); the surface (waves, absorption, glint, foam, emission) is presentation data (ATMO-013 to ATMO-015).

Verify: C
Status: active
Source: report §14, §17 item 13, ADR 0041 points 4 to 6, ADR 0042 point 2, ADR 0053

#### Scenario: The class view follows the depth under the water
- **WHEN** the class view of a coastal scene is drawn at 2 km and at 3 m and every water pixel's class is compared with the class of the CPU's depth at the point its ray meets the water
- **THEN** every water pixel is a palette colour, none is off the oracle by more than the mesh's own interpolation near a band edge (measured: 0 of 19,669 and 0 of 21,759), and the classes along the middle column only grow towards the sea

#### Scenario: Coastal flight
- **WHEN** a scripted coastal flight (a 6 m glide towards the coast and a descent from 300 m to 4 m, over a beach of 1:50) is drawn in the class view with the level of detail changing under it
- **THEN** the share of water pixels that change coverage between consecutive frames after reprojection with the f64 poses stays within the budget of ATMO-017 (measured 0.0000 % on both flights)

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
- **WHEN** the lit, albedo and lighting views of the same terrain frame are drawn with an atmosphere (the scene target before the composite)
- **THEN** every terrain pixel of the lit view equals the albedo view times pi times the lighting view over the pre-exposure within 3e-3

#### Scenario: Goldens
- **WHEN** the orbit, 20 km, 1 m and sunset views and the two table views are drawn at a fixed exposure
- **THEN** they equal the adapters' goldens, or are pending for the golden review policy (ADR 0037)

### Requirement: ATMO-018 A second producer through the same water pass
A synthetic lake (a basin with its own level, body id and kind, produced by a test-only `WaterSource` ahead of the sea) SHALL be drawn by the same pass as the sea with no change to it (ADR 0041 point 7, the owner's M5 constraint 5): the plan holds the lake's tiles at the lake's level with the lava liquid, no pipeline or target is made for the second liquid, the lava's emission lights the HDR target (the lake is at least five times brighter than the sunlit land around it and red), a liquid with no waves has one level in the wave view, and the class view shows the basin's shoreline in its own level (goldens `water_class_lava_lake`). Class: presentation data; the lake's level and kind are fields read from W.

Verify: C
Status: active
Source: ADR 0041 point 7, owner's M5 constraints 3 to 5

#### Scenario: The lava lake
- **WHEN** a lake of lava in a bowl of the terrain is drawn with the sea's plan machinery
- **THEN** the lake's nodes carry its level and the lava slot, nothing new is created, its pixels glow red, the wave view is one level on it, and the class view has at least three bands

### Requirement: ATMO-020 The water nodes are cut by the horizon of their level and by their terrain's height
When the request names the terrain's height field, the plan SHALL leave out of the water node list a node that can show no water from the camera.
The cut SHALL act on the node list only; the tile's answer (ATMO-012), the cache's query counters and every key are unchanged.
Definitions (f64, `libm`, CON-11, CON-14):
- `R`: the body's radius; `W`: the tile's level rounded to f32; `A`: the sum of the wave amplitudes of the node's liquid; `S = A (1 + choppy)`.
- `s = (2 d)^2 / (8 (R + W))`, `d` the sample spacing of the node's level: the sag of the drawn surface under the sphere.
- `c`: the chord radius of the tile's bounding sphere on the sphere `R`; `q`: the tile's centre direction times `R + W`.
- `m`: the lowest height of the tile's own `(cells + 1)^2` mesh vertices (f32, as the vertex stage forms it from the tile's texels), the exact minimum of the drawn terrain: the mesh is linear between its vertices, the morph targets are averages of own vertices (heights inside `[m, max]`), and a skirt vertex repeats its border vertex's height, so no drawn height is below `m`.
Horizon:
- `h = |camera| - (R + W)`; with `h` not above 0 (camera at or under the level, a lake above the camera) the node SHALL be kept.
- Otherwise the node SHALL be cut when `|camera - q| - c (R + W) / R - S > sqrt(2 (R + W - s) (h + s) + (h + s)^2) + sqrt(2 (R + W - s) (A + s) + (A + s)^2)`.
Height:
- The node SHALL be cut when the lower height bound of its tile is at or above `W + A + 4 eps_f32 max(1, |W|, |bound|)`.
- The bound is `HeightField::height_bounds` at the tile's centre direction and angular radius, the values of the frustum cut; the morph's heights lie inside it.
- The node SHALL also be cut when `m` is at or above `W + A + 4 eps_f32 max(1, |W|, |m|)`.
- `m` SHALL be computed in the tile's worker job from the mesh it built (the compact tile's lowest texel; a pre-filled tile's likewise) and kept in the `WaterBook` beside a wet entry of the tile (an f32 of RAM, not stored, `MESH_VERSION` and every key unchanged); it is a pure function of the tile and the field (CON-14). A cancelled job and a tile whose job did not run (a root, a tile not yet loaded) hold none, and the plan SHALL read it from the book without computing it and keep the node by the loose bound's verdict until the book holds it.
Result:
- The cut SHALL be a pure function of the camera, the tile, `W`, the look and the book's content: the same for every thread count, cache state and order of questions.
- The kept nodes SHALL be the uncut plan's nodes bit for bit, in the same order; a plan whose nodes are all cut is a water plan with no node.
- The CPU-ray count of sea pixels inside water nodes SHALL be equal for the cut and the uncut plan, and the class view and the lit view pixel-identical.
- The cache SHALL count the nodes cut by the horizon (`cut_horizon`), by the height tests (`cut_height`) and, of those, by `m` (`cut_exact`), and the nodes kept with no minimum in the book (`no_minimum`); recorded only. Every path that plans water SHALL leave the tile's minimum where its tiles are built: the streamer's jobs (the viewer, the streaming scenes), and the held views and flights of the lit perf scenes, which build their meshes beside the plan and fill the water of each as a job does. Those scenes SHALL log the counters on each held view's line and, summed over its plans, on each flight's line.

Class (ADR 0035): plan data (no GPU twin, no stored data). Bound: exact; `m` is the minimum of the f32 heights the vertex stage reads, so the f32 rounding of the vertex stage's `h_origin + hr` and of the water pass's `W - h` is inside the 4 eps margin.
Known limit: the blocking surface is the sphere `R + W` less `s`, so dry ground below `W` between the camera and the tile (a valley under a lake, a dry basin below the sea) can hide nothing and the tile is cut where it is visible; the waves' troughs are taken to be hidden by their crests; skirts add `skirt depth^2 / (2 c)` to a tile's reach when `S` is 0. The terrain between the mesh's vertices is the mesh's, not the field's: a field detail below the lattice spacing that dips under `W` between dry vertices is not drawn and not covered. A node's minimum is read from the book on every plan, never memoised (a lock shared with the jobs, no computation): a tile rebuilt is cut by the minimum of its rebuilt mesh, and a node whose tile has none keeps the loose verdict. Reduced coverage: the tier B scenario of the four coast views samples frames 0, 120 and 239 of the glide and of the descent (not 0, 60, 120, 180 and 239), to keep the test near 16 s; the tier C pictures cover the exact cut at four views only.

Verify: A, B, C
Status: active
Source: ADR 0056, ADR 0058, ATMO-012, ATMO-016, ATMO-019, docs/perf/water-attribution.md, CON-11, LOD-011, ADR 0062

#### Scenario: The horizon at its boundary
- **WHEN** a tile lies 0.5 m inside and 0.5 m beyond the visible distance, for cameras 1 m, 3 m, 2 km and 20,000 km above the level, at levels of -140 m, 0 and 37.5 m
- **THEN** the first is kept, the second is cut, and the tile under the camera is kept

#### Scenario: A camera at or under the level
- **WHEN** the camera is under the level, at it, or a lake's level is above the camera
- **THEN** no node of that level is cut by the horizon

#### Scenario: The height test
- **WHEN** the lowest terrain of a tile is below, at, and above the level plus the rise
- **THEN** the first is kept, the others are cut, and a bound 1e-4 m over a level of 4,000 m with no waves is kept

#### Scenario: The plan is the full plan less nodes
- **WHEN** a plan is made with and without the cut, with a warm and with a cold cache
- **THEN** the cut plan's nodes are the full plan's nodes bit for bit, the cache's query counters are equal, and a node that floods is cut only beyond the horizon

#### Scenario: The four coast views
- **WHEN** the coast-2km, coast-3m, coast-glide and coast-descent frames of the in-memory world of 1,024 cells are planned with and without the cut, and with the exact minimum from the tile jobs
- **THEN** the CPU-ray sea pixel counts are equal for all three, and the nodes kept are 70 to 95 % (coast-2km), 35 to 65 % (coast-3m), 40 to 70 % (glide) and 50 to 80 % (descent) of the uncut plan's by the loose cut, and 45 to 75 %, 25 to 50 %, 30 to 60 % and 35 to 65 % by the exact cut

#### Scenario: Exact minimum at its bound
- **WHEN** a tile's job left a minimum at the first f32 at or above the bound, one f32 below it, and far below it, for levels from -140 m to 9,000 m and a camera at 1 m
- **THEN** the node is cut at the bound and kept one f32 below it, every other node of the plan is the loose cut's bit for bit, and `cut_exact` counts the cut

#### Scenario: Minimum from a real job
- **WHEN** the jobs of wet and dry tiles run over a world with water
- **THEN** a wet tile's minimum equals the minimum recomputed from the mesh it built, a dry or cancelled tile has none, and the minimum of tiles of levels 3 to 20 bounds every vertex height, morphed height and skirt vertex height of the tile, over the reference and the steep field

#### Scenario: Pictures
- **WHEN** an aerial, a low, a beach and an inland view of the 1,024-cell world are drawn in the class view and the lit view with the full selection and with the exact cut
- **THEN** the HDR frames are equal, and the exact cut keeps fewer nodes than the loose cut over the four views together

#### Scenario: Pre-filled tiles
- **WHEN** the 384 pre-filled tiles of level 3 of a stored bake of 256 cells (floor 3) are read from the pack and their jobs fill the water, and orbital plans over the store are made with the book
- **THEN** each pre-filled tile equals the generated tile bit for bit, a wet tile's stored minimum equals its minimum, a dry tile has none, and the plan keeps the nodes the stored minimum allows (none of these views has a node cut by it, so the cutting side is covered by the generated-tile scenarios)

#### Scenario: Held views of the lit scenes
- **WHEN** the lit scene runs its held views coast-2km and coast-3m and its two flights (each also with the shore filter off) in smoke mode
- **THEN** each line reports a nonzero number of nodes cut by the exact minimum and 0 kept with no minimum

