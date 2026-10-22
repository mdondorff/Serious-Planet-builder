# terrain-lod Specification

## Purpose
Choosing what terrain to draw: CPU quadtree node selection as a pure, GPU-free function of camera, parameters and tree (CON-03, ADR 0009), complete coverage and crack-free morphing (ADRs 0014, 0016), height-aware selection with proven height bounds (LOD-008), frustum selection against the reversed-Z camera (CON-12, ADR 0002), the plan builder that turns a selection into FramePlan draws (CON-11), and its reuse of per-tile origin heights and mesh keys across frames (LOD-009).

## Requirements

### Requirement: LOD-001 CPU node selection
Terrain node selection (which quadtree nodes at which level, with morph factors) SHALL be a pure function of camera, parameters and the tree, implemented in GPU-free code, and its result SHALL be plain data (`NodeSelection`) that the plan builder consumes; terrain draws in the FramePlan arrive with the terrain render change.

Verify: A
Status: active
Source: CON-03, ADR 0009

#### Scenario: Determinism
- **WHEN** selection runs twice with the same camera and parameters
- **THEN** the node lists and morph factors are identical

### Requirement: LOD-002 Selection snapshots
Node selection for a fixed set of scripted views SHALL be snapshot-tested (node count, per-level counts and a hash over node IDs and morph bits per view), and a snapshot change SHALL be reported per view with the old and new values.

Verify: A
Status: active
Source: CON-03, report §16

#### Scenario: Behaviour change
- **WHEN** the selection function changes its output for a scripted view
- **THEN** the snapshot test fails and prints, per changed view, the node count, per-level counts and hash now and in the snapshot

### Requirement: LOD-003 Complete coverage
The selected nodes SHALL cover every surface point visible over the horizon exactly once, without gaps or overlaps (nodes entirely below the horizon, padded by the maximum terrain height, MAY be dropped), and neighbouring selected nodes SHALL differ by at most one level.

Verify: A
Status: active
Source: report §2

#### Scenario: Partition
- **WHEN** a deterministic sample of surface directions in view is tested against the selection
- **THEN** each direction lies in exactly one selected node

### Requirement: LOD-004 Morph continuity
The morph factor of a node SHALL be a continuous, monotone function of the distance metric used for selection (measured to the parent's bounding volume, LOD-008), with value 0 until the last `morph_band` of the parent's split distance and 1 at that distance, so that a merge never pops; a leaf at the render floor (LOD-011) has no drawn parent and its morph factor is 0 at every distance.

Verify: A
Status: active
Source: report §2 (CDLOD), ADR 0016, LOD-011, ADR 0062

#### Scenario: Distance sweep
- **WHEN** the camera moves away from a node in small steps
- **THEN** its morph factor never decreases and changes by at most the step bound

#### Scenario: Floor leaf
- **WHEN** a leaf at the render floor is selected at every distance of a sweep
- **THEN** its morph factor is 0 throughout and its morph interval is `[1e30, 1e30]`

### Requirement: LOD-005 No cracks in debug views
In the tile-ID debug view of each scripted view, every pixel whose view ray hits the sphere just below the lowest terrain (reduced by the sagitta of the coarsest drawn mesh) SHALL show terrain, never the background.

Verify: C
Status: active
Source: report §16

#### Scenario: Ten scripted views
- **WHEN** the ten scripted views are rendered in the tile-ID debug view
- **THEN** the hole-pixel count of each is zero

### Requirement: LOD-006 Plan builder
The GPU-free plan builder SHALL lay tiles out in pixel rectangles, compute each camera-relative origin as the float64 difference rounded once to float32, place tile origins on the reference sphere, give distinct stable colours to distinct tile IDs, and reject unusable requests (no tiles, unknown face mapping, frame too small) with a message.

Verify: A
Status: active
Source: CON-03, CON-11, ADR 0001

#### Scenario: Two tiles
- **WHEN** two tiles are planned into a 256 by 128 frame
- **THEN** their rectangles are [0,0,128,128] and [128,0,128,128] and each camera-relative origin equals the f64 difference rounded once

### Requirement: LOD-007 Frustum selection
Selection for a camera SHALL additionally drop nodes entirely outside the side planes of its view frustum and SHALL still hold every in-view point of the reference sphere in exactly one leaf; the camera SHALL use a reversed-Z, infinite-far projection whose depth is 1 at the near plane and decreases with distance. The test SHALL use each node's own bounding volume (its height bounds, LOD-008, enlarged by the depth of its skirts) rather than a pad by the global height maximum, and SHALL never drop a node that owns a visible pixel (a pure frustum test: no occlusion culling, CON-03). The cover of the resident set (STRM-016) SHALL apply the same horizon, frustum and tight cut to the tiles of its walk, from the bounds the plan cached, so a node the plan drops the cover drops too.

Verify: A
Status: active
Source: CON-03, CON-12, ADR 0002, LOD-008, M4 item 7c, STRM-016

#### Scenario: Smaller than the hemisphere
- **WHEN** a scripted ground or aerial view is selected with and without the frustum
- **THEN** the frustum selection has fewer nodes and still covers every ray through the image that reaches the sphere

#### Scenario: Tight cut
- **WHEN** the ten standard views are planned at the production parameters and rendered through a CPU depth pass (tier B, `oracle_frustum_cut.rs`; the vertex-per-plane and count checks are tier A, `frustum_cut.rs`, which checks every eighth dropped tile of every camera; `oracle_frustum_cut_sweep.rs` (tier B) checks every second one on the full set of cameras)
- **THEN** every tile with a visible triangle is in the plan, no vertex of a dropped tile lies inside the frustum, and the ground-1m plan holds fewer than half the nodes the global pad kept

#### Scenario: The cover cuts like the plan
- **WHEN** R of a position is resident and the cover is made for the ten standard views
- **THEN** it holds the plan's nodes and no tile that the horizon, the frustum or the tight cut drops

### Requirement: LOD-008 Height-aware selection
Node selection SHALL measure distances to per-node bounding volumes that include the node's height bounds instead of the reference sphere. The morph interval of a node SHALL end at the split distance of its parent and start no earlier than its own split distance plus twice the largest bounding radius among its same-level neighbours, equalised over the leaves of a level (shared border vertices morph identically), and vertex distances SHALL be measured to the vertices' real positions, so that in every scripted view, on the reference and the steep stress field (TEST-015), the camera stays above the drawn surface by at least the near plane, no drawn surface comes nearer than the near plane, and the leaf under the camera meets `tau_px` (or is at the maximum level). A tile is morphable when its morph interval has room (`morph_room`: finite, and starting within 95 % of its end; the one predicate of `LodParams::validate`, the selection and the plan's backstop; a bound that is not a number leaves none). A tile above the render floor that is not morphable SHALL NOT be a leaf: the selection SHALL keep from splitting every tile whose children are not all morphable, and the tiles whose parent is such a tile stay under it, so the plan degrades locally and no interval is ever narrowed (ADR 0074).
The ceiling of the leaves SHALL be `min over the unmorphable tiles u (topmost, found on the position-only selection without the frustum, so that R and the plan of any view agree) of (level(parent(u)) + d)`, with `d` the number of tile edges between the level-`level(parent(u))` tiles (across cube faces; for a coarser tile the least over its descendants, for a finer one that of its ancestor). It is 1-Lipschitz across edges: a selection clamped to it stays balanced (2:1), and the result is the finest balanced partition not finer than the uncapped selection with every leaf morphable, whatever the order the tiles are met in.
A leaf that the ceiling kept coarse is a capped node (`SelectedNode::capped`, `TerrainDraw::capped`, a runtime flag not in the plan's text). Where a capped leaf has finer leaves across an edge, the plan SHALL mark that edge of the capped leaf `fine` and the edges of the finer leaves that face it `stitch` (`TerrainDraw::fine`, `TerrainDraw::stitch`, bit `k` the side `Side::ALL[k]`: East, West, North, South; `terrain::edge_masks`), from the plan's leaves alone and never the camera, so that a reused plan stays crack-free; a leaf with no capped neighbour has no mask. A plan with no unmorphable tile is the plan of the selection without the ceiling. The plan SHALL still be rejected with the tile, the interval and the parameters to change where `LodParams::validate` finds parameters with no room at any slope and, as a backstop after the selection, where a leaf above the floor has no room.

Verify: A
Status: active
Source: ADR 0014, ADR 0016, TEST-015, CON-13

#### Scenario: Steep ground at 1 m
- **WHEN** the steep ground-1m view is planned
- **THEN** the drawn surface under the camera lies below it by at least the near plane, the nearest drawn surface is no nearer than the near plane, and the leaf there projects its sample spacing to at most `tau_px` at the camera's height above the terrain (or is at the maximum level)

#### Scenario: Too steep for the parameters
- **WHEN** a plan is requested over terrain whose height extent at some tile size exceeds what the split distance leaves room for (the steep stress field at ground level, a spike of 220 m over 100 m)
- **THEN** the plan is made, the tiles over that ground are capped, every leaf above the floor has a morph interval with room (none narrowed), the leaves are balanced (2:1), and the same request over the reference field has no capped tile

#### Scenario: The edges of a capped leaf
- **WHEN** a plan over a spike has capped leaves with finer leaves across some of their edges
- **THEN** each such edge of the capped leaf is `fine` and the facing edges of the finer leaves are `stitch`, no other leaf has a mask, a plan with no capped leaf has none, and the finer leaves' vertices on a stitched edge, morphed to the parent grid, lie on the capped leaf's edge to a millimetre

#### Scenario: The ceiling
- **WHEN** random sets of unmorphable tiles on a sphere of tiles are turned into a ceiling and the finest tiles are clamped to it, in any order of the set
- **THEN** the leaves tile the sphere with none under another, every leaf is at or under its ceiling, the unmorphable tiles and what is under them are not leaves, the selection is balanced, splitting any coarsened leaf puts a child over its ceiling or unbalances it, and the order of the set changes nothing

#### Scenario: Bounds that are not a number
- **WHEN** the height bounds of a field are not numbers near a point and a plan is made there
- **THEN** no leaf is kept with a morph interval without room, and the tiles whose neighbours' bounds are not numbers are capped

### Requirement: LOD-009 Plan reuse across frames
The plan builder SHALL reuse a tile's origin height and mesh key across plans of one cache, so that a warm plan of unchanged tiles evaluates the height field zero times and derives no mesh key. It SHALL evaluate the origin heights of the tiles its cache lacks as one batch (`HeightField::heights_threaded`, GEN-020), after the selection and before the plan is assembled, so that a field over a block store reads each block the batch needs once, however small its cache and however many plan threads there are, instead of once per cache miss per node; a plan with nothing new SHALL make no batch. The plan SHALL be bit for bit the plan of the per-node evaluation (every origin is `height(centre)` of its tile) at every thread count. A field that reads authored cells and a non-finite origin height SHALL never be reused. A change to the face mapping, the render floor `min_level` (LOD-011), the field's identity (including its reference surface), the radius or (for mesh keys) the cell count or the finest level (the key carries the finest-level class of a tile, GEN-010) SHALL start the reused values over, and every plan SHALL equal the plan of a fresh cache.

Verify: A
Status: active
Source: LOD-006, LOD-008, M4 item 7a, ADR 0018, LOD-010, M4 fix F3 (perf window 1), GEN-020, LOD-011

#### Scenario: Warm plan
- **WHEN** a plan is repeated with the same cache and the same tiles
- **THEN** the field is evaluated zero times, no batch is made, no mesh key is derived, and the plan equals the first plan and the plan of a fresh cache

#### Scenario: Moved camera
- **WHEN** the camera moves and some tiles are new
- **THEN** only the new tiles evaluate the field and derive a mesh key, in one batch of exactly the new tiles

#### Scenario: Cold plan at altitude
- **WHEN** a cold plan at 20,000 km, 5,000 km, 2,000 km and 500 km above a bake of many blocks is made over a source with a cache of one block, at 1 and 20 threads
- **THEN** no origin height is evaluated by a single `height` call, the origins are one batch of one direction per node, and the batch decodes at most the distinct blocks its directions touch

#### Scenario: Batch equals per-node evaluation
- **WHEN** the ten standard views and the dive's cameras at five altitudes from 20,000 km to 200 km are planned at 1 and 20 threads (tier A: every third camera and the cube-corner and pole views at 1 and 3 threads; the full sweep is tier B, `oracle_plan_origin_batch`)
- **THEN** every plan is equal and the origin of every node is bit for bit `height` of its tile's centre

#### Scenario: Grid size changes
- **WHEN** the cell count changes between plans
- **THEN** the mesh keys are those of the new cell count

#### Scenario: Finest level changes
- **WHEN** the finest level changes between plans of one cache
- **THEN** the mesh keys are those of the new level, equal to the keys of a fresh cache

#### Scenario: Fields that read cells
- **WHEN** the field reads authored cells (`reads_cells`)
- **THEN** nothing is reused, because an edit changes keys and heights without changing the field's identity

### Requirement: LOD-010 Finest level
The finest geometry level SHALL be one LOD parameter, `LodParams::max_level`, read by node selection, the residency set R, the prefetch set (the set R of a predicted position), the cover plan and the draw set, with no second constant (there is no relief guard at the cap since the vertex fallback is gone, ADR 0062). Its production default for tiles of `cells` cells SHALL be `27 - ceil(log2(cells))` (`2^level * cells = 2^27`, so equal vertex spacing of about 7.46 cm at Earth's radius whatever the tile size), never deeper than the reference level 26: 22 at cells 32, 21 at cells 64, 23 at cells 16 (ADR 0033 point 6, owner 2026-10-15); `LodParams::earth_1080p()` SHALL carry it for cells 32. The level SHALL stay settable (`--finest-level N` on the test-render terrain paths, or the parameter in a test) so that the reference level 26 reproduces the earlier node counts, and a request SHALL carry it in its repro bundle (`max_level`). No tau exception is made at the cap: the facets of 7.5 cm at 1 m are inside the pixel tolerance, and height queries use the field, never the mesh. The detail term below the finest level is not part of this requirement (deferred).

Verify: A
Status: active
Source: ADR 0033 points 5-6, SPIKE-07, CON-13, LOD-008

#### Scenario: Production default
- **WHEN** the production finest level is asked for cells 8, 16, 32 and 64
- **THEN** it is 24, 23, 22 and 21, the sample spacing at that level is equal for all (7.46 cm), and the default LOD parameters carry 22

#### Scenario: One cap everywhere
- **WHEN** the 1 m ground view is selected, its R, the prefetch set of a position ahead and a cover plan over a fully resident tree are built with the cap 22, 23 and 24
- **THEN** the draw set and R reach the cap and none of the four goes deeper

#### Scenario: Reference level
- **WHEN** the standard views are planned at cap 22 and at the reference 26
- **THEN** only ground-1m differs (its pin at 26 is kept beside the production pin)

### Requirement: LOD-011 Render floor
Node selection SHALL have a render floor, `LodParams::min_level` (production default 3, ADR 0062 point 3; 0 reproduces the earlier selection, for CPU tests only: the renderer refuses a tile below level 3, REND-016; settable by `--floor-level N` on the `test-render` terrain paths and in a test; carried in a repro bundle as `min_level`, like `max_level`, LOD-010). Every node of a level below the floor SHALL be split whatever the distance, so no tile below the floor is selected, drawn or a member of D, R, the prefetch set or the cover. A leaf at the floor is a root of the draw set: it has no drawn parent, its morph factor is 0 and its morph interval is `[1e30, 1e30]`, so its vertices are never moved to a parent grid that is not drawn (LOD-004), and the plan builder's check of an empty morph interval SHALL exempt leaves at the floor and only those. Culling (horizon, frustum, height-aware bounds, LOD-007, LOD-008) and the 2:1 balance (LOD-003) are unchanged. Wherever the specs say a root or a level-0 tile in a residency or cover sense (STRM-001, STRM-007, STRM-015, STRM-016), they mean a tile at the floor level: at floor 3 there are 384 such tiles. The floor SHALL NOT change the tiles of a view that draws no node above the floor: with floor 3 and with floor 0 nine of the ten standard views at 2560 x 1440 (tau 6, cells 32) draw the same tiles with the same mesh keys (11,661 nodes in total), every node above the floor equal in every field and every leaf at the floor equal but for having no morph (morph 0 and the interval `[1e30, 1e30]`, where floor 0 morphs it to its parent), and orbit-20000km changes from 52 level-2 nodes to 181 level-3 nodes (182 over the default world's eroded field). A view drawn with a small tolerance or viewport may also change where it draws a few level-2 nodes at its edge: the 128 x 96 snapshot view high-2000km (tau 2.5, cells 16) draws 71 nodes with floor 3 instead of 73 (three level-2 nodes whose children are all outside the view become one level-3 node). A camera beyond the standard orbit draws at most 384 level-3 tiles. The effective floor is `min(min_level, max_level)`, so a body or scene with `max_level` 0 to 2 draws down to its finest level without any flag; only an explicit `--floor-level` above `max_level` SHALL be refused with an error naming both values. The viewer takes no flag; `test-render` and `generate` do.

Verify: A, C
Status: active
Source: ADR 0062 point 3, SPIKE-12 Q1 option (b), LOD-003, LOD-004, LOD-007, LOD-008, LOD-010

#### Scenario: Levels below the floor are never selected
- **WHEN** the ten standard views and a camera at 100,000 km are selected with floor 3
- **THEN** no node, D, R, prefetch or cover tile has a level below 3, the 100,000 km view draws at most 384 nodes, orbit-20000km draws 181 level-3 nodes (R is the same 181 tiles, with no ancestor), and the other nine views draw the tiles of floor 0 (11,661 nodes) with every node above the floor equal to the floor-0 node

#### Scenario: Floor leaves do not morph
- **WHEN** the orbit view is planned with floor 3
- **THEN** every floor leaf has morph factor 0 and the interval `[1e30, 1e30]`, the plan builder accepts it, and the same interval on a leaf above the floor is still refused

#### Scenario: Small finest level needs no flag
- **WHEN** a scene with `max_level` 2 is planned with the default floor
- **THEN** the effective floor is 2 (and 0 and 1 for a `max_level` of 0 and 1), the plan draws the tiles of the floor-0 plan of that scene (its level-2 leaves without a morph), and nothing is refused

#### Scenario: Explicit floor above the finest level
- **WHEN** `--floor-level 5 --finest-level 4` is given
- **THEN** it is refused before any work with an error naming 5 and 4

#### Scenario: Floor zero reproduces the earlier plans
- **WHEN** the ten standard views and the three view variants of the plan snapshot are planned with floor 0
- **THEN** the node counts and plan hashes equal those recorded before the change (`terrain_plans_floor0.txt`)

#### Scenario: The orbit view with the floor
- **WHEN** the orbit-20000km views (level, relief and normals) are drawn from floor-3 tiles on the software adapter
- **THEN** the picture covers the whole planet without a hole or a crack, floor leaves show no morph, and the goldens that change by this declaration (tile ids, levels, geometry) are listed in the change and go to the owner's `/bless` (ADR 0062 consequences)

### Requirement: LOD-012 Selection over nested bounds
A height field MAY say that its bounds nest (`HeightField::bounds_monotone`, default no): for a cap inside another (the angle between the centres plus the inner radius at most the outer radius) the bounds of the inner lie inside those of the outer widened by `MONOTONE_SLACK_M` (0.01 m) on each side, and are numbers where the outer's are. A wrapper says yes only if the field below it does and its own addition keeps it so; every field that says yes SHALL have a contract test over random nested caps.
Over such a field the node selection MAY skip the ceiling work of LOD-008 where it has proved that no tile of the position-only selection can be too steep to morph. The proof SHALL cover, for every level that reads its neighbours' bounds, every tile the selection can hold (a tile split by distance, or one balancing made, which lies within the split distance of the camera plus the radius of the volume of its parent), with the bounds of caps that hold the volumes of those tiles and of the same-level neighbours their morph intervals read, and SHALL test each level's interval on those bounds as `morph_interval_for` tests it. The proof SHALL fail, and the selection take the full path, whenever a bound is not a number, a cap is too wide for the interval, or the work exceeds its budget. The result SHALL be the selection of the full path, node for node and ceiling for ceiling, on every field and camera, and an edit SHALL restart every cached bound and certificate unless STRM-034 keeps it.

Verify: A
Status: active
Source: ADR 0074, LOD-008, perf window 22

#### Scenario: Nested bounds
- **WHEN** random caps and random caps inside them are asked of every field that says its bounds nest
- **THEN** the bounds of the inner cap lie inside those of the outer within the slack, and a field that does not say so, or a stage over one, does not say so

#### Scenario: The same selection
- **WHEN** random steep and gentle spike fields are selected from random cameras over a field that says its bounds nest and over the same field without saying so
- **THEN** the nodes and the ceiling are equal, and over gentle ground the first visits fewer tiles
