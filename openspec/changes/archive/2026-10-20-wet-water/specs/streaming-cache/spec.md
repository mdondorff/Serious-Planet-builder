## MODIFIED Requirements

### Requirement: STRM-025 The water summary in the store
The coarse level of W (GEN-024) of a bake SHALL be stored beside its blocks as one record of a kind of its own (`Kind::GlobalWaterSummary` = 5), filed under the root tile of face 0. The payload SHALL be `codec version (u8 = 2) | res (u32 LE) | B (u32 LE) | raw length (u32 LE, 7 x 6 (res / d)^2) | LZ4 block` of the flag bytes of all cells, their band elevations as a byte-shuffled little-endian `f32` plane, then their floors as a plane of low bytes and a plane of high bytes (version 1, ADR 0058, had no floor plane and is refused); decoding SHALL give back every flag, every elevation and every floor bit for bit and SHALL refuse a short payload, another version, a resolution above the tiled limit, a block size that makes no summary or a header that implies more than `6 * 1024^2` cells (all before allocating), a wrong raw length, a cut or damaged block and a flag bit the version does not know. The content key SHALL be the ADR 0018 encoding with kind 5, the implementation id `cpu-water-summary-v2+cpu-ocean-connectivity-v1+cpu-wet-floor-v1` (the summary is a function of the floor, so renaming the ocean or floor implementation renames the summary), `GLOBAL_VERSION`, the root tile of face 0, the bake's stage key as its source, `B` under tag 1, `res` under tag 4, the ocean area threshold under tag 7 and an empty read set; another bake, grid, `B`, seed, body or threshold SHALL have another key, the sea level and liquid of the definition SHALL NOT enter it, and the key SHALL have a pinned known answer. The liveness rule is that of the sample blocks (STRM-013). `build_global` SHALL write the summary after the ocean channel and the floor blocks (STRM-032), from the stored sample blocks and floor blocks in batches bounded as the ocean passes, when the store lacks it under its current key, and SHALL recompute nothing when it has it: a store with the blocks, the ocean channel and the floor blocks and no summary under the current key gains only the summary (the migration of a store built before this requirement, idempotent: a version-1 summary is under another key and stale), and another threshold adds its own without baking again. `load_water_summary` SHALL name the record, its key and the command (`generate --global-dir DIR`) when the summary is missing, damaged or of another shape, and the viewer SHALL refuse a store that has the ocean channel and no summary, or no floor blocks (STRM-032), with that message; there is no fallback to the point query. `verify_global` SHALL decode summary records first, treat a record under another key as stale, and compare a record under the world's key cell by cell with the summary recomputed from the stored sample and floor blocks, naming the first differing cell (flags, band elevation and floor). `generate --global-dir` SHALL print whether the summary was written, its bytes and the seconds it took. No `GLOBAL_VERSION` or `MESH_VERSION` change: sample blocks, ocean blocks and tiles keep their keys.

Class (ADR 0035): the storage of GEN-024's **field**; bound: exact, every bit.

Verify: A
Status: active
Source: ADR 0058, ADR 0041 point 2, ADR 0018, ADR 0019, STRM-012, STRM-013, STRM-023, CON-05, CON-14

#### Scenario: Codec
- **WHEN** a summary is encoded and decoded, its header is read, and hostile payloads (short, another version including 1, hostile resolution or block size, wrong raw length, cut, an unknown flag bit) are decoded
- **THEN** every flag, elevation and floor comes back bit for bit (infinities included), the header holds the documented words, and each hostile payload is refused by name without allocating

#### Scenario: Keys
- **WHEN** a summary key is computed for another grid, `B`, seed, threshold, and for a definition with another sea level
- **THEN** it differs for each of the first four, equals the key of the same bake under the other sea level, the record is live by the rule of the sample blocks, a block size that is not the canonical layout makes no record, and the known answer is pinned

#### Scenario: Build, reuse and threads
- **WHEN** a bake is built on one and on three threads, built again, and built with another threshold
- **THEN** the two summaries are equal to each other and to the summary of the in-memory bake, the second build writes no block, ocean block, floor block or summary, and the other threshold adds its own summary without baking

#### Scenario: Migration
- **WHEN** a store holding every record but the summary is built over
- **THEN** no block is baked or analysed again, the summary is written and equals the one a fresh build writes, and a further build writes nothing

#### Scenario: Verify
- **WHEN** a store with one cell flipped, a damaged summary record and another threshold's summary is verified
- **THEN** the first is a mismatch naming the cell, the second is unreadable, the third is stale, and a clean store passes

#### Scenario: The viewer refuses a store without it
- **WHEN** the terrain of a store that has the blocks and the ocean channel but no summary is opened
- **THEN** it is refused naming the summary record and `generate --global-dir`

### Requirement: STRM-032 The wet floor in the store
The global bake's store SHALL hold the wet floor (GEN-031) as blocks of record kind 6, one per sample block, each `(B + 1)^2` codes. The payload SHALL be `codec version (u8) | B (u32 LE) | raw length (u32 LE) | LZ4 block` of the low bytes of the codes followed by their high bytes, and the decoder SHALL refuse a version it does not read, a `B` that is not a power of two up to the block limit, a raw length other than `2 (B + 1)^2` and a damaged or short LZ4 block, without allocating more than `B` allows. The key SHALL be the ocean block's inputs of the same tile with kind 6 and the implementation id `cpu-wet-floor-v1+cpu-ocean-connectivity-v1`, so that it depends on the bake, the area threshold of the ocean channel and both rules, and not on the sea level. `build_global` SHALL write the floor blocks the store lacks after the ocean blocks and before the summary, from the stored sample and ocean blocks, with the same result for every thread count, and a second run SHALL write none. `StoreWetFloorSource` SHALL implement `FloorLattice` over the store: a block that is missing, damaged or of other cells is an error naming the block, its key and the `generate --global-dir DIR --global-res N` command that writes it, never a default, and the viewer SHALL refuse a store that has the ocean channel and no floor blocks with that message. `verify_global` SHALL decode every kind 6 record, count one under another key as stale, and compare one under the world's key with the floors recomputed from the stored blocks. A record of kind 6 SHALL be live when its versions and node set id equal the world's (STRM-013).

Verify: A
Status: active
Source: ADR 0071 point 3, STRM-025, STRM-014

#### Scenario: Payload
- **WHEN** a block with every sentinel is encoded and decoded, and payloads with a wrong version, a wrong or huge `B`, a wrong raw length and a truncated LZ4 block are decoded
- **THEN** the codes round-trip bit for bit and each bad payload is an error without a large allocation

#### Scenario: Key
- **WHEN** the floor key of a tile is compared with the ocean and sample keys of it, with that of another seed and that of another area threshold
- **THEN** all differ, and a record of an older generator version is not live

#### Scenario: Build and read
- **WHEN** a 32-cell bake is built on one and on three threads and read through `StoreWetFloorSource`, then built again, then built for another threshold
- **THEN** every vertex equals the in-memory floor, the second build writes no block of any kind, and the other threshold adds only its ocean and floor blocks

#### Scenario: Missing and verified
- **WHEN** a store is read for a threshold whose floor blocks it does not hold, and a fresh store is verified
- **THEN** the error names the block, its key and the command, and the verification finds no mismatch and some matching records
