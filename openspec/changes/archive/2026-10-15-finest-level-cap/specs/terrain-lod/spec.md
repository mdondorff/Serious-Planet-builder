## ADDED Requirements

### Requirement: LOD-010 Finest level
The finest geometry level SHALL be one LOD parameter, `LodParams::max_level`, read by node selection, the residency set R, the prefetch set (the set R of a predicted position), the cover plan, the draw set and the compact format's CON-13 guard (GEN-019), with no second constant. Its production default for tiles of `cells` cells SHALL be `27 - ceil(log2(cells))` (`2^level * cells = 2^27`, so equal vertex spacing of about 7.46 cm at Earth's radius whatever the tile size), never deeper than the reference level 26: 22 at cells 32, 21 at cells 64, 23 at cells 16 (ADR 0033 point 6, owner 2026-10-15); `LodParams::earth_1080p()` SHALL carry it for cells 32. The level SHALL stay settable (`--finest-level N` on the test-render terrain paths, or the parameter in a test) so that the reference level 26 reproduces the earlier node counts, and a request SHALL carry it in its repro bundle (`max_level`). No tau exception is made at the cap: the facets of 7.5 cm at 1 m are inside the pixel tolerance, and height queries use the field, never the mesh. The detail term below the finest level is not part of this requirement (deferred).

Verify: A
Status: active
Source: ADR 0033 points 5-6, SPIKE-07, CON-13, LOD-008

#### Scenario: Production default
- **WHEN** the production finest level is asked for cells 8, 16, 32 and 64
- **THEN** it is 24, 23, 22 and 21, the sample spacing at that level is equal for all (7.46 cm), and the default LOD parameters carry 22

#### Scenario: One cap everywhere
- **WHEN** the 1 m ground view is selected, its R, the prefetch set of a position ahead and a cover plan over a fully resident tree are built with the cap 22, 23 and 24
- **THEN** the draw set and R reach the cap and none of the four goes deeper

#### Scenario: Reference level
- **WHEN** the standard views are planned at cap 22 and at the reference 26
- **THEN** only ground-1m differs (its pin at 26 is kept beside the production pin)

## MODIFIED Requirements

### Requirement: LOD-009 Plan reuse across frames
The plan builder SHALL reuse a tile's origin height and mesh key across plans of one cache, so that a warm plan of unchanged tiles evaluates the height field zero times and derives no mesh key. A field that reads authored cells and a non-finite origin height SHALL never be reused. A change to the face mapping, the field's identity (including its reference surface), the radius or (for mesh keys) the cell count or the finest level (the key carries the finest-level class of a tile, GEN-010) SHALL start the reused values over, and every plan SHALL equal the plan of a fresh cache.

Verify: A
Status: active
Source: LOD-006, LOD-008, M4 item 7a, ADR 0018, LOD-010

#### Scenario: Warm plan
- **WHEN** a plan is repeated with the same cache and the same tiles
- **THEN** the field is evaluated zero times, no mesh key is derived, and the plan equals the first plan and the plan of a fresh cache

#### Scenario: Moved camera
- **WHEN** the camera moves and some tiles are new
- **THEN** only the new tiles evaluate the field and derive a mesh key

#### Scenario: Grid size changes
- **WHEN** the cell count changes between plans
- **THEN** the mesh keys are those of the new cell count

#### Scenario: Finest level changes
- **WHEN** the finest level changes between plans of one cache
- **THEN** the mesh keys are those of the new level, equal to the keys of a fresh cache

#### Scenario: Fields that read cells
- **WHEN** the field reads authored cells (`reads_cells`)
- **THEN** nothing is reused, because an edit changes keys and heights without changing the field's identity
