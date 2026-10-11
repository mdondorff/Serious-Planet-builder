## MODIFIED Requirements

### Requirement: TEST-015 Steep-terrain stress
A test-only steep height field (`generators::stress::Steep`, behind the `stress` feature that only dev-dependencies enable: ridged, `|h| <= 8 km`, about -5.5 to +6 km in practice, 20 m slopes above 27 degrees; its own implementation id and version enter its height and mesh keys) SHALL pass the crack, morph, border and precision invariants of the reference terrain with the reference tolerances (GEN-004, REND-009, LOD-004, LOD-005, LOD-008): border positions and normals of neighbouring tiles across every cube edge, fully morphed borders on the coarser neighbour's border polyline, morph closing every level transition in the ten scripted views, deep-tile vertices within 1 mm, vertices within 0.25 px at 1 m on the leaf that selection picks under the camera, and no holes in the ten scripted views on the GPU. The selection-dependent checks SHALL run with parameters for which a crack-free morph interval exists on this field (`viewport_h_px / tau_px` of about 17 times `cells` or more: cells 4 at 77 and cells 8 at 180 in the tests), and the plan SHALL reject production parameters (cells 32 at 240) on this field with a plan error (LOD-008). Plans with a field whose seed or height bound does not fit the request SHALL be rejected with a plan error. The reference field through the field-generic paths SHALL equal the default paths.

Verify: C
Status: active
Source: CON-11, CON-13, ADR 0003, ADR 0006, ADR 0014, ADR 0016, M2 report (SPIKE-02 caveat)

#### Scenario: Steep invariants
- **WHEN** the tier-A and tier-C checks run on the steep field
- **THEN** they pass with the reference tolerances, and the ten views are written to `target/review/steep/<adapter>/` for review

#### Scenario: Reference unchanged
- **WHEN** a mesh, height or mesh key, plan or scripted view is built through the field-generic path with the reference field
- **THEN** it equals the default path bit for bit (mesh and key compared directly; plans and views through the same snapshots and the determinism manifest of GEN-003)

#### Scenario: Stress field versioned
- **WHEN** the committed stress mesh is generated
- **THEN** its content hash equals the known answer recorded for its version
