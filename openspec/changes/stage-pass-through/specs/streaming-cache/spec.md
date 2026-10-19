## ADDED Requirements

### Requirement: STRM-029 A tile a stage passes through keys as its source
By exception to STRM-027, a tile a stage passes through (GEN-027) SHALL have the keys of the stage below it, in both key domains (the 128-bit persistent height and mesh keys and the 64-bit height and mesh keys of the plan, the batch keyer included), whatever the number of stages and whatever wraps them; a tile the stage acts on SHALL be keyed as STRM-027 says. So the first stamp of a world, in any layer, re-keys only the tiles its footprint touches, the tiles out of its reach keep the keys they had without the stage, and the pre-filled tiles of the plain world serve them.

Verify: A
Status: active
Source: ADR 0068, ADR 0066 point 4

#### Scenario: Both domains
- **WHEN** a stamp stage with an item near one tile is keyed at a far tile and a coarse far tile, alone, under a second stage, and wrapped in the water bound, and at the tile under the stamp
- **THEN** the far tiles' height, mesh and 64-bit keys and the batch keyer's equal the plain field's in every form, and the tile under the stamp differs in every form

#### Scenario: First stamp of a world
- **WHEN** the first stamp is put into the definition of a world through `set_definition`, and then a stamp in a second layer
- **THEN** a tile out of reach keeps the key it had in the plain world, and the tile under the stamp is re-keyed
