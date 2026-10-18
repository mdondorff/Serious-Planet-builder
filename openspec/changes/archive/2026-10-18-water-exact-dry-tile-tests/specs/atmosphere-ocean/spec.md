## MODIFIED Requirements

### Requirement: ATMO-020 The water nodes are cut by the horizon of their level and by their terrain's height
When the request names the terrain's height field, the plan SHALL leave out of the water node list a node that can show no water from the camera.
The cut SHALL act on the node list only; the tile's answer (ATMO-012), the cache's query counters and every key are unchanged.
Definitions (f64, `libm`, CON-11, CON-14):
- `R`: the body's radius; `W`: the tile's level rounded to f32; `A`: the sum of the wave amplitudes of the node's liquid; `S = A (1 + choppy)`.
- `s = (2 d)^2 / (8 (R + W))`, `d` the sample spacing of the node's level: the sag of the drawn surface under the sphere.
- `c`: the chord radius of the tile's bounding sphere on the sphere `R`; `q`: the tile's centre direction times `R + W`.
- `m`: the lowest height of the tile's own `(cells + 1)^2` mesh vertices (f32, as the vertex stage forms it from the tile's texels or vertices), the exact minimum of the drawn terrain: the mesh is linear between its vertices, the morph targets are averages of own vertices (heights inside `[m, max]`), and a skirt vertex repeats its border vertex's height, so no drawn height is below `m`.
Horizon:
- `h = |camera| - (R + W)`; with `h` not above 0 (camera at or under the level, a lake above the camera) the node SHALL be kept.
- Otherwise the node SHALL be cut when `|camera - q| - c (R + W) / R - S > sqrt(2 (R + W - s) (h + s) + (h + s)^2) + sqrt(2 (R + W - s) (A + s) + (A + s)^2)`.
Height:
- The node SHALL be cut when the lower height bound of its tile is at or above `W + A + 4 eps_f32 max(1, |W|, |bound|)`.
- The bound is `HeightField::height_bounds` at the tile's centre direction and angular radius, the values of the frustum cut; the morph's heights lie inside it.
- The node SHALL also be cut when `m` is at or above `W + A + 4 eps_f32 max(1, |W|, |m|)`.
- `m` SHALL be computed in the tile's worker job from the mesh it built (the vertex format's vertices, the compact format's lowest texel; a pre-filled tile's likewise) and kept in the `WaterBook` beside a wet entry of the tile (an f32 of RAM, not stored, `MESH_VERSION` and every key unchanged); it is a pure function of the tile, the field and the tile's format (CON-14; the compact format's differs from the vertex format's by at most one height step). A cancelled job and a tile whose job did not run (a root, a tile not yet loaded) hold none, and the plan SHALL read it from the book without computing it and keep the node by the loose bound's verdict until the book holds it.
Result:
- The cut SHALL be a pure function of the camera, the tile, `W`, the look and the book's content: the same for every thread count, cache state and order of questions.
- The kept nodes SHALL be the uncut plan's nodes bit for bit, in the same order; a plan whose nodes are all cut is a water plan with no node.
- The CPU-ray count of sea pixels inside water nodes SHALL be equal for the cut and the uncut plan, and the class view and the lit view pixel-identical.
- The cache SHALL count the nodes cut by the horizon (`cut_horizon`), by the height tests (`cut_height`) and, of those, by `m` (`cut_exact`); recorded only.

Class (ADR 0035): plan data (no GPU twin, no stored data). Bound: exact; `m` is the minimum of the f32 heights the vertex stage reads, so the f32 rounding of the vertex stage's `h_origin + hr` and of the water pass's `W - h` is inside the 4 eps margin.
Known limit: the blocking surface is the sphere `R + W` less `s`, so dry ground below `W` between the camera and the tile (a valley under a lake, a dry basin below the sea) can hide nothing and the tile is cut where it is visible; the waves' troughs are taken to be hidden by their crests; skirts add `skirt depth^2 / (2 c)` to a tile's reach when `S` is 0. The terrain between the mesh's vertices is the mesh's, not the field's: a field detail below the lattice spacing that dips under `W` between dry vertices is not drawn and not covered. A node whose tile has no minimum yet is looked up in the book on every plan (a lock shared with the jobs, no computation) until its job leaves one. Reduced coverage: the tier B scenario of the four coast views samples frames 0, 120 and 239 of the glide and of the descent (not 0, 60, 120, 180 and 239), to keep the test near 16 s; the tier C pictures cover the exact cut at four views only.

Verify: A, B, C
Status: active
Source: ADR 0056, ADR 0058, ATMO-012, ATMO-016, ATMO-019, docs/perf/water-attribution.md, CON-11

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
- **WHEN** the jobs of wet and dry tiles run in both tile formats over a world with water
- **THEN** a wet tile's minimum equals the minimum recomputed from the mesh it built, a dry or cancelled tile has none, and the minimum of tiles of levels 3 to 20 bounds every vertex height, morphed height and skirt vertex height of the tile, in the vertex and compact formats over the reference and the steep field

#### Scenario: Pictures
- **WHEN** an aerial, a low, a beach and an inland view of the 1,024-cell world are drawn in the class view and the lit view with the full selection and with the exact cut
- **THEN** the HDR frames are equal, and the exact cut keeps fewer nodes than the loose cut over the four views together

#### Scenario: Pre-filled tiles
- **WHEN** the 30 pre-filled tiles of levels 0 and 1 of a stored bake of 256 cells are read from the pack and their jobs fill the water, and orbital plans over the store are made with the book
- **THEN** a wet tile's stored minimum equals the minimum of the pre-filled tile (which is the generated tile bit for bit), a dry tile has none, and the plan keeps exactly the nodes whose stored minimum is below the bound
