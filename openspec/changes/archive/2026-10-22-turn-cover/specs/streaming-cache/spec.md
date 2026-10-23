## ADDED Requirements

### Requirement: STRM-033 Cover tile states
The frame that draws the cover (`DrawKind::Cover`, STRM-016) SHALL label each cover tile by whether it is the tile the plan of the frame's camera would draw: a resident tile drawn whole at the level the plan's selection gives it is exact; only a tile that stands in for a tile of the plan that is not resident (a coarser tile, a tile drawn over some quadrants, or a tile the balancing made by splitting one of those) is a stand-in. With every tile of the residency set resident no cover tile is a stand-in, so the F3 stale-tile overlay (VIEW-014) does not show a camera that only turns as a stand-in.

Verify: A
Status: active
Source: owner (2026-10-21, P1-4), STRM-016, VIEW-014

#### Scenario: A turn
- **WHEN** the camera turns away from the view of the plan in hand over a residency set that is complete, and again over one with tiles missing
- **THEN** the first frame draws the cover with every tile exact; the second draws exact the tiles at the plan's level and stand-in exactly the tiles that stand in for a missing one
