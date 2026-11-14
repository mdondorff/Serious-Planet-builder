## ADDED Requirements

### Requirement: STRM-065 The erosion layer is a separate store beside the bake
The erosion layer SHALL be stored as its own store beside the global bake (`global16384`), which it SHALL NOT rewrite. It SHALL hold these records:
- each tile's core change against the base field (GEN-101);
- each face's pass-1 change (GEN-100), kept until the layer is complete and then dropped;
- the drainage area and the depth channels (STRM-072).
The store SHALL be keyed by the bake's stage key, the versions of the class-weights and relief channels and the kernels' parameters (ADR 0006). Records are written per tile and per face, so that a bake resumes from its last complete checkpoint. A reader SHALL refuse a layer whose key does not match the bake it opens, naming both keys. The face change SHALL be record kind 12 (`LayerFace`), one record per cube face filed under the face's level-0 tile: `codec (u8, value 1) | cells (u32 LE) | halo (u32 LE) | key (16 bytes) |` the face's f32 plane of `cells + 2 halo` cells a side (the halo of GEN-100; row `j` outer, then `i`, little-endian). The key carries the bake's stage key as its source, the bake's resolution (tag 4), the cells (tag 1) and, under tag 15, the layer version, the version of the pass-1 kernels (`BAKE_KERNEL_VERSION`), whether the carve factor is applied, the halo, the radius, the sea level, the seed, every parameter of the carve and `inputs_id`, an identity of the base field's and the carve factor's inputs that the caller supplies (the class source's and class list's versions in PR 4; the relief channels' hash joins it when the runtime reads them, PR 6); a face stored under another key SHALL be an error naming that key and the expected one, never a recompute or a mix. Liveness (STRM-013): a record of kind 12 is judged like the bake's blocks (generator version, hash version and node set id equal the world's). A tile's change SHALL be record kind 13 (`LayerTile`), one record per tile filed under the tile's quadtree id at the level whose `2^level` tiles a face edge are the tile grid (a grid whose tiles a face edge are not a power of two is refused): `codec (u8, value 2) | tile (u32 LE, core cells a side) | land cells (u64) | thermal passes (u32) | thermal cells still moving at the pass cap (u64) | eroded, deposited, exported (f64 each, cubic metres, totals over the tile's window) | clamped cells (u64: core cells beyond the range) | key (16 bytes) |` a brotli stream (quality 5, window 22 bits; owner 2026-11-03 after SPIKE-97) of the core's change against the base field as `i16` counts of 0.25 m (clamped to the range; row `j` outer, then `i`; the change contains the pass-1 change), each row stored as its first count and the wrapping differences along the row (about 0.92 bytes a land cell, 20 GB for the planet at 0.25 m, lossless against the quantised change). A decoder SHALL refuse a codec value it does not know and a stream that does not hold exactly `tile x tile x 2` bytes, reserving no more than that. The read side SHALL keep a bounded cache of decoded tiles, so that the render tiles of one tile's square decode it once (SPIKE-97: a decode per render tile would cost 3.1 times terrain's share). The key carries the bake's stage key as its source, the bake's resolution (the grid-resolution tag), the cells a face edge (the resolution tag) and, under tag 16, the layer version, the kernel version, the tile grid (cells, tile, halo, radius), every parameter of pass 2, `inputs_id`, whether the carve factor is applied and a digest of the keys of the six pass-1 face records the tile was computed from; a tile stored under another key SHALL be an error naming that key and the expected one. Liveness (STRM-013) is judged like the bake's blocks. The pass-1 face records SHALL be dropped (their store removed) after a complete planet pass unless the run keeps them (`--keep-pass1`); a run over a region of tiles keeps them. Kind 12 is temporary working data (approved by the owner on 2026-11-03); kind 13 is the layer itself; its codec was chosen by the owner on 2026-11-03 (brotli, SPIKE-97) and the rest of its layout and key await the owner's approval of the persisted format (planner, 2026-11-03: persisted formats are the owner's call).

Verify: A
Status: active
Source: ADR 0088 points 4 and 7, M9 draft decision 7 (owner: a separate layer first), ADR 0088/0089 accepted 2026-11-01; SPIKE-61 (tile changes about 7-10 GB); record kind 12 (the face change, about 3.7 GB for six faces at full resolution) approved by the owner 2026-11-03 as temporary working data, dropped after the final layer with `--keep-pass1` to keep it (PR 5)

#### Scenario: Resume and refusal
- **WHEN** a layer bake is killed after some tiles and resumed, and its layer is later opened against a bake with another key
- **THEN** the resumed layer equals an uninterrupted one byte for byte, and the second open is refused with both keys named

### Requirement: STRM-066 The layer has a min/max pyramid
The layer SHALL hold, with each tile, the min and max of each block of 32 x 32 cells of the tile's core, of each coarser level from 2 x 2 blocks below, and of the whole tile. It SHALL be stored with its tile or built when the tile is decoded (PR 6 builds it at decode, in f32, exactly over the dequantised core), and read through a `planet-generators` trait that `planet-cache` implements (CON-02). A stored pyramid holds `u16` over `[-H, H]` with `H` the body field's `max_abs_height_m`, the min rounded down and the max up.

Verify: A
Status: active
Source: ADR 0088 point 8, ADR 0088/0089 accepted 2026-11-01; SPIKE-83 (32 x 32 blocks; about 90 MB for the planet, built in seconds); built at decode accepted by the planner 2026-11-04 (kind 13's format stays as it is)

#### Scenario: Exact, rounded outward and nested
- **WHEN** a tile's pyramid is built at decode, or stored and read back, for tiles of 96, 100 and 40 cells
- **THEN** each node's range equals (built) or holds with an outward error under two steps (stored) the range of its cells, and each coarser node holds its children's range

### Requirement: STRM-067 The class weights are stored with the bake
The class-weights lattice (GEN-093, its geometry and its `u8` weights) SHALL be stored with the global bake as one channel with its class-list version, because class paint makes it more than a function of the seed. A store whose class-list version differs from the generator's SHALL be refused, naming both versions. A store without the channel SHALL open as before the channel existed. The channel is record kind 11 (`GlobalClassWeights`), one record per cube face filed under the face's level-0 tile; its key carries the bake's stage key as its source, the bake's resolution under tag 4, the cells per face edge under tag 1 and the number of classes under tag 14 (not the class-list version, so that a store of another version is found and refused). A record's payload is `codec (u8, value 1) | class-list version (u32 LE) | class-source version (u32 LE) | cells (u32 LE) | the lattice's content hash (16 bytes) |` the face's weights (`u8`, row `j` outer, then `i`, 8 bytes a point). A store with records of the kind but of another class-list version, an incomplete face set or weights that do not match the hash is an error naming the record. The reader returns the stored class-source version and the caller compares it with the generator's (the key does not carry it); the class weights SHALL be loaded before the relief channels, because a changed class-list version is refused for the weights but reads as absent for the channels. The layout is in the m9-bake design.

Verify: A
Status: active
Source: ADR 0089 decisions 1 and 8, ADR 0080 point 3 (its "not stored" ends with ADR 0089's acceptance), ADR 0088/0089 accepted 2026-11-01

#### Scenario: Old and new stores
- **WHEN** `global16384` (without the channel) and a new store with the channel are opened
- **THEN** the old store opens unchanged, the new one reads the weights byte for byte as written, and a store of another class-list version is refused

### Requirement: STRM-068 The relief channels are stored with block ranges
The relief channels of GEN-099 (`k` and `m`, f32 on the coupled lattice, min and max per 32 x 32 block) SHALL be stored beside the global bake, versioned, as record kind 10 (`GlobalRelief`), one record per cube face filed under the face's level-0 tile: `codec (u8, value 1) | cells (u32 LE) |` the face's `k` plane then `m` plane (f32 little-endian, row `j` outer, then `i`). The block ranges are not stored: the reader recomputes them from the planes. The key carries the bake's stage key as its source, the bake's resolution (tag 4), the cells (tag 1) and, under tag 13, the channels' version, the class-list and class-source versions, the hash of the class lattice the channels were computed from, the step limit `max_step`, the ring radius and the taper: a changed lattice, version or parameter reads as absent, never as wrong. A store without the kind opens and reads as absent (the same as before the channel existed). Liveness (STRM-013): records of kinds 10 and 11 are judged like the bake's blocks (generator version, hash version and node set id equal the world's); a stale record is dropped by compaction. Together with the class-weights channel (STRM-067) they SHALL stay under 1 GB for a planet of Earth's size; the limit is a start value from the m9-bake design.

Verify: A
Status: active
Source: ADR 0088 point 1, ADR 0088/0089 accepted 2026-11-01; SPIKE-66, m9-bake design

#### Scenario: Block ranges and size
- **WHEN** the channels of a synthetic bake are stored and read back, and their size for a planet is computed from the lattice sizes
- **THEN** every value lies in its block's stored range, and the planet size is under 1 GB

### Requirement: STRM-072 The layer's channels are stored beside kinds 12 and 13
The drainage area (GEN-110) and the deposited depth (GEN-111) SHALL be stored in the layer's store as record kinds 14 and 15, versioned and keyed, and read through a small API:
- kind 14 (`LayerArea`), one record per cube face filed under the face's root tile: `codec (u8, value 1) | cells (u32 LE, pass 1's cells a face edge) | key (16 bytes) |` a brotli stream (quality 5, window 22 bits) of the face's own cells as area codes (row `j` outer, then `i`), each row as its first byte and the wrapping differences along the row;
- kind 15 (`LayerDepth`), one record per tile filed like kind 13: `codec (u8, value 1) | tile (u32 LE) | key (16 bytes) |` a brotli stream of the deposited plane of the core (codes of GEN-111, a square-root code `round(12 sqrt(m))`, same row coding; the scale and the factor 12 are in the key, `CHANNELS_VERSION` 4); the depth removed is derived by the reader as `max(0, deposited - change)` from the tile's kind 13;
- the keys carry, under tag 17, the channels' version, the scales of the codes and the key parameters of the layer they were made beside (the pass-1 layer's for kind 14, the tile layer's for kind 15), so a channel of another layer is an error naming both keys, never a recompute or a mix;
- a decoder refuses an unknown codec and a stream that does not decode to exactly the expected bytes, and reserves no more than that;
- the pass-1 store is temporary (STRM-065): the areas are copied beside the tiles before it is dropped, and a run over a layer stored before the channels existed computes the missing channels and writes what is stored no second time;
- liveness (STRM-013) is judged like the bake's blocks; a store without the kinds opens and reads as absent;
- the read API (`LayerChannels`) SHALL return, at a direction, the drainage area as log2 of cells (bilinear in codes over the four pass-1 cells around the point) and the deposited depth in metres (bilinear in metres between the cell centres around the point, read across tile borders; `removed_depth_m(deposited, change)` gives the depth removed; the area is not blended across a cube edge, where a catchment is cut at each face's halo), with bounded caches of the decoded faces and tiles; where the store holds no record it returns none.
The size of the channels for a planet SHALL be reported with the run that makes them, not bounded (the one-plane, 1 m form was chosen by the owner on 2026-11-04 at about 0.13 to 0.17 bytes a land cell for the depth, about 4 GB for the planet beside kind 13's 20 GB, 24 GB together; the rest of the persisted format awaits the owner's approval).

Verify: A
Status: active
Source: owner 2026-11-04 (planner inbox #58.3, M9 plan I5a(c)); STRM-065, GEN-110, GEN-111

#### Scenario: The channels come back as computed
- **WHEN** three tiles are run into stores on 1 and on 8 threads and the six faces' areas are copied from the pass-1 store
- **THEN** the pack bytes are equal on both thread counts, the stored depth of every tile equals the computed one, a tile without a record reads as none, the copy writes nothing the second time, and the stored area equals the pass-1 store's

#### Scenario: Refusal and completion
- **WHEN** a depth is read under another layer's parameters, a record under the right key has its stream cut, and a store holds a tile without its depth
- **THEN** the first is an error naming both keys, the second an error naming the tile, and a run computes the third tile again, stores its depth only (the tile record stays one), and skips it on the next run

#### Scenario: The reader
- **WHEN** the depth is read at the centre of a core cell and the area at the same direction
- **THEN** the depth equals the cell's stored depth within 1 mm and the area is between 0 and 32 in log2 cells (at least one cell)

