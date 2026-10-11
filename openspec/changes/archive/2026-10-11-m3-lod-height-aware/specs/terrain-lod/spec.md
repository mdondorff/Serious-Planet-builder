## MODIFIED Requirements

### Requirement: LOD-004 Morph continuity
The morph factor of a node SHALL be a continuous, monotone function of the distance metric used for selection (measured to the parent's bounding volume, LOD-008), with value 0 until the last `morph_band` of the parent's split distance and 1 at that distance, so that a merge never pops.

Verify: A
Status: active
Source: report §2 (CDLOD), ADR 0016

#### Scenario: Distance sweep
- **WHEN** the camera moves away from a node in small steps
- **THEN** its morph factor never decreases and changes by at most the step bound

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
