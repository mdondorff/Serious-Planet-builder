## MODIFIED Requirements

### Requirement: LOD-009 Plan reuse across frames
The plan builder SHALL reuse a tile's origin height and mesh key across plans of one cache, so that a warm plan of unchanged tiles evaluates the height field zero times and derives no mesh key. It SHALL evaluate the origin heights of the tiles its cache lacks as one batch (`HeightField::heights_threaded`, GEN-020), after the selection and before the plan is assembled, so that a field over a block store reads each block the batch needs once, however small its cache and however many plan threads there are, instead of once per cache miss per node; a plan with nothing new SHALL make no batch. The plan SHALL be bit for bit the plan of the per-node evaluation (every origin is `height(centre)` of its tile) at every thread count. A field that reads authored cells and a non-finite origin height SHALL never be reused. A change to the face mapping, the field's identity (including its reference surface), the radius or (for mesh keys) the cell count or the finest level (the key carries the finest-level class of a tile, GEN-010) SHALL start the reused values over, and every plan SHALL equal the plan of a fresh cache.

Verify: A
Status: active
Source: LOD-006, LOD-008, M4 item 7a, ADR 0018, LOD-010, M4 fix F3 (perf window 1), GEN-020

#### Scenario: Warm plan
- **WHEN** a plan is repeated with the same cache and the same tiles
- **THEN** the field is evaluated zero times, no batch is made, no mesh key is derived, and the plan equals the first plan and the plan of a fresh cache

#### Scenario: Moved camera
- **WHEN** the camera moves and some tiles are new
- **THEN** only the new tiles evaluate the field and derive a mesh key, in one batch of exactly the new tiles

#### Scenario: Cold plan at altitude
- **WHEN** a cold plan at 20,000 km, 5,000 km, 2,000 km and 500 km above a bake of many blocks is made over a source with a cache of one block, at 1 and 20 threads
- **THEN** no origin height is evaluated by a single `height` call, the origins are one batch of one direction per node, and the batch decodes at most the distinct blocks its directions touch

#### Scenario: Batch equals per-node evaluation
- **WHEN** the ten standard views and the dive's cameras at five altitudes from 20,000 km to 200 km are planned at 1 and 20 threads
- **THEN** every plan is equal and the origin of every node is bit for bit `height` of its tile's centre

#### Scenario: Grid size changes
- **WHEN** the cell count changes between plans
- **THEN** the mesh keys are those of the new cell count

#### Scenario: Finest level changes
- **WHEN** the finest level changes between plans of one cache
- **THEN** the mesh keys are those of the new level, equal to the keys of a fresh cache

#### Scenario: Fields that read cells
- **WHEN** the field reads authored cells (`reads_cells`)
- **THEN** nothing is reused, because an edit changes keys and heights without changing the field's identity
