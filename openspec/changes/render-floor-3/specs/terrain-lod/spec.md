## MODIFIED Requirements

### Requirement: LOD-004 Morph continuity
The morph factor of a node SHALL be a continuous, monotone function of the distance metric used for selection (measured to the parent's bounding volume, LOD-008), with value 0 until the last `morph_band` of the parent's split distance and 1 at that distance, so that a merge never pops; a leaf at the render floor (LOD-011) has no drawn parent and its morph factor is 0 at every distance.

Verify: A
Status: planned
Source: report §2 (CDLOD), ADR 0016, LOD-011, ADR 0062

#### Scenario: Distance sweep
- **WHEN** the camera moves away from a node in small steps
- **THEN** its morph factor never decreases and changes by at most the step bound

#### Scenario: Floor leaf
- **WHEN** a leaf at the render floor is selected at every distance of a sweep
- **THEN** its morph factor is 0 throughout and its morph interval is `[1e30, 1e30]`

### Requirement: LOD-009 Plan reuse across frames
The plan builder SHALL reuse a tile's origin height and mesh key across plans of one cache, so that a warm plan of unchanged tiles evaluates the height field zero times and derives no mesh key. It SHALL evaluate the origin heights of the tiles its cache lacks as one batch (`HeightField::heights_threaded`, GEN-020), after the selection and before the plan is assembled, so that a field over a block store reads each block the batch needs once, however small its cache and however many plan threads there are, instead of once per cache miss per node; a plan with nothing new SHALL make no batch. The plan SHALL be bit for bit the plan of the per-node evaluation (every origin is `height(centre)` of its tile) at every thread count. A field that reads authored cells and a non-finite origin height SHALL never be reused. A change to the face mapping, the render floor `min_level` (LOD-011), the field's identity (including its reference surface), the radius or (for mesh keys) the cell count or the finest level (the key carries the finest-level class of a tile, GEN-010) SHALL start the reused values over, and every plan SHALL equal the plan of a fresh cache.

Verify: A
Status: planned
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

## ADDED Requirements

### Requirement: LOD-011 Render floor
Node selection SHALL have a render floor, `LodParams::min_level` (production default 3, ADR 0062 point 3; 0 reproduces the earlier selection; settable by `--floor-level N` on the `test-render` terrain paths and in a test; carried in a repro bundle as `min_level`, like `max_level`, LOD-010). Every node of a level below the floor SHALL be split whatever the distance, so no tile below the floor is selected, drawn or a member of D, R, the prefetch set or the cover. A leaf at the floor is a root of the draw set: it has no drawn parent, its morph factor is 0 and its morph interval is `[1e30, 1e30]`, so its vertices are never moved to a parent grid that is not drawn (LOD-004), and the plan builder's check of an empty morph interval SHALL exempt leaves at the floor and only those. Culling (horizon, frustum, height-aware bounds, LOD-007, LOD-008) and the 2:1 balance (LOD-003) are unchanged. Wherever the specs say a root or a level-0 tile in a residency or cover sense (STRM-001, STRM-007, STRM-015, STRM-016), they mean a tile at the floor level: at floor 3 there are 384 such tiles. The floor SHALL NOT change the plan of a view that draws no node above the floor: with floor 3 and with floor 0 nine of the ten standard views have identical plans node for node (11,661 nodes in total), and orbit-20000km at 2560 x 1440 (tau 6, cells 32) changes from 52 level-2 nodes to 181 level-3 nodes. A camera beyond the standard orbit draws at most 384 level-3 tiles. The effective floor is `min(min_level, max_level)`, so a body or scene with `max_level` 0 to 2 draws down to its finest level without any flag; only an explicit `--floor-level` above `max_level` SHALL be refused with an error naming both values. The viewer takes no flag; `test-render` and `generate` do.

Verify: A, C
Status: planned
Source: ADR 0062 point 3, SPIKE-12 Q1 option (b), LOD-003, LOD-004, LOD-007, LOD-008, LOD-010

#### Scenario: Levels below the floor are never selected
- **WHEN** the ten standard views and a camera at 100,000 km are selected with floor 3
- **THEN** no node, D, R, prefetch or cover tile has a level below 3, the 100,000 km view draws at most 384 nodes, orbit-20000km draws 181 level-3 nodes, and the other nine views equal the plans of floor 0 node for node

#### Scenario: Floor leaves do not morph
- **WHEN** the orbit view is planned with floor 3
- **THEN** every floor leaf has morph factor 0 and the interval `[1e30, 1e30]`, the plan builder accepts it, and the same interval on a leaf above the floor is still refused

#### Scenario: Small finest level needs no flag
- **WHEN** a scene with `max_level` 2 is planned with the default floor
- **THEN** the effective floor is 2, the plan equals the floor-0 plan of that scene, and nothing is refused

#### Scenario: Explicit floor above the finest level
- **WHEN** `--floor-level 5 --finest-level 4` is given
- **THEN** it is refused before any work with an error naming 5 and 4

#### Scenario: Floor zero reproduces the earlier plans
- **WHEN** the ten standard views are planned with floor 0
- **THEN** the plans equal the plans recorded before the change, node for node

#### Scenario: The orbit view with the floor
- **WHEN** the orbit-20000km views (level, relief and normals) are drawn from floor-3 tiles on the software adapter
- **THEN** the picture covers the whole planet without a hole or a crack, floor leaves show no morph, and the goldens that change by this declaration (tile ids, levels, geometry) are listed in the change and go to the owner's `/bless` (ADR 0062 consequences)
