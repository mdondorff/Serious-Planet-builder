## ADDED Requirements

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
