## ADDED Requirements

### Requirement: STRM-047 The change and its weight reach the draw
The compact tile builder SHALL store the chunk change of a tile at the tile's own level beside its heights (`HeightField::level_change`, forwarded by every wrapper that forwards `height_at_level`), and the plan's node (`TerrainDraw::chunk_weight`) SHALL carry the tile's fade weight, the minimum of the weights of the chunks the tile reads, so two tiles on either side of a chunk border rise together; the node record holds it as the fade `1 - w` (REND-039).

Verify: A
Status: active
Source: STRM-039 (the weight), STRM-038 (the chunks a tile reads), REND-039

#### Scenario: Change and weight
- **WHEN** a tile at the pyramid top over a ready chunk and a tile near its border with a chunk that landed later are built and planned
- **THEN** the stored change equals the field's height at the tile's level minus the base's, the node weights follow the fade of each chunk, and the border tile takes the smaller weight of the two chunks it reads
