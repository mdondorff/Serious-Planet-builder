## ADDED Requirements

### Requirement: STRM-027 Authored stage keys
The height key of a tile of an authored stage SHALL chain the stage below (ADR 0066 point 4): its source SHALL be the key of the same tile, at the same resolution, of the stage it reads (the base first), its generator version SHALL be the stage version, its implementation id the stage's, its output parameters SHALL include the layer name and the layer's index in the stack of ADR 0007 (not its priority), and its reads SHALL be the layer's footprint or region summary entries of the tile (WDEF-007, WDEF-008) plus the nodes the stage reads. The mesh key of the tile SHALL chain the stage's height key as it chains any height key. A change of the base SHALL re-key every tile of the stage; a change of the stage version, the layer name or its index SHALL re-key the tiles of that layer and of the layers above it; an edit of an item SHALL re-key exactly the tiles whose footprint or summary entries change. Inserting a layer into the stack SHALL bump the stage version of every layer above it. A key without a stage SHALL be byte for byte what it was.

Verify: A
Status: planned
Source: ADR 0066 point 4, ADR 0006, CON-08

#### Scenario: Chain
- **WHEN** the base seed, the stage version, the layer name or the stack index changes, and when a stamp far from the tile changes
- **THEN** the stage's tile key changes in the first four cases and not in the last, and the mesh key follows the height key

#### Scenario: Edit locality
- **WHEN** a stamp is added, moved, edited or removed
- **THEN** the keys of exactly the tiles its reach touches change, and removing it restores every key

#### Scenario: Unstaged keys
- **WHEN** the keys of the base field are computed with and without the stage code
- **THEN** they are identical and the pinned known answers are unchanged
