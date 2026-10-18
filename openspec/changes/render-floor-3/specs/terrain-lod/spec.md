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

## ADDED Requirements

### Requirement: LOD-011 Render floor
Node selection SHALL have a render floor, `LodParams::min_level` (production default 3, ADR 0062 point 3; 0 reproduces the earlier selection; settable by `--floor-level N` on the `test-render` terrain paths and in a test; carried in a repro bundle as `min_level`, like `max_level`, LOD-010). Every node of a level below the floor SHALL be split whatever the distance, so no tile below the floor is selected, drawn or a member of D, R, the prefetch set or the cover. A leaf at the floor is a root of the draw set: it has no drawn parent, its morph factor is 0 and its morph interval is `[1e30, 1e30]`, so its vertices are never moved to a parent grid that is not drawn (LOD-004), and the plan builder's check of an empty morph interval SHALL exempt leaves at the floor and only those. Culling (horizon, frustum, height-aware bounds, LOD-007, LOD-008) and the 2:1 balance (LOD-003) are unchanged. Wherever the specs say a root or a level-0 tile in a residency or cover sense (STRM-001, STRM-007, STRM-015, STRM-016), they mean a tile at the floor level: at floor 3 there are 384 such tiles. The floor SHALL NOT change the plan of a view that draws no node above the floor: with floor 3 and with floor 0 nine of the ten standard views have identical plans node for node (11,661 nodes in total), and orbit-20000km at 2560 x 1440 (tau 6, cells 32) changes from 52 level-2 nodes to 181 level-3 nodes. A camera beyond the standard orbit draws at most 384 level-3 tiles.

Verify: A, C
Status: planned
Source: ADR 0062 point 3, SPIKE-12 Q1 option (b), LOD-003, LOD-004, LOD-007, LOD-008, LOD-010

#### Scenario: Levels below the floor are never selected
- **WHEN** the ten standard views and a camera at 100,000 km are selected with floor 3
- **THEN** no node, D, R, prefetch or cover tile has a level below 3, the 100,000 km view draws at most 384 nodes, orbit-20000km draws 181 level-3 nodes, and the other nine views equal the plans of floor 0 node for node

#### Scenario: Floor leaves do not morph
- **WHEN** the orbit view is planned with floor 3
- **THEN** every floor leaf has morph factor 0 and the interval `[1e30, 1e30]`, the plan builder accepts it, and the same interval on a leaf above the floor is still refused

#### Scenario: Floor zero reproduces the earlier plans
- **WHEN** the ten standard views are planned with floor 0
- **THEN** the plans equal the plans recorded before the change, node for node

#### Scenario: The orbit view with the floor
- **WHEN** the orbit-20000km views (level, relief and normals) are drawn from floor-3 tiles on the software adapter
- **THEN** the picture covers the whole planet without a hole or a crack, floor leaves show no morph, and the goldens that change by this declaration (tile ids, levels, geometry) are listed in the change and go to the owner's `/bless` (ADR 0062 consequences)
