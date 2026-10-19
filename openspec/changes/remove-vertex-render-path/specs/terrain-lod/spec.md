## MODIFIED Requirements

### Requirement: LOD-010 Finest level
The finest geometry level SHALL be one LOD parameter, `LodParams::max_level`, read by node selection, the residency set R, the prefetch set (the set R of a predicted position), the cover plan and the draw set, with no second constant (there is no relief guard at the cap since the vertex fallback is gone, ADR 0062). Its production default for tiles of `cells` cells SHALL be `27 - ceil(log2(cells))` (`2^level * cells = 2^27`, so equal vertex spacing of about 7.46 cm at Earth's radius whatever the tile size), never deeper than the reference level 26: 22 at cells 32, 21 at cells 64, 23 at cells 16 (ADR 0033 point 6, owner 2026-10-15); `LodParams::earth_1080p()` SHALL carry it for cells 32. The level SHALL stay settable (`--finest-level N` on the test-render terrain paths, or the parameter in a test) so that the reference level 26 reproduces the earlier node counts, and a request SHALL carry it in its repro bundle (`max_level`). No tau exception is made at the cap: the facets of 7.5 cm at 1 m are inside the pixel tolerance, and height queries use the field, never the mesh. The detail term below the finest level is not part of this requirement (deferred).

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

### Requirement: LOD-011 Render floor
Node selection SHALL have a render floor, `LodParams::min_level` (production default 3, ADR 0062 point 3; 0 reproduces the earlier selection, for CPU tests only: the renderer refuses a tile below level 3, REND-016; settable by `--floor-level N` on the `test-render` terrain paths and in a test; carried in a repro bundle as `min_level`, like `max_level`, LOD-010). Every node of a level below the floor SHALL be split whatever the distance, so no tile below the floor is selected, drawn or a member of D, R, the prefetch set or the cover. A leaf at the floor is a root of the draw set: it has no drawn parent, its morph factor is 0 and its morph interval is `[1e30, 1e30]`, so its vertices are never moved to a parent grid that is not drawn (LOD-004), and the plan builder's check of an empty morph interval SHALL exempt leaves at the floor and only those. Culling (horizon, frustum, height-aware bounds, LOD-007, LOD-008) and the 2:1 balance (LOD-003) are unchanged. Wherever the specs say a root or a level-0 tile in a residency or cover sense (STRM-001, STRM-007, STRM-015, STRM-016), they mean a tile at the floor level: at floor 3 there are 384 such tiles. The floor SHALL NOT change the tiles of a view that draws no node above the floor: with floor 3 and with floor 0 nine of the ten standard views at 2560 x 1440 (tau 6, cells 32) draw the same tiles with the same mesh keys (11,661 nodes in total), every node above the floor equal in every field and every leaf at the floor equal but for having no morph (morph 0 and the interval `[1e30, 1e30]`, where floor 0 morphs it to its parent), and orbit-20000km changes from 52 level-2 nodes to 181 level-3 nodes (182 over the default world's eroded field). A view drawn with a small tolerance or viewport may also change where it draws a few level-2 nodes at its edge: the 128 x 96 snapshot view high-2000km (tau 2.5, cells 16) draws 71 nodes with floor 3 instead of 73 (three level-2 nodes whose children are all outside the view become one level-3 node). A camera beyond the standard orbit draws at most 384 level-3 tiles. The effective floor is `min(min_level, max_level)`, so a body or scene with `max_level` 0 to 2 draws down to its finest level without any flag; only an explicit `--floor-level` above `max_level` SHALL be refused with an error naming both values. The viewer takes no flag; `test-render` and `generate` do.

Verify: A, C
Status: active
Source: ADR 0062 point 3, SPIKE-12 Q1 option (b), LOD-003, LOD-004, LOD-007, LOD-008, LOD-010

#### Scenario: Levels below the floor are never selected
- **WHEN** the ten standard views and a camera at 100,000 km are selected with floor 3
- **THEN** no node, D, R, prefetch or cover tile has a level below 3, the 100,000 km view draws at most 384 nodes, orbit-20000km draws 181 level-3 nodes (R is the same 181 tiles, with no ancestor), and the other nine views draw the tiles of floor 0 (11,661 nodes) with every node above the floor equal to the floor-0 node

#### Scenario: Floor leaves do not morph
- **WHEN** the orbit view is planned with floor 3
- **THEN** every floor leaf has morph factor 0 and the interval `[1e30, 1e30]`, the plan builder accepts it, and the same interval on a leaf above the floor is still refused

#### Scenario: Small finest level needs no flag
- **WHEN** a scene with `max_level` 2 is planned with the default floor
- **THEN** the effective floor is 2 (and 0 and 1 for a `max_level` of 0 and 1), the plan draws the tiles of the floor-0 plan of that scene (its level-2 leaves without a morph), and nothing is refused

#### Scenario: Explicit floor above the finest level
- **WHEN** `--floor-level 5 --finest-level 4` is given
- **THEN** it is refused before any work with an error naming 5 and 4

#### Scenario: Floor zero reproduces the earlier plans
- **WHEN** the ten standard views and the three view variants of the plan snapshot are planned with floor 0
- **THEN** the node counts and plan hashes equal those recorded before the change (`terrain_plans_floor0.txt`)

#### Scenario: The orbit view with the floor
- **WHEN** the orbit-20000km views (level, relief and normals) are drawn from floor-3 tiles on the software adapter
- **THEN** the picture covers the whole planet without a hole or a crack, floor leaves show no morph, and the goldens that change by this declaration (tile ids, levels, geometry) are listed in the change and go to the owner's `/bless` (ADR 0062 consequences)

