## ADDED Requirements

### Requirement: REND-039 A landed chunk's change fades in the draw
A tile with chunk detail SHALL carry the chunk's change per vertex beside its heights, and the vertex shader SHALL place each vertex at `h_q - (1 - w) x change` along the base normal, `w` the tile's `chunk_weight` of the node record (an f32, CON-11). With `w = 1` or without a change plane the arithmetic SHALL be the old one bit for bit, so a world without chunks draws as it did, and at `w = 0` the drawn height SHALL be the base's. The CPU mirror and the reference renderer SHALL form the same f32 operations.

Verify: A, C
Status: planned
Source: M8 exit criterion 3, ADR 0077 point 7, STRM-039 (the weight), GEN-025, REND-031

#### Scenario: The fade in the draw
- **WHEN** a tile with a change plane is drawn at `w` 0, 0.5 and 1 on the software adapter, and a tile without one is drawn
- **THEN** the drawn heights are the base's, the base plus half the change, and the final heights; the tile without a plane is bit-identical to the old draw; and the step per frame of a fade over `chunk_fade_ms` at 60 frames per second is at most the weight step times the largest change, which the test prints
