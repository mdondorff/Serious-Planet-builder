## ADDED Requirements
### Requirement: STRM-076 The layer's fill depth is stored beside the other channels
The pit depth of GEN-120 SHALL be stored in the layer's stores as two record kinds, versioned and keyed like the channels of STRM-072:
- kind 16 (`LayerFaceFill`), one record per face of pass 1's store: `codec (u8, value 1) | cells (u32 LE) | key (16 bytes) |` a brotli stream of the face's own `cells x cells` codes (the rows' wrapping differences, as kind 14);
- kind 17 (`LayerFill`), one record per tile filed like kind 13: the same layout over the core's `tile x tile` codes;
- the key of both is the channels' key (STRM-072) with the step of the code (`FILL_STEPS_PER_M`) among the scales it holds, so a change of the code is another layer and never a mix; `CHANNELS_VERSION` 2;
- a record found under another key SHALL be refused naming both keys; `run_pass1` writes a face's fill record and `run_pass2` a tile's, a store that holds a tile and its channels but no fill record is completed by computing the tile again for it, what is stored is not written twice, and a pass 2 that is given the face fill reads pass 1's depth (GEN-120);
- the runtime layer (`StoreLayer`) adds the tile's depth to its change when it builds the tile, so the pyramid (GEN-102) holds the sum; the bake's manifest range (STRM-074) is the range of that sum; a tile with no fill record is built as it is stored;
- the pass-1 fill records are part of pass 1's store and are dropped with it: pass 2 has read them.

Verify: A
Status: planned
Source: Owner 2026-11-05 (SPIKE-102 D1); STRM-065, STRM-072, GEN-120

#### Scenario: The depth comes back
- **WHEN** pass 1 and a pass 2 over every tile of three faces of a synthetic planet with a basin on every face are stored, on 1 and 8 threads, read back, and run again
- **THEN** each stored face and tile depth equals the computed codes, the packs are the same bytes on both thread counts, a tile that holds a basin has codes above 0, a tile the store lacks reads as absent, a depth asked for under another layer's key is refused naming both keys, and the second run computes nothing

#### Scenario: Keys
- **WHEN** the keys of the fill records are compared with the deposited depth's and the area's
- **THEN** they differ, and the step of the code is 1 per metre
