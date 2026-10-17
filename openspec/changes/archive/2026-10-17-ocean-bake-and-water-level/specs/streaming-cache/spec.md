## ADDED Requirements

### Requirement: STRM-023 Ocean blocks in the store
The ocean-connectivity channel of a bake (GEN-022) SHALL be stored beside its sample blocks as blocks of a record kind of their own (`Kind::GlobalOcean` = 4): block `b` of a grid of `res = B << L` cells per face edge is the level-`L` tile `b`, holding its `(B + 1)^2` bits, row-major (`k` fastest), the first vertex in the lowest bit of the first byte. The payload SHALL be `codec version (u8 = 1) | B (u32 LE, a power of two up to 1024) | raw length (u32 LE, ceil((B + 1)^2 / 8)) | LZ4 block`; decoding SHALL give back every bit exactly and SHALL refuse a short payload, another version, a `B` outside the limit, a wrong raw length, a damaged block and a bit set beyond the last vertex, without allocating more than `B` allows, and the encoder SHALL refuse what the decoder would. The content key SHALL be the ADR 0018 encoding with kind 4, the implementation id `cpu-ocean-connectivity-v1`, `GLOBAL_VERSION`, the block's tile, the bake's stage key as its source, `B` under tag 1, `res` under tag 4, the area threshold (the bits of the `f64` fraction of the body's surface, WDEF-006) under tag 7 and an empty read set; two blocks, another `B`, `res`, seed, body or threshold SHALL have different keys, the sea nodes of the definition SHALL reach the key only through the threshold and SHALL NOT change the stage key or a sample block's key (the sea is not an input of the bake), and the key SHALL have a pinned known answer. The node set id and the liveness rule of an ocean block are those of the sample blocks (STRM-012, STRM-013). `build_global` SHALL analyse every block it bakes on the worker thread that baked it while the block is in memory, decode and analyse (in batches bounded as the bake's) the blocks it did not bake, merge the reports, and write the ocean blocks the store lacks from a second pass over the stored blocks; a build that finds every ocean block under its current key SHALL recompute nothing, and another threshold SHALL add its channel without baking again. A `StoreOceanSource` SHALL read the stored channel as an ocean lattice (it finds a vertex's block through the canonical layout, keeps a bounded number of decoded blocks and names a missing or damaged block and its key). `verify_global` SHALL decode ocean blocks first, treat a record under another key as stale (explained when its versions or node set id differ), and compare a record under the world's key bit for bit with the channel recomputed from the stored sample blocks, naming the tile, pack, offset and first differing vertex. `generate --global-dir` SHALL print the channel's blocks written and already stored, its sea components and oceans, the threshold, the time beyond the bake, the analysis thread time inside it and the merger's bytes. Stored bakes and meshes of `GLOBAL_VERSION` 3 are unreachable under the new keys (no `MESH_VERSION` change).

Class (ADR 0035): the storage of GEN-022's **field**; bound: exact, every bit. Ocean blocks of an older threshold stay live (liveness judges versions and node set only) until compaction drops them with their bake.

Verify: A
Status: active
Source: ADR 0041 point 2, ADR 0018, ADR 0019, ADR 0026, ADR 0027, STRM-012, STRM-013, CON-05, CON-14

#### Scenario: Codec
- **WHEN** ocean blocks of 1 to 32 cells are encoded and decoded, a one-cell block is written out by hand, and hostile payloads (short, another version, hostile cells, wrong raw length, cut, a spare bit set) are decoded
- **THEN** every bit comes back, the bytes equal the documented layout, and each hostile payload is refused by name without allocating

#### Scenario: Keys
- **WHEN** an ocean key is computed for another tile, `B`, `res`, seed, body or threshold, and for a definition that sets the sea
- **THEN** it differs for each, equals the key of the same threshold set in code, the sample block keys and the stage key do not move with the sea, and the known answer is pinned

#### Scenario: Build, reuse and read
- **WHEN** a bake is built twice, then read back through the store source, and built again with another threshold
- **THEN** the second build writes nothing and recomputes no table, the stored channel equals the in-memory computation and the serial reference at every vertex, the third adds the new channel without baking, and one thread or three give the same payloads

#### Scenario: Verify
- **WHEN** a store with a flipped vertex, a damaged ocean block and a block of another threshold is verified
- **THEN** the first is a mismatch naming its vertex, the second is unreadable, the third is stale, and a missing sample block is named by the table computation
