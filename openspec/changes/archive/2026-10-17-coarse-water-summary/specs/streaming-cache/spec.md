## ADDED Requirements

### Requirement: STRM-025 The water summary in the store
The coarse level of W (GEN-024) of a bake SHALL be stored beside its blocks as one record of a kind of its own (`Kind::GlobalWaterSummary` = 5), filed under the root tile of face 0. The payload SHALL be `codec version (u8 = 1) | res (u32 LE) | B (u32 LE) | raw length (u32 LE, 5 x 6 (res / d)^2) | LZ4 block` of the flag bytes of all cells followed by their band elevations as a byte-shuffled little-endian `f32` plane; decoding SHALL give back every flag and every elevation bit for bit and SHALL refuse a short payload, another version, a resolution above the tiled limit, a block size that makes no summary or a header that implies more than `6 * 1024^2` cells (all before allocating), a wrong raw length, a cut or damaged block and a flag bit the version does not know. The content key SHALL be the ADR 0018 encoding with kind 5, the implementation id `cpu-water-summary-v1+cpu-ocean-connectivity-v1` (the summary is a function of the ocean bits, so renaming the channel's implementation renames the summary), `GLOBAL_VERSION`, the root tile of face 0, the bake's stage key as its source, `B` under tag 1, `res` under tag 4, the ocean area threshold under tag 7 and an empty read set; another bake, grid, `B`, seed, body or threshold SHALL have another key, the sea level and liquid of the definition SHALL NOT enter it, and the key SHALL have a pinned known answer. The liveness rule is that of the sample blocks (STRM-013). `build_global` SHALL write the summary after the ocean channel, from the stored sample blocks and ocean blocks in batches bounded as the ocean passes, when the store lacks it under its current key, and SHALL recompute nothing when it has it: a store with the blocks and the ocean channel and no summary gains only the summary (the migration of a store built before this requirement, idempotent), and another threshold adds its own without baking again. `load_water_summary` SHALL name the record, its key and the command (`generate --global-dir DIR`) when the summary is missing, damaged or of another shape, and the viewer SHALL refuse a store that has the ocean channel and no summary with that message; there is no fallback to the point query. `verify_global` SHALL decode summary records first, treat a record under another key as stale, and compare a record under the world's key cell by cell with the summary recomputed from the stored blocks, naming the first differing cell. `generate --global-dir` SHALL print whether the summary was written, its bytes and the seconds it took. No `GLOBAL_VERSION` or `MESH_VERSION` change: sample blocks, ocean blocks and tiles keep their keys.

Class (ADR 0035): the storage of GEN-024's **field**; bound: exact, every bit.

Verify: A
Status: active
Source: ADR 0057, ADR 0041 point 2, ADR 0018, ADR 0019, STRM-012, STRM-013, STRM-023, CON-05, CON-14

#### Scenario: Codec
- **WHEN** a summary is encoded and decoded, its header is read, and hostile payloads (short, another version, hostile resolution or block size, wrong raw length, cut, an unknown flag bit) are decoded
- **THEN** every flag and elevation comes back bit for bit (infinities included), the header holds the documented words, and each hostile payload is refused by name without allocating

#### Scenario: Keys
- **WHEN** a summary key is computed for another grid, `B`, seed, threshold, and for a definition with another sea level
- **THEN** it differs for each of the first four, equals the key of the same bake under the other sea level, the record is live by the rule of the sample blocks, a block size that is not the canonical layout makes no record, and the known answer is pinned

#### Scenario: Build, reuse and threads
- **WHEN** a bake is built on one and on three threads, built again, and built with another threshold
- **THEN** the two summaries are equal to each other and to the summary of the in-memory bake, the second build writes no block, ocean block or summary, and the other threshold adds its own summary without baking

#### Scenario: Migration
- **WHEN** a store holding every record but the summary is built over
- **THEN** no block is baked or analysed again, the summary is written and equals the one a fresh build writes, and a further build writes nothing

#### Scenario: Verify
- **WHEN** a store with one cell flipped, a damaged summary record and another threshold's summary is verified
- **THEN** the first is a mismatch naming the cell, the second is unreadable, the third is stale, and a clean store passes

#### Scenario: The viewer refuses a store without it
- **WHEN** the terrain of a store that has the blocks and the ocean channel but no summary is opened
- **THEN** it is refused naming the summary record and `generate --global-dir`
