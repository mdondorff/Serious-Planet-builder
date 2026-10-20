## ADDED Requirements

### Requirement: STRM-032 The wet floor in the store
The global bake's store SHALL hold the wet floor (GEN-031) as blocks of record kind 6, one per sample block, each `(B + 1)^2` codes. The payload SHALL be `codec version (u8) | B (u32 LE) | raw length (u32 LE) | LZ4 block` of the low bytes of the codes followed by their high bytes, and the decoder SHALL refuse a version it does not read, a `B` that is not a power of two up to the block limit, a raw length other than `2 (B + 1)^2` and a damaged or short LZ4 block, without allocating more than `B` allows. The key SHALL be the ocean block's inputs of the same tile with kind 6 and the implementation id `cpu-wet-floor-v1+cpu-ocean-connectivity-v1`, so that it depends on the bake, the area threshold of the ocean channel and both rules, and not on the sea level. `build_global` SHALL write the floor blocks the store lacks after the ocean blocks and before the summary, from the stored sample and ocean blocks, with the same result for every thread count, and a second run SHALL write none. `StoreWetFloorSource` SHALL implement `FloorLattice` over the store: a block that is missing, damaged or of other cells is an error naming the block, its key and the `generate --global-dir DIR --global-res N` command that writes it, never a default. `verify_global` SHALL decode every kind 6 record, count one under another key as stale, and compare one under the world's key with the floors recomputed from the stored blocks. A record of kind 6 SHALL be live when its versions and node set id equal the world's (STRM-013).

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
