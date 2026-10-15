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
The morph factor of a node SHALL be a continuous, monotone function of the distance metric used for selection (measured to the parent's bounding volume, LOD-008), with value 0 until the last `morph_band` of the parent's split distance and 1 at that distance, so that a merge never pops.

Verify: A
Status: active
Source: report §2 (CDLOD), ADR 0016

#### Scenario: Distance sweep
- **WHEN** the camera moves away from a node in small steps
- **THEN** its morph factor never decreases and changes by at most the step bound

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
Selection for a camera SHALL additionally drop nodes entirely outside the side planes of its view frustum and SHALL still hold every in-view point of the reference sphere in exactly one leaf; the camera SHALL use a reversed-Z, infinite-far projection whose depth is 1 at the near plane and decreases with distance. The test SHALL use each node's own bounding volume (its height bounds, LOD-008, enlarged by the depth of its skirts) rather than a pad by the global height maximum, and SHALL never drop a node that owns a visible pixel (a pure frustum test: no occlusion culling, CON-03).

Verify: A
Status: active
Source: CON-03, CON-12, ADR 0002, LOD-008, M4 item 7c

#### Scenario: Smaller than the hemisphere
- **WHEN** a scripted ground or aerial view is selected with and without the frustum
- **THEN** the frustum selection has fewer nodes and still covers every ray through the image that reaches the sphere

#### Scenario: Tight cut
- **WHEN** the ten standard views are planned at the production parameters and rendered through a CPU depth pass (tier B, `oracle_frustum_cut.rs`; the vertex-per-plane and count checks are tier A, `frustum_cut.rs`)
- **THEN** every tile with a visible triangle is in the plan, no vertex of a dropped tile lies inside the frustum, and the ground-1m plan holds fewer than half the nodes the global pad kept

### Requirement: LOD-008 Height-aware selection
Node selection SHALL measure distances to per-node bounding volumes that include the node's height bounds instead of the reference sphere. The morph interval of a node SHALL end at the split distance of its parent and start no earlier than its own split distance plus twice the largest bounding radius among its same-level neighbours, equalised over the leaves of a level (shared border vertices morph identically), and vertex distances SHALL be measured to the vertices' real positions, so that in every scripted view, on the reference and the steep stress field (TEST-015), the camera stays above the drawn surface by at least the near plane, no drawn surface comes nearer than the near plane, and the leaf under the camera meets `tau_px` (or is at the maximum level). A request whose terrain leaves no crack-free morph interval for a node (start beyond 95 % of end) SHALL be rejected with the tile, the interval and the parameters to change.

Verify: A
Status: active
Source: ADR 0014, ADR 0016, TEST-015, CON-13

#### Scenario: Steep ground at 1 m
- **WHEN** the steep ground-1m view is planned
- **THEN** the drawn surface under the camera lies below it by at least the near plane, the nearest drawn surface is no nearer than the near plane, and the leaf there projects its sample spacing to at most `tau_px` at the camera's height above the terrain (or is at the maximum level)

#### Scenario: Too steep for the parameters
- **WHEN** a plan is requested over terrain whose height extent at some tile size exceeds what the split distance leaves room for
- **THEN** the plan is rejected with the tile, its interval and the parameters to raise or reduce, never planned with a narrowed interval

### Requirement: LOD-009 Plan reuse across frames
The plan builder SHALL reuse a tile's origin height and mesh key across plans of one cache, so that a warm plan of unchanged tiles evaluates the height field zero times. A field that reads authored cells and a non-finite origin height SHALL never be reused. A change to the face mapping, the field's identity (including its reference surface), the radius or (for mesh keys) the cell count SHALL start the reused values over, and every plan SHALL equal the plan of a fresh cache.

Verify: A
Status: active
Source: LOD-006, LOD-008, M4 item 7a, ADR 0018

#### Scenario: Warm plan
- **WHEN** a plan is repeated with the same cache and the same tiles
- **THEN** the field is evaluated zero times and the plan equals the first plan and the plan of a fresh cache

#### Scenario: Moved camera
- **WHEN** the camera moves and some tiles are new
- **THEN** only the new tiles evaluate the field

#### Scenario: Grid size changes
- **WHEN** the cell count changes between plans
- **THEN** the mesh keys are those of the new cell count

#### Scenario: Fields that read cells
- **WHEN** the field reads authored cells (`reads_cells`)
- **THEN** nothing is reused, because an edit changes keys and heights without changing the field's identity
