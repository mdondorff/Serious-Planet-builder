## ADDED Requirements

### Requirement: STRM-027 Authored stage keys
The height key of a tile of an authored stage SHALL chain the stage below (ADR 0066 point 4; the chained height key is the stage key of that point, and `stage_key` of STRM-011 is not used because a stamped tile is a `Kind::Height` record that the mesh key and the tile store chain on): its source SHALL be the key of the same tile, at the same resolution, of the stage it reads (the base first, preferred over a field-wide source key), its generator version SHALL be the stage version, its implementation id the stage's, its output parameters SHALL include the layer name (tag 8) and the layer's index in the stack of ADR 0007 (tag 9, not its priority), and its reads SHALL be the layer's footprint or region summary entries of the tile (WDEF-007, WDEF-008) plus the nodes the stage reads. The mesh key of the tile SHALL chain the stage's height key as it chains any height key. A change of the base SHALL re-key every tile of the stage; a change of the stage version, the layer name or its index SHALL re-key the tiles of that layer and of the layers above it; an edit of an item SHALL re-key the tiles whose footprint or summary entries change (at least those its reach touches, none beyond the margin of the query). Inserting a layer into the stack SHALL bump the stage version of every layer above it. Every wrapper of a height field SHALL forward the chain (`upstream`, `stage_params`) with its other keying methods. A key without a stage SHALL be byte for byte what it was.

Verify: A
Status: active
Source: ADR 0066 point 4, ADR 0006, CON-08

#### Scenario: Chain
- **WHEN** the base seed, the stage version, the layer name or the stack index changes, and when a stamp far from the tile changes
- **THEN** the stage's tile key changes in the first four cases and not in the last, and the mesh key follows the height key

#### Scenario: Edit locality
- **WHEN** a stamp is added, moved, edited or removed
- **THEN** the keys of the tiles its footprint finds change, none beyond the query's margin does, and removing it restores every key

#### Scenario: Two layers
- **WHEN** a base change and then an edit in the lower layer are made under a two-layer stack
- **THEN** the first re-keys the tiles of both layers and the second re-keys the upper layer's tiles of that place and no other

#### Scenario: Wrappers
- **WHEN** the source files of the code base are scanned for wrappers that forward `source_key`
- **THEN** each also forwards `upstream` and `stage_params`

#### Scenario: Unstaged keys
- **WHEN** the keys of a base field are computed
- **THEN** they equal the pinned known answers of the key tests, which are unchanged
