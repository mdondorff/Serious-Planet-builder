## MODIFIED Requirements

### Requirement: LOD-008 Height-aware selection
Node selection SHALL measure distances to per-node bounding volumes that include the node's height bounds instead of the reference sphere, and the morph interval and the vertex morph distance SHALL use the same volumes and the vertex's real position, so that in every scripted view, on the reference and the steep stress field (TEST-015), the camera stays above the drawn surface by at least the near plane and the leaf under the camera meets `tau_px`.

Verify: A
Status: active
Source: ADR 0014, ADR 0016, TEST-015, CON-13

#### Scenario: Steep ground at 1 m
- **WHEN** the steep ground-1m view is planned
- **THEN** the drawn surface under the camera lies below it by at least the near plane, and the leaf there is as fine as on flat terrain at 1 m

#### Scenario: Bounds are sound
- **WHEN** random points inside random caps are evaluated
- **THEN** every height lies inside the cap's bounds
