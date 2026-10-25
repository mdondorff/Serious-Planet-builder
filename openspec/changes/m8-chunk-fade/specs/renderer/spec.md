## ADDED Requirements

### Requirement: REND-039 A landed chunk's change fades in the draw
A tile with chunk detail SHALL carry the chunk's change per vertex beside its heights, and the vertex shader SHALL place each vertex, and the parent-grid target it morphs to, at `h_q - (1 - w) x change` along the base normal, `1 - w` the fade in the node record (the fourth lane of grid phase 6, an f32, CON-11; 0 at `w` 1, so the record does not grow and a settled node's record is what it was), the plane read from the frame's tile-reference buffer where the node's reference points (the renderer writes the plane there only for a node whose change is still fading in, at most `FADE_PLANES` (512) a frame, the rest drawing their final height and counted in `TerrainStats::fade_overflow`; one storage binding fewer than a second pool would need, so the vertex stage still reads four). With `w = 1` or without a change plane the arithmetic SHALL be the old one bit for bit, so a world without chunks draws as it did, and at `w = 0` the drawn height SHALL be the base's. The normals stay those of the final surface (design decision 4). The CPU mirror (`CompactTile::shade_vertex_faded`) and the reference renderer SHALL form the same f32 operations.

Verify: A, C
Status: active
Source: M8 exit criterion 3, ADR 0077 point 7, STRM-039 (the weight), GEN-025, REND-031

#### Scenario: The fade in the draw
- **WHEN** a tile with a change plane is drawn at `w` 0, 0.5 and 1 on the software adapter, and a tile without one is drawn
- **THEN** the drawn heights are the base's, the base plus half the change, and the final heights (the compact renderer's image equals the reference renderer's of the blended field, no pixel differing); the tile without a plane is bit-identical to the old draw, and so is a tile with one at weight 1; the mirror's vertices are the final minus fade times change; and the step per frame of a fade over `chunk_fade_ms` at 60 frames per second is at most the weight step times the largest change (0.2 m for the largest entry step measured)
