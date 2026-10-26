## ADDED Requirements

### Requirement: STRM-053 A rebuilt tile carries the increment over the tile it replaces
A rebuilt tile SHALL be uploaded with a change plane that is, per texel, its own final height less the height the replaced tile is drawn at (`CompactTile::change_replacing`), so that at the weight of its chunks' clock it draws `drawn_old + w x (final' - drawn_old)`: at weight 0 what was on screen, at weight 1 its own surface, and at any weight the same as every other tile rebuilt for the same landing along their shared edges. A tile without a predecessor SHALL keep the plane over the base. This rests on an invariant: a drawn tile that reads a chunk is never stale while that chunk's clock runs (STRM-052).
Verify: A
Status: active
Source: STRM-047, STRM-052, numerics review of 2026-10-26

#### Scenario: Neighbour landing and a chunk that is not ready
- **WHEN** two tiles side by side across the border of chunks A and B are drawn with A faded in and B not ready, and B then lands and both tiles are rebuilt
- **THEN** before and after the landing the edge between them is closed to within 0.05 m at every weight, the rebuilt tiles draw what the old ones drew at weight 0 and their own surface at weight 1, and no tile moves while a stale tile that reads the chunk is still drawn

## MODIFIED Requirements

### Requirement: STRM-054 A replaced tile keeps its weight until the swap
The loop SHALL keep, for each resident tile that reads chunks, the tile as it was last uploaded, and the weight it keeps drawing at from the landing of a chunk it reads until its rebuilt tile is uploaded (the weight it had when the chunk landed).

Verify: A
Status: active
Source: STRM-053, STRM-047

### Requirement: STRM-047 The change and its weight reach the draw
The compact tile builder SHALL store the chunk change of a tile at the tile's own level beside its heights (`HeightField::level_change`, forwarded by every wrapper that forwards `height_at_level`), and the plan's node (`TerrainDraw::chunk_weight`) SHALL carry the tile's fade weight, the minimum of the weights of the chunks the tile reads that are ready (a chunk that is not ready adds nothing to its plane and has no weight), so two tiles on either side of a chunk border rise together, also next to a chunk that is still being computed; the node record holds it as the fade `1 - w` (REND-039).

Verify: A
Status: active
Source: STRM-039 (the weight), STRM-038 (the chunks a tile reads), REND-039

#### Scenario: Change and weight
- **WHEN** a tile at the pyramid top over a ready chunk and a tile near its border with a chunk that landed later are built and planned
- **THEN** the stored change equals the field's height at the tile's level minus the base's (for a tile without a predecessor), the node weights follow the fade of each chunk, and the border tile takes the smaller weight of the ready chunks it reads
