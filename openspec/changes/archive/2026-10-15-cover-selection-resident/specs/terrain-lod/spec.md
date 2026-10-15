## MODIFIED Requirements

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
