## ADDED Requirements

### Requirement: STRM-065 The erosion layer is a separate store beside the bake
The erosion layer SHALL be stored as its own store beside the global bake (`global16384`), which it SHALL NOT rewrite. It SHALL hold two kinds of record:
- each tile's core change against the base field (GEN-101);
- each face's pass-1 change (GEN-100), kept until the layer is complete and then dropped.
The store SHALL be keyed by the bake's stage key, the versions of the class-weights and relief channels and the kernels' parameters (ADR 0006). Records are written per tile and per face, so that a bake resumes from its last complete checkpoint. A reader SHALL refuse a layer whose key does not match the bake it opens, naming both keys. The face change SHALL be record kind 12 (`LayerFace`), one record per cube face filed under the face's level-0 tile: `codec (u8, value 1) | cells (u32 LE) | key (16 bytes) |` the face's f32 plane (row `j` outer, then `i`, little-endian). The key carries the bake's stage key as its source, the bake's resolution (tag 4), the cells (tag 1) and, under tag 15, the layer version, the version of the pass-1 kernels (`BAKE_KERNEL_VERSION`), whether the carve factor is applied, the radius, the sea level, the seed, every parameter of the carve and `inputs_id`, an identity of the base field's and the carve factor's inputs that the caller supplies (the class source's and class list's versions in PR 4; the relief channels' hash joins it in PR 5); a face stored under another key SHALL be an error naming that key and the expected one, never a recompute or a mix. Liveness (STRM-013): a record of kind 12 is judged like the bake's blocks (generator version, hash version and node set id equal the world's). The tile records' kind and encoding are in the m9-bake design.

Verify: A
Status: planned
Source: ADR 0088 points 4 and 7, M9 draft decision 7 (owner: a separate layer first), ADR 0088/0089 accepted 2026-11-01; SPIKE-61 (tile changes about 7-10 GB)

#### Scenario: Resume and refusal
- **WHEN** a layer bake is killed after some tiles and resumed, and its layer is later opened against a bake with another key
- **THEN** the resumed layer equals an uninterrupted one byte for byte, and the second open is refused with both keys named

### Requirement: STRM-066 The layer stores a min/max pyramid
The layer SHALL store beside each tile's heights the min and max of:
- each 32 x 32 block of the tile's core (its 33 x 33 nodes, edges shared), 96 x 96 blocks a tile;
- each coarser level from four blocks below, down to 3 x 3;
- the whole tile.
Each value SHALL be a `u16` over `[-H, H]` with `H` the body field's `max_abs_height_m`, in steps of `2 H / 65535`, the min rounded down and the max rounded up. The pyramid SHALL be written and read with its tile, through a `planet-generators` trait that `planet-cache` implements (CON-02). The block size and the precision are start values (SPIKE-83).

Verify: A
Status: planned
Source: ADR 0088 point 8, ADR 0088/0089 accepted 2026-11-01; SPIKE-83 (32 x 32 blocks; about 90 MB for the planet, built in seconds)

#### Scenario: Rounded outward and nested
- **WHEN** a tile's pyramid is built and read back
- **THEN** every stored min is at most and every stored max at least the true value of its block, each by under two steps, and each coarser block holds its children's range

### Requirement: STRM-067 The class weights are stored with the bake
The class-weights lattice (GEN-093, its geometry and its `u8` weights) SHALL be stored with the global bake as one channel with its class-list version, because class paint makes it more than a function of the seed. A store whose class-list version differs from the generator's SHALL be refused, naming both versions. A store without the channel SHALL open as before the channel existed. The channel is record kind 11 (`GlobalClassWeights`), one record per cube face filed under the face's level-0 tile; its key carries the bake's stage key as its source, the bake's resolution under tag 4, the cells per face edge under tag 1 and the number of classes under tag 14 (not the class-list version, so that a store of another version is found and refused). A record's payload is `codec (u8, value 1) | class-list version (u32 LE) | class-source version (u32 LE) | cells (u32 LE) | the lattice's content hash (16 bytes) |` the face's weights (`u8`, row `j` outer, then `i`, 8 bytes a point). A store with records of the kind but of another class-list version, an incomplete face set or weights that do not match the hash is an error naming the record. The reader returns the stored class-source version and the caller compares it with the generator's (the key does not carry it); the class weights SHALL be loaded before the relief channels, because a changed class-list version is refused for the weights but reads as absent for the channels. The layout is in the m9-bake design.

Verify: A
Status: planned
Source: ADR 0089 decisions 1 and 8, ADR 0080 point 3 (its "not stored" ends with ADR 0089's acceptance), ADR 0088/0089 accepted 2026-11-01

#### Scenario: Old and new stores
- **WHEN** `global16384` (without the channel) and a new store with the channel are opened
- **THEN** the old store opens unchanged, the new one reads the weights byte for byte as written, and a store of another class-list version is refused

### Requirement: STRM-068 The relief channels are stored with block ranges
The relief channels of GEN-099 (`k` and `m`, f32 on the coupled lattice, min and max per 32 x 32 block) SHALL be stored beside the global bake, versioned, as record kind 10 (`GlobalRelief`), one record per cube face filed under the face's level-0 tile: `codec (u8, value 1) | cells (u32 LE) |` the face's `k` plane then `m` plane (f32 little-endian, row `j` outer, then `i`). The block ranges are not stored: the reader recomputes them from the planes. The key carries the bake's stage key as its source, the bake's resolution (tag 4), the cells (tag 1) and, under tag 13, the channels' version, the class-list and class-source versions, the hash of the class lattice the channels were computed from, the step limit `max_step`, the ring radius and the taper: a changed lattice, version or parameter reads as absent, never as wrong. A store without the kind opens and reads as absent (the same as before the channel existed). Liveness (STRM-013): records of kinds 10 and 11 are judged like the bake's blocks (generator version, hash version and node set id equal the world's); a stale record is dropped by compaction. Together with the class-weights channel (STRM-067) they SHALL stay under 1 GB for a planet of Earth's size; the limit is a start value from the m9-bake design.

Verify: A
Status: planned
Source: ADR 0088 point 1, ADR 0088/0089 accepted 2026-11-01; SPIKE-66, m9-bake design

#### Scenario: Block ranges and size
- **WHEN** the channels of a synthetic bake are stored and read back, and their size for a planet is computed from the lattice sizes
- **THEN** every value lies in its block's stored range, and the planet size is under 1 GB
