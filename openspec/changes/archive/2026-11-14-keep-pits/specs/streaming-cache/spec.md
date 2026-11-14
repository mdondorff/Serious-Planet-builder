## ADDED Requirements
### Requirement: STRM-076 The layer's fill depth is stored beside the other channels
The pit depth of GEN-120 SHALL be stored in the layer's tile store as record kind 17 (`LayerFill`), versioned and keyed like the channels of STRM-072:
- one record per tile filed like kind 13: `codec (u8, value 2) | tile (u32 LE) | key (16 bytes) |` a brotli stream of the core's 16-bit codes (GEN-120; 0.25 m steps, owner 2026-11-05, a key parameter; the rows' wrapping differences, little-endian), written by the post-pass (STRM-077);
- the key is the channels' key (STRM-072) with the step of the code (`FILL_STEPS_PER_M`) among the scales it holds, so a change of the code is another layer and never a mix; `CHANNELS_VERSION` 3; codec 1 records (8-bit codes, keep-pits #1192) are another key and are never read; kind 16 (a face's pass-1 depth) is not used;
- a record found under another key SHALL be refused naming both keys;
- the runtime layer (`StoreLayer`) adds the tile's depth to its change when it builds the tile, so the pyramid (GEN-102) holds the sum; the bake's manifest range (STRM-074) is the range of that sum; a tile with no fill record is built as it is stored.

Verify: A
Status: active
Source: Owner 2026-11-05 (SPIKE-102 D1); planner inbox #67; STRM-065, STRM-072, GEN-120

#### Scenario: Keys
- **WHEN** the keys of the fill and the scratch heights are compared with the deposited depth's
- **THEN** they differ, and the step of the code is 4 a metre
