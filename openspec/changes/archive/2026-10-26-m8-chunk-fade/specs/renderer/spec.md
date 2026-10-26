## ADDED Requirements

### Requirement: REND-039 A landed chunk's change fades in the draw
A tile with chunk detail SHALL carry the chunk's change per vertex beside its heights, and the vertex shader SHALL place each vertex, and the parent-grid target it morphs to, at `h_q - (1 - w) x change` along the base normal. The fade `1 - w` SHALL be the fourth lane of grid phase 6 of the node record (an f32, CON-11), the plane read from the frame's tile-reference buffer. With `w = 1` or without a plane the arithmetic SHALL be the old one bit for bit, and at `w = 0` the drawn height SHALL be the base's. The normals stay the final surface's. The CPU mirror (`CompactTile::shade_vertex_faded`) SHALL form the same f32 operations.

Verify: A, C
Status: active
Source: M8 exit criterion 3, ADR 0077 point 7, STRM-039 (the weight), GEN-025, REND-031

#### Scenario: The fade in the draw
- **WHEN** a tile with a change plane is drawn at `w` 0, 0.5 and 1 on the software adapter, and a tile without one is drawn
- **THEN** the drawn heights are the base's, the base plus half the change, and the final heights (the compact renderer's image equals the reference renderer's image of a product mesh of the blended field, which uses its own arithmetic: no pixel differs by more than the test's tolerance of 0.4 %, 0 measured); the tile without a plane is bit-identical to the old draw, and so is a tile with one at weight 1; the mirror's vertices are the final minus fade times change; and the step per frame of a fade over `chunk_fade_ms` at 60 frames per second is at most the weight step times the largest change (0.19 m for the 256 bake's largest entry step of 17.55 m, 0.65 m for the shared store's 58.3 m, which is 0.20 px on screen where it enters: `design.md` 6a; a measurement of the scene, not an assertion of the test)

### Requirement: REND-042 Fade planes
The renderer SHALL write a plane into the frame's tile-reference buffer only for a node whose change is still fading in, at most `FADE_PLANES` (512) a frame, the rest drawing their final height (`TerrainStats::fade_overflow`). A plane that does not hold one value per texel, or holds one that is not finite, SHALL be refused naming its length or texel, and the CPU mirror SHALL form no `fade x change` at fade 0. Planes SHALL be given stickily (`FadePlaneGrants`): a tile holding one keeps it while it fades, a tile left without stays without until its fade ends, a new one is granted in tile-id order while there is room.

Verify: A, C
Status: active
Source: REND-039, M8 exit criterion 3

#### Scenario: Planes
- **WHEN** more tiles fade than there are planes, the lowest id finishes, and a plane holds NaN, inf or too few values
- **THEN** the tiles that held planes keep them and those left without stay without until their fades end, and the bad plane is refused naming its texel or its length

#### Scenario: Known limit at a chunk border
- **WHEN** a tile reads a chunk that is not ready while its neighbour reads only the ready ones
- **THEN** the tile weighs 0 while the neighbour is at its weight, so the two differ along their shared edge by up to the change times the weight difference for as long as the neighbour chunk takes; the skirt (0.08 of the tile edge, about 12 m at level 16 and 49 m at level 14) covers only the steps of the finer levels, not the 58 m of the largest valley, until `m8-chunk-fade-increment`
