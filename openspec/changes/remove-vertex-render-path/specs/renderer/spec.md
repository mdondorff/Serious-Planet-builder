## ADDED Requirements

### Requirement: REND-031 Reference renderer
`render` SHALL offer a test-only reference renderer (`render::reference`, behind the cargo feature `reference-renderer`, ADR 0062 point 1) that draws a `TerrainPlan` from CPU `TileMesh`es in the debug views, with one pipeline and one vertex buffer per tile, so that the compact renderer's image tests keep an independent geometry source. It SHALL have no pools, no fallback, no lit path, no atmosphere and no water, and SHALL refuse a plan with a lit part. It SHALL NOT be a second production path: the feature is off by default and no product crate and no non-dev dependency edge enables it (BUILD-001).

Verify: C
Status: planned
Source: ADR 0062, SPIKE-12 Q4, REND-016

#### Scenario: Reference against the compact renderer
- **WHEN** the ten scripted views are drawn in the level, relief and normals views, once by the reference renderer from the meshes and once by the compact renderer from the same tiles
- **THEN** at most the tolerance of REND-016 of the pixels differ (the model error of the compact base surface), and the compact path is exercised on more than 100 tiles

#### Scenario: Lit plan refused
- **WHEN** a plan with a lit part is given to the reference renderer
- **THEN** it fails with an error naming the missing lit path, and draws nothing
