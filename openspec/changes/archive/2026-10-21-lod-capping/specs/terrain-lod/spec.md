## MODIFIED Requirements

### Requirement: LOD-008 Height-aware selection
Node selection SHALL measure distances to per-node bounding volumes that include the node's height bounds instead of the reference sphere. The morph interval of a node SHALL end at the split distance of its parent and start no earlier than its own split distance plus twice the largest bounding radius among its same-level neighbours, equalised over the leaves of a level (shared border vertices morph identically), and vertex distances SHALL be measured to the vertices' real positions, so that in every scripted view, on the reference and the steep stress field (TEST-015), the camera stays above the drawn surface by at least the near plane, no drawn surface comes nearer than the near plane, and the leaf under the camera meets `tau_px` (or is at the maximum level). A tile is morphable when its morph interval has room (`morph_room`: finite, and starting within 95 % of its end; the one predicate of `LodParams::validate`, the selection and the plan's backstop; a bound that is not a number leaves none). A tile above the render floor that is not morphable SHALL NOT be a leaf: the selection SHALL keep from splitting every tile whose children are not all morphable, and the tiles whose parent is such a tile stay under it, so the plan degrades locally and no interval is ever narrowed (ADR 0074).
The ceiling of the leaves SHALL be `min over the unmorphable tiles u (topmost, found on the position-only selection without the frustum, so that R and the plan of any view agree) of (level(parent(u)) + d)`, with `d` the number of tile edges between the level-`level(parent(u))` tiles (across cube faces; for a coarser tile the least over its descendants, for a finer one that of its ancestor). It is 1-Lipschitz across edges: a selection clamped to it stays balanced (2:1), and the result is the finest balanced partition not finer than the uncapped selection with every leaf morphable, whatever the order the tiles are met in.
A leaf that the ceiling kept coarse is a capped node (`SelectedNode::capped`, `TerrainDraw::capped`, a runtime flag not in the plan's text). A plan with no unmorphable tile is the plan of the selection without the ceiling. The plan SHALL still be rejected with the tile, the interval and the parameters to change where `LodParams::validate` finds parameters with no room at any slope and, as a backstop after the selection, where a leaf above the floor has no room.

Verify: A
Status: active
Source: ADR 0014, ADR 0016, TEST-015, CON-13

#### Scenario: Steep ground at 1 m
- **WHEN** the steep ground-1m view is planned
- **THEN** the drawn surface under the camera lies below it by at least the near plane, the nearest drawn surface is no nearer than the near plane, and the leaf there projects its sample spacing to at most `tau_px` at the camera's height above the terrain (or is at the maximum level)

#### Scenario: Too steep for the parameters
- **WHEN** a plan is requested over terrain whose height extent at some tile size exceeds what the split distance leaves room for (the steep stress field at ground level, a spike of 220 m over 100 m)
- **THEN** the plan is made, the tiles over that ground are capped, every leaf above the floor has a morph interval with room (none narrowed), the leaves are balanced (2:1), and the same request over the reference field has no capped tile

#### Scenario: The ceiling
- **WHEN** random sets of unmorphable tiles on a sphere of tiles are turned into a ceiling and the finest tiles are clamped to it, in any order of the set
- **THEN** the leaves tile the sphere with none under another, every leaf is at or under its ceiling, the unmorphable tiles and what is under them are not leaves, the selection is balanced, splitting any coarsened leaf puts a child over its ceiling or unbalances it, and the order of the set changes nothing

#### Scenario: Bounds that are not a number
- **WHEN** the height bounds of a field are not numbers near a point and a plan is made there
- **THEN** no leaf is kept with a morph interval without room, and the tiles whose neighbours' bounds are not numbers are capped
