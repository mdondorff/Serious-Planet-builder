## ADDED Requirements

### Requirement: LOD-008 Height-aware selection
Node selection SHALL measure distances to per-node height bounds instead of the reference sphere, so that in every scripted view, on the reference and the steep stress field (TEST-015), the camera stays above the drawn surface by at least the near plane and the leaf under the camera meets `tau_px`.

Verify: A
Status: planned
Source: ADR 0014, TEST-015, CON-13

#### Scenario: Steep ground at 1 m
- **WHEN** the steep ground-1m view is planned (today: terrain 687.4 m, camera 688.4 m, level-16 leaf drawn fully morphed to the level-15 grid, nearest vertex 689.5 m)
- **THEN** the drawn surface under the camera lies below it by at least the near plane, and the leaf there is as fine as on flat terrain at 1 m
