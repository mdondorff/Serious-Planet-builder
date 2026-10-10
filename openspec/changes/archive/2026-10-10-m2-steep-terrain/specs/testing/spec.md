## ADDED Requirements

### Requirement: TEST-015 Steep-terrain stress
A test-only steep height field (`generators::stress::Steep`: ridged, about 6 km of relief, slopes above 27 degrees over 20 m, keys salted so it never aliases reference meshes) SHALL pass the crack, morph, border and precision invariants of the reference terrain: bit-identical border positions and normals across every cube edge, fully morphed borders on the coarser neighbour's border, morph closing every level transition in the ten scripted views, vertices within 1 mm and within 0.25 px at 1 m, and no holes in the ten scripted views on the GPU. The reference field through the field-generic paths SHALL be bit-identical to the existing paths.

Verify: C
Status: active
Source: CON-13, ADR 0003, ADR 0014, M2 report (SPIKE-02 caveat)

#### Scenario: Steep invariants
- **WHEN** the tier-A and tier-C checks run on the steep field
- **THEN** they pass with the same tolerances as on the reference field, and the ten views are written to `target/review/steep/<adapter>/` for review

#### Scenario: Reference unchanged
- **WHEN** a mesh, mesh key, plan or scripted view is built through the field-generic path with the reference field
- **THEN** it equals the existing path bit for bit (existing known answers and snapshots unchanged)
