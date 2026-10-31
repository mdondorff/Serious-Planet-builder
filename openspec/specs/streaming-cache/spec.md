# streaming-cache Specification

## Purpose
Getting generated data to the renderer without blocking a frame (CON-22): injected time, I/O and spawning (CON-04), content-addressed persistent keys, the tile pack store with verification and compaction (CON-05, ADRs 0018, 0019, 0024, 0025, 0029), the persisted global bake blocks and stage outputs (ADR 0026), and the scheduling, cancellation and memory budgets of the residency system (CON-20, CON-21).

## Requirements

### Requirement: STRM-001 Injected time, I/O and spawning
Streaming and scheduling code SHALL receive time, I/O and task spawning through injected interfaces, and tests SHALL be able to run it with a fake clock, fake I/O with injectable latency and failures, and a deterministic single-threaded spawner. The start-up load of the roots (the tiles at the render floor, LOD-011: six level-0 tiles at floor 0, 384 level-3 tiles at floor 3) SHALL hand all its jobs to the spawner before it waits for any, so a pool loads them concurrently; the tiles SHALL become resident in tile-id order (face, then position) once all have arrived, so the resident set, its slots and its change order do not depend on the spawner, the thread count or the order the jobs finish (CON-14); the load of the roots is exempt from the per-frame upload cap (STRM-006, a declared exception, ADR 0064 amending ADR 0033 point 9: 384 slots are 3.4 MiB, three and a half frames at the cap, and a floor tile has no coarser stand-in), so all roots are written in the first frame, before it is drawn, as a load step of the body, not a steady frame (the same reading as the upload of a body's static tables); the roots stay resident for the body's life (STRM-007 never evicts them), so a teleport never reloads them, and the exemption is that of the tiles at the render floor level, so in practice that load only: every other tile counts against the cap, and the cap and the one-tile rule govern the tiles behind the roots in the first frame too; the readiness report (ADR 0045 point 5) SHALL show the roots loaded so far in the window title while they load; if one fails, the others SHALL be cancelled, nothing SHALL become resident, and the error SHALL name the failing tile.

Verify: A
Status: active
Source: CON-04, CON-14, LOD-011, ADR 0045 point 5, ADR 0062, ADR 0064

#### Scenario: Fake clock
- **WHEN** a scheduler test advances a fake clock by 5 s
- **THEN** exactly the jobs due within 5 s run, in a reproducible order

#### Scenario: Concurrent roots
- **WHEN** the roots (six at floor 0, 384 at floor 3) are loaded on the manual spawner and on pools of 1, 2 and 8 threads
- **THEN** all jobs were handed over before the first wait, a pool as large as the root count has all loads in flight together, and the resident set is identical in every case

#### Scenario: A failing root
- **WHEN** one root fails to load
- **THEN** the other roots are told to stop, no root is resident, the slots are given back and the error names the failing tile and its cause

#### Scenario: Roots beyond the cap
- **WHEN** the 384 floor tiles are loaded at start-up with the default cap of 1 MiB
- **THEN** all 384 are written in the first frame, before it is drawn (3.5 MiB at floor 3 with the 9,488 B slot of GEN-025, more than the cap), and every later frame writes at most the cap or one tile

#### Scenario: Teleport keeps the roots
- **WHEN** the dive and a teleport to the antipode run over many frames
- **THEN** no floor tile is evicted or loaded again after start-up, and the bytes of every later frame obey the cap

### Requirement: STRM-002 Never block a frame
For every visible node some level of detail SHALL be available: missing tiles SHALL be requested parent-first, and a frame SHALL never wait for generation, I/O or a plan (STRM-016).

Verify: A
Status: active
Source: CON-22, ADR 0032 point 5

#### Scenario: Fast dive
- **WHEN** a scripted orbit-to-ground dive runs with fake I/O of 200 ms latency
- **THEN** every frame has a resident tile for every visible node

#### Scenario: Teleport
- **WHEN** the camera jumps to the other side of the planet while the plan of the new place is still being computed
- **THEN** the frame after the jump draws a cover plan from the resident tiles (never blank, no height field evaluated by the frame), and the frame never waits for the plan

#### Scenario: Mouse look
- **WHEN** a seeded script of yaw and pitch steps with reversals and a 180 degree turn runs at a fixed position
- **THEN** every node of each frame's own plan is served by a drawn tile of its own, an ancestor or its descendants

#### Scenario: Rotation at the settled state
- **WHEN** a camera at the settled state (R resident) only turns, at any rate up to a 180 degree turn in 0.2 s, with and without pitch changes
- **THEN** in every frame the detail lag against the fresh plan of the frame's own camera is 0 and no node of it is uncovered, and nothing is requested, cancelled or uploaded (ADR 0031 point 3: rotation never fetches and never blurs)

### Requirement: STRM-003 Content-addressed keys
A persistent tile cache key SHALL be the first 16 bytes of BLAKE3 (`derive_key` mode, context `planet-builder cache key v1`) over a canonical encoding of everything the artefact depends on (ADR 0018, clarified by ADR 0023): the format tag, the kind, the hash version and generator version, the implementation id, the face mapping id, the body id and the 128-bit body parameter hash (id, seed, radius, reference surface), the tile id, the key of its source artefact with a presence byte (a mesh from its heights), its output parameters (resolution, grid size, skirt fraction, mesh version) in ascending tag order, and the read set as entries (a node entry with the 128-bit content hash of the node read, and a query entry with the kind, the parameters and the 128-bit result hash of each spatial query, an empty result included), nodes sorted by path then queries by kind and parameters, duplicates collapsed. A cell or body query's result hash covers the matching items' ids, content hashes and direction bits in id order; a footprint query (kind 3, WDEF-007) covers the items in application order (priority, then id big-endian), each as a 16-byte id and a 128-bit item hash that contains its anchor, reach, bound, layer and priority. A region summary entry (kind 4, WDEF-008) has the parameters `body u32 | layer (u32 length + UTF-8) | level u8 | node raw id u64` and the 16-byte node hash itself as its result, a cell hashed under `planet-builder cell hash v1` and an inner node under `planet-builder summary node v1`. The format tag is 2 for a key that carries no footprint (kind 3) or region summary (kind 4) entry, byte for byte as before, and 3 for a key that carries one, so no stored entry is invalidated and the two layouts never alias (ADR 0066 point 2). Every variable-length part SHALL be length-prefixed and every list preceded by its count, every hash that feeds a key SHALL have its own `derive_key` context, a change of any input SHALL change the key, equal keys SHALL imply equal tile bytes (checked by STRM-004, `cache-verify`), and the encoding SHALL have a version tag and pinned known answers that are the same on every platform.

Verify: A
Status: active
Source: CON-05, CON-06, CON-14, ADR 0006, ADR 0018, ADR 0066

#### Scenario: Version bump
- **WHEN** the generator version changes
- **THEN** every key changes and no stale tile is served

#### Scenario: Every input
- **WHEN** any single input of a key changes (kind, versions, implementation, mapping, body, body parameters, tile, source key, an output parameter, a node entry, a query entry)
- **THEN** the key changes, and a set of generated inputs gives distinct encodings and keys

#### Scenario: Orders
- **WHEN** the same output parameters or reads are given in another order or with duplicates
- **THEN** the key is the same

#### Scenario: Reads
- **WHEN** a field that reads cells is keyed
- **THEN** its persistent key follows the same per-tile reads as its 64-bit key (one source), and a field that says it reads cells but reports none cannot be keyed

#### Scenario: Empty query
- **WHEN** a spatial query found nothing and an item is then added to that cell, or is added elsewhere
- **THEN** the key changes in the first case and not in the second

#### Scenario: Format tag per key
- **WHEN** a key carries no footprint or summary entry, and when it carries one
- **THEN** the first has tag 2 and the bytes it always had (the pinned known answers are unchanged) and the second has tag 3

### Requirement: STRM-004 Cache verification
`cargo xtask cache-verify <dir>` SHALL regenerate an evenly spaced sample (or all) of the entries of a tile store from the definition (`--world <file>` or the default planet with `--seed`; both together are refused) and compare each with what the store holds, and SHALL refuse a directory that does not exist. Every entry SHALL be decoded first, whatever its key: an entry that cannot be read or decoded, and every record the pack scan found unusable, SHALL be reported with its pack, offset and key and fail the command. An entry whose key is the key the current definition gives for its tile SHALL carry the generator version, hash version and node set id the definition gives and SHALL equal the regeneration sample for sample (bit for bit, judged on the decoded heights, not on the compressed bytes); a difference SHALL fail the command and be reported with the tile (id, face, level, x, y), the pack and its offset, the key and either the disagreeing metadata or the first differing sample with both values. An entry under a key the current definition no longer gives SHALL be counted as stale: *explained* when its versions or node set id differ from the current ones, *without a known reason* when they are all equal (a node value changed, or the key function drifted); stale entries are not failures, but a run in which nothing could be compared (an empty store, only entries of another kind or all entries stale) SHALL fail, unless `--allow-stale` says the definition changed on purpose and at least one entry is stale (ADR 0025 point 4). Entries of a kind the verifier does not regenerate SHALL be counted. `generate --cache-dir <dir> --region FACE,LEVEL` SHALL write the tiles of one face at one level (at most level 6, at most 4 GiB of raw tiles, a resolution the codec accepts) into a store, reusing tiles already stored under the current key whose payload decodes, so that a second run writes nothing. The store options SHALL be refused by every other mode and when combined with options they would silently ignore.

Verify: A
Status: active
Source: CON-05, report §16, ADR 0006, ADR 0019, ADR 0025

#### Scenario: Missing input in a key
- **WHEN** a cached tile was produced with an input that is not part of its key, and that input's value differs between building and verifying (or the record was written wrongly)
- **THEN** cache-verify reports the tile ID, the pack offset and the first differing sample (or the disagreeing metadata), and fails

#### Scenario: Clean store
- **WHEN** every sampled entry was generated from the current definition
- **THEN** all match and the command passes, whatever the sample size

#### Scenario: Changed definition
- **WHEN** the definition changed since the entries were stored
- **THEN** they are counted as stale (explained or not), and the command fails because it compared nothing unless `--allow-stale` is given or other entries do match

#### Scenario: Nothing to compare
- **WHEN** the store is an existing empty directory, has only entries of another kind or only packs that are skipped
- **THEN** the command fails because it compared nothing, with or without `--allow-stale` (no entry is stale)

#### Scenario: Damage
- **WHEN** a payload fails its CRC, is damaged inside a stale entry, or the pack scan skipped a record
- **THEN** it is reported with pack, offset and key and the command fails

#### Scenario: Incremental invalidation
- **WHEN** one authored item is edited and the region is built again
- **THEN** exactly the tiles whose footprint and margin hold its cell are written and the rest are reused

### Requirement: STRM-008 Tile store
Tiles SHALL be stored in append-only packs with a rebuildable index (ADR 0019, clarified by ADR 0024): each record carries its content key, kind, tile, versions, node set id, payload length and a CRC-32C of the payload and of its own header and is never updated in place; the index is a sorted array ordered by (kind, face, level, Hilbert index of the tile) and strictly increasing in (that sort key, content key), so one record has one entry, written atomically and always rebuildable from the data by one scan, and a missing, damaged or stale index, or one that names records outside the data, SHALL be rebuilt, never trusted. A torn tail (a record cut short, or a zero-filled remainder) SHALL be truncated by the writer and ignored by readers; a record whose payload fails its CRC SHALL be skipped (a miss to regenerate, naming the file, offset and key when it is read, and then a plain miss for that store, which an older pack's copy of the key does not answer, so a long-lived store reports it once) without cutting the records after it; a header that fails its CRC in the middle of a pack SHALL stop readers there and leave the pack untouched while the writer continues in a new one; a file that ends before the length a scan took from its metadata (cut under the reader) SHALL be a torn tail at that position, and any other read error SHALL be an error, never a torn tail. A cache SHALL degrade to misses: a pack still being created, shorter than its header, made under an older hash version or not a pack SHALL be skipped by readers and reported, never fail the store, and a pack of another body SHALL be refused naming the file. A lookup SHALL binary-search the run of entries with the tile's sort key and match the content key, several keys per tile SHALL coexist, and when a key is held twice the later record in a pack and the newer pack win. A pack SHALL roll over at a size limit and sequence numbers SHALL never wrap. One writer per directory SHALL hold an advisory OS lock that a crash releases, across rollovers, a second writer SHALL be refused naming the holder, and readers SHALL need no lock. The Hilbert curve SHALL be the standard one (consecutive indices are neighbouring cells, the four children of a cell take four consecutive indices) and the CRC the standard CRC-32C.

Verify: A
Status: active
Source: CON-05, CON-22, ADR 0003, ADR 0019, ADR 0024

#### Scenario: Round trip and ordering
- **WHEN** records are appended, the pack finished and the store opened
- **THEN** every record is found by its kind, tile and key, an unknown key is a miss, and index entries are in Hilbert order within a kind, face and level

#### Scenario: Crash
- **WHEN** a writer crashed mid-record, or left a zero-filled tail
- **THEN** readers ignore the tail and the writer truncates it; a lock file left behind blocks nothing, and a writer process that is killed releases its lock

#### Scenario: Damage
- **WHEN** a payload byte flips, or a record header byte flips
- **THEN** a flipped payload is a miss for that record only (the records after it stay readable and no writer cuts them), and a damaged header leaves the pack untouched for a writer that continues in a new pack

#### Scenario: Repeated damage
- **WHEN** a long-lived store meets the same payload that fails its CRC twice
- **THEN** the first lookup is `Corrupt` and later ones a miss, even if an older pack holds an intact copy of the key

#### Scenario: Cut under a scan
- **WHEN** a pack file is shorter than the length a scan took from its metadata
- **THEN** the scan ends in a torn tail at the first incomplete record instead of failing

#### Scenario: Index
- **WHEN** the index is missing, damaged, older than the data or names records outside it
- **THEN** it is rebuilt by a scan and the rebuilt index equals the one a writer writes

#### Scenario: Writers
- **WHEN** a second writer opens a locked directory, even while the first rolls packs over
- **THEN** the second is refused naming the holder

#### Scenario: Stores
- **WHEN** a pack is of an older hash version, still being created, or not a pack
- **THEN** readers skip it and report it, and a pack of another body is refused naming the file

### Requirement: STRM-009 Height tile payload
A height tile SHALL be stored as a payload with a codec version, its resolution and the raw length, followed by an LZ4 block of its `(res + 1)^2` heights as little-endian `f32` bytes shuffled by byte index (ADR 0019). Decoding SHALL give back every `f32` bit pattern exactly (NaN payloads, signed zeros, subnormals), SHALL refuse a payload that is short, of another version, of a resolution that is not a power of two up to 4096, whose raw length does not match its resolution or whose block is damaged, and SHALL never allocate more than the resolution allows. The encoder SHALL refuse a tile the decoder would refuse (a resolution that is not a power of two up to 4096, a number of heights that does not match). Tiles SHALL be compared on decoded heights, never on compressed bytes.

Verify: A
Status: active
Source: CON-05, CON-14, ADR 0019, ADR 0025

#### Scenario: Round trip
- **WHEN** a tile is encoded and decoded
- **THEN** every height has the same bits, and a smooth tile is smaller than its raw size

#### Scenario: Layout
- **WHEN** a hand-built payload of the documented layout is decoded
- **THEN** it gives the expected heights, independent of the compressor in use

#### Scenario: Hostile payload
- **WHEN** a payload is truncated, has another version, resolution or raw length, or a damaged block
- **THEN** decoding fails with an error naming what is wrong

### Requirement: STRM-010 Node set id
Every record of a world SHALL carry a node set id: the first 16 bytes of BLAKE3 `derive_key` (context `planet-builder node set v1`) over the paths of the field's field-wide reads and of the body's radius node (`body.0.radius_m`), deduplicated, sorted by UTF-8 byte order and concatenated, each as its byte length (u32 little endian) followed by its UTF-8 bytes, with no count prefix (ADR 0025 point 3). It SHALL depend on which nodes are read and not on their values, SHALL be equal for the records of one world, and its encoding SHALL have a pinned known answer that is the same on every platform. It is per world for a field that reads no items; for a field that reads items (`reads_cells`) the id function (`node_set_id_of_tile`) SHALL give a per-tile id, which a record writer of such a field SHALL use (the writers are wired with the first stored item-reading stage, E2; no production field reads items before then), and it also SHALL cover the identities of the tile's own item queries (for a footprint query: kind, body, layer, tile id and halo bits; for a region summary node: kind, body, layer, level and node id; never a result or a hash), so it depends on which reads the tile makes and not on what they found, and equal tiles of a world have equal ids (ADR 0025 point 3, ADR 0066).

Verify: A
Status: active
Source: CON-05, CON-14, ADR 0019, ADR 0025, ADR 0066

#### Scenario: Pinned encoding
- **WHEN** the node set id of the default planet is computed by hand from its sorted paths and compared with the library's
- **THEN** they are equal and equal to the pinned known answer, another seed gives the same id, and every stored record carries it

### Requirement: STRM-011 Stage output keys
A staged field output that is stored SHALL be keyed by the first 16 bytes of BLAKE3 in `derive_key` mode (context `planet-builder stage key v1`, so it can never equal a height or mesh key) over a canonical encoding with its own format tag: the hash version, the field's version, stage tag, id, implementation id and halo (as bits, `-0.0` read as `0.0`), the scope (the whole body, or one tile; the tile with raw id 0 never aliases the global scope), the body id and the 128-bit hash of its parameters, the face mapping id, the output resolution, the read set of the definition as entries (nodes by path, then queries by kind and parameters, duplicates collapsed) and the 128-bit keys of the outputs of the fields it reads, by field id (ADR 0022, ADR 0023 point 10, ADR 0026 point 6). A change of any of them SHALL change the key, a version bump of one field SHALL re-key exactly its consumers, the order of inputs and reads SHALL NOT matter, a missing, extra or unknown input and a scope of the wrong kind for the field's stage SHALL be programming errors that panic, and the encoding SHALL have a format tag, a pinned layout (ADR 0026 point 6) and known answers that are the same on every platform. A bake stored as blocks keeps this one stage key; each block is a kind-3 record whose content key chains it (STRM-012, ADR 0026 point 6), and a reader lists the stage key once (ADR 0027 point 6).

Verify: A
Status: active
Source: CON-05, CON-07, CON-08, CON-14, ADR 0006, ADR 0022, ADR 0023, ADR 0026

#### Scenario: Re-keying
- **WHEN** one field's version or halo, or the resolution, face mapping, body, parameters or reads change
- **THEN** exactly the keys that depend on it change

#### Scenario: Scope and inputs
- **WHEN** a global field is keyed for a tile, a regional field globally, or an input is missing or extra
- **THEN** it panics

#### Scenario: Pinned layout
- **WHEN** the encoding of a small global field is written out byte by byte
- **THEN** it equals the encoding, and the keys of a three-stage example equal their known answers

### Requirement: STRM-012 Global bake blocks
A global-coarse bake SHALL be stored in the tile store as blocks (ADR 0026 points 2 and 6): block `b` of a grid of `res = B << L` cells per face edge is the level-`L` tile `b`, a pack record of kind 3 holding its `(B + 1)^2` samples of four `f32` channels (continentalness, elevation, temperature, precipitation). The payload SHALL be `codec version (u8 = 1) | B (u32 LE, a power of two up to 1024) | raw length (u32 LE, 16 (B + 1)^2) | LZ4 block` of four planes in that channel order, each `(B + 1)^2` little-endian `f32` row-major (`k` fastest) and byte-shuffled, one plane after the other; decoding SHALL give back every `f32` pattern exactly, SHALL refuse a short payload, another version, a `B` that is not a power of two up to the limit, a wrong raw length or a damaged block without allocating more than `B` allows, and the encoder SHALL refuse what the decoder would. The bake SHALL have one stage key (STRM-011, `Scope::Global`, `res` as the resolution), and each block's content key SHALL be the ADR 0018 encoding with kind 3, the block's tile, the stage key as its source, `B` under tag 1 and `res` under tag 4, the face mapping and body parameter fields as in the stage key and an empty read set; two blocks of one bake, another `B`, another `res`, another seed and another body SHALL have different keys, and the keys SHALL have pinned known answers. The node set id of every block SHALL be ADR 0025 point 3's construction over the body's seed node path only (`world.seed` for body 0, `derived.body.N.seed` otherwise; not the radius, which only the body parameter hash covers; ADR 0026 consequences), with a pinned known answer written out from the ADR text. Only the canonical layout of a grid SHALL be stored: blocks of `min(res, limit)` cells (limit 1024) that are the tiles of the level that covers the grid (ADR 0027 point 1), derived from `res` by the build and checked by the keyer and by verification (another layout is refused or unreadable); a stored bake SHALL be readable as a lattice source for the reader of GEN-015 that finds each block through that layout and names a missing or damaged block, its key and why. A block's grid (`res`) SHALL be derived from its tile level and its cells, never passed separately, and a record whose cells and level make no grid the tiled bake accepts SHALL be unreadable, never keyed. A build SHALL refuse a bad shape before it opens or creates anything, SHALL write again a block whose stored payload has other cells than its key says, SHALL accept no further block for computing after a write fails (the batch already running finishes), and SHALL bound the blocks alive by a memory budget whatever the thread count. Building SHALL write the blocks the store does not hold under their current key and compute no others, hold at most one batch of blocks in memory, and write nothing on a second run. `generate --global-dir --global-res N` SHALL build a bake of N cells per face edge and print the time per sample, `generate --verify-global` SHALL regenerate stored blocks and compare them bit for bit by the rules of STRM-004 (decode first, stale entries counted, a run that compared nothing fails unless `--allow-stale` and something is stale, a missing directory refused), `generate --global-products` SHALL write the cube nets and maps, and options they would ignore, combinations of them and a resolution that is not a power of two or is finer than the stated finest grid SHALL be refused naming the limit.

Verify: A
Status: active
Source: CON-05, CON-14, ADR 0018, ADR 0019, ADR 0025, ADR 0026

#### Scenario: Round trip and layout
- **WHEN** blocks of 1 to 32 cells with NaN patterns and signed zeros are encoded and decoded, and a one-cell block is written out by hand
- **THEN** every bit comes back and the bytes equal the documented layout

#### Scenario: Keys
- **WHEN** a block key is computed for another tile, `B`, `res`, seed or body
- **THEN** it differs, and the stage key, block key and node set id equal their known answers

#### Scenario: Build and verify
- **WHEN** a bake is built twice and verified, then verified under another seed, or with a block made from another seed or a damaged payload
- **THEN** the second build writes nothing, the first verify matches every block, the other seed is stale and compares nothing, and the bad blocks are reported with tile, pack, offset and first differing sample

### Requirement: STRM-013 Compaction and the size cap
`planet_cache::pack::compact` (`cargo xtask cache-compact <dir>`, `planet generate --compact-cache <dir>`) SHALL rewrite the live records of a store directory into fresh packs and remove the packs it read, under the directory's writer lock (a held lock is refused, naming the holder), never editing a pack in place (ADR 0019 space reclamation, ADR 0024). A record is live when the caller's predicate says so (for the CLI: a height tile or a global block whose generator version, hash version and definition node set id equal the current definition's; a record of a kind the build does not know is kept); each key keeps its winning copy only (the newest pack, the later offset within a pack), and a key whose newest copy is damaged is dropped with the older copies it hides (ADR 0024 point 8). Packs of another hash version SHALL be removed with the packs read, a file that is not a usable pack of the store SHALL be left alone and reported, and a pack of another body SHALL be an error that removes nothing. The kept records SHALL be written in index order (kind, face, level, Hilbert, then content key) into packs that roll over at the size limit, numbered after the highest existing sequence number, so the output bytes are a pure function of the set of records kept: independent of which pack held a record, of the order the records were first written, of dead and duplicate records and of timing (CON-14); compacting an already compact store rewrites the same bytes under new sequence numbers; the report carries a BLAKE3-128 hash of a framed stream of the output packs (per pack: the marker "pack", its header, its records, the marker "end", its length), independent of file names. When a cap in bytes is given and the live records exceed it (headers and payloads of the kept records; pack headers and index files come on top), records SHALL be evicted by rank, lowest first, ties to the later index order, stopping at the first record that does not fit: the default rank is the sequence number of the pack of the winning copy (write order, not last use; after a compaction the ranks tie and the later index order goes first, ADR 0029); the kept set therefore depends on the rank input, and for a given rank the output bytes are as above; the rank is an input, so a last-hit sidecar (ADR 0019) replaces it without changing the mechanism. Records after a damaged record header in a pack are dropped (they were unreachable); a key with an intact copy and a CRC-failed copy in the same pack keeps the intact one, as the reader does. Eviction happens only through this rewrite. Not covered: an entry whose read nodes changed value but not path set cannot be told dead from the metadata (ADR 0019); until the node set id is per tile (ADR 0026), a global block's liveness is that of the bake's seed node.

Verify: A
Status: active
Source: CON-05, CON-14, CON-22, ADR 0019, ADR 0024, ADR 0025, ADR 0029

#### Scenario: Dead and superseded records
- **WHEN** a store holds records of an old generator version and older copies of keys written again
- **THEN** compaction keeps one copy of each live key, every kept record reads back equal, and the dead ones are gone

#### Scenario: Byte determinism
- **WHEN** the same live set was written in different orders, split into different packs and with dead records mixed in
- **THEN** the compacted packs are byte for byte equal and their hashes match

#### Scenario: Size cap
- **WHEN** the live records exceed the cap
- **THEN** the lowest-ranked (by default the oldest) are evicted, the kept bytes are within the cap, and a second run changes nothing

### Requirement: STRM-014 Concurrent store source
A stored global bake read as a lattice source (STRM-012) SHALL return, for every vertex, the same bits whatever the number of threads reading it, the order of the reads and the state of its block cache, and SHALL let concurrent readers of already-decoded blocks proceed without a lock on the read path (the 20-thread rate over the store is at least 80 % of the in-memory rate: measured ad hoc; asserted by proxy, as the shared lock being taken only when a thread changes block, and one decode per distinct block). Its decoded blocks SHALL be bounded by a byte budget (default 512 MiB, at least one block): blocks resident in the shared cache (including blocks being decoded) SHALL never exceed it. Threads that want the same block that is not resident SHALL share one decode. A block that cannot be read SHALL be reported to every reader that wanted it, SHALL not be cached, and SHALL be read again by readers that arrive after the failed read (readers already waiting for it get its error).

Verify: A
Status: active
Source: STRM-012, CON-14, CON-20, ADR 0027 point 7, ADR 0033, M4 item 7b

#### Scenario: Identical at any thread count
- **WHEN** tiles of the eroded field are generated over a stored bake at 1, 4 and 20 threads (tier A: 1 and 3 threads on four tiles of every face (levels 0 to 2 and (4, 9, 15)); the full sweep is tier B, `oracle_concurrent_source`)
- **THEN** every tile is bit-identical to the others and to the tile over an in-memory bake of the same grid

#### Scenario: One decode per block
- **WHEN** many threads read the same blocks of a cold source whose budget holds them
- **THEN** every block is decoded exactly once

#### Scenario: Lock only on a change of block
- **WHEN** 20 threads generate tiles over a stored bake whose budget holds the working set
- **THEN** the shared-cache lock is taken for a small fraction of the lattice reads, and the decodes equal the decodes of one thread (the distinct blocks)

#### Scenario: Budget
- **WHEN** reads touch more blocks than the budget holds
- **THEN** resident bytes never exceed the budget and every read still returns the stored sample

#### Scenario: Missing block
- **WHEN** a block is missing or damaged
- **THEN** every reader gets the error naming the block, its key and why, and nothing is cached

### Requirement: STRM-005 Cancellation and prioritisation
Requests SHALL be in three classes, highest first: the missing nodes of the draw set D (ancestors before descendants; within the class by screen-space error times visibility), the rest of the residency set R (coarse before fine, nearer before farther; the view direction may order within this class but not change its membership), and prefetch (R at a position predicted from the camera translation only; yaw and pitch rates are never extrapolated). Prefetch SHALL be requested only while no request of the other two classes is waiting or running, and only into free capacity, and SHALL never evict a tile of R or D. A queued or in-flight request SHALL be cancelled only when its tile is in none of D, R and the prefetch set (with their ancestors); leaving the view frustum is never a reason. When the prefetch set changes (the travel direction changed), its requests that are no longer in it are cancelled first. An in-flight job SHALL stop at its next height sample, and the tile of a cancelled job SHALL NOT become resident (a cancelled job is never revived: if its tile is needed again it is requested anew after the job ends). Dropping the streamer (a teardown) SHALL cancel every request that is not resident: the queued ones never reach the spawner, the dispatched ones are told to stop, a dispatched job that has not started SHALL end without calling the source, and a job in flight SHALL stop at its next height sample, so a teardown waits only for the jobs in flight and for their current sample. Dropping the plan pipeline SHALL close its worker: a plan job that has not started does not run, and a running one starts no residency set, prefetch set or follow-up plan after its own. A teleport is served by the same path: the requests that left R and the prefetch set while still queued are cancelled before they start. The order of requests SHALL be a pure function of the inputs, independent of the thread count (CON-14).

Verify: A
Status: active
Source: report §4, ADR 0031 points 2, 3 and 6, CON-04, CON-14

#### Scenario: Rotation requests nothing
- **WHEN** the camera turns through 360 degrees at a fixed position with R fully resident and fitting the capacity
- **THEN** nothing is requested and nothing is cancelled (while R is still filling, a rotation only reorders queued requests)

#### Scenario: Cancel
- **WHEN** a node leaves R before its job starts
- **THEN** the job is cancelled and never runs, and a node that only leaves the frustum is not cancelled

#### Scenario: Order
- **WHEN** D, the rest of R and a prefetch set are all missing
- **THEN** D is dispatched before the rest of R and that before prefetch, ancestors before descendants in D and coarse before fine in R

#### Scenario: Prefetch is translation only
- **WHEN** a prefetch position is predicted for a camera with a velocity and any yaw or pitch rate
- **THEN** it depends on the position, the velocity and the look-ahead only, and prefetch requests wait for free capacity and for the higher classes

#### Scenario: Travel change
- **WHEN** the prefetch set changes while prefetch requests are queued
- **THEN** the queued requests outside the new set are cancelled and the others stay

#### Scenario: Teardown
- **WHEN** the streamer is dropped with N requests queued, some dispatched jobs waiting in the spawner and k jobs in flight
- **THEN** no queued or waiting job calls the source, each of the k jobs in flight stops at its next height sample, and no tile becomes resident

#### Scenario: Teardown of the loop
- **WHEN** the managed loop is dropped with a plan job waiting in the plan worker's spawner, or with a plan job running and a newer request pending
- **THEN** the waiting job plans nothing, and the running job finishes its plan and selects no residency set and spawns no follow-up job

#### Scenario: Teleport
- **WHEN** the camera jumps while many requests of the old R are queued
- **THEN** the requests the new R does not contain are cancelled before they start, and every request started after the jump is in the new wants

#### Scenario: Stage boundary
- **WHEN** an in-flight job is cancelled
- **THEN** it stops before its next stage and its tile does not become resident

#### Scenario: Rotation through the loop
- **WHEN** the managed frame loop (STRM-017) runs a 360 degree turn at a fixed position with R resident, plans and R coming from the worker and tiles from fake I/O
- **THEN** every frame of the turn dispatches, cancels, queues, uploads and evicts nothing, the worker reuses the previous R instead of selecting one, and each settled frame equals the synchronous plan of its view

#### Scenario: Order through the loop
- **WHEN** the camera jumps and the loop fills the new place
- **THEN** every request of the new draw set with its ancestors precedes the first request of the rest of R, ancestors precede descendants, and every frame draws something with no blank node

### Requirement: STRM-006 Upload and memory budgets
Terrain SHALL have its own share of the VRAM cap (2 GiB at the 8 GiB mid tier, 3 GiB at the 14 GiB high tier, configurable), a ceiling and not a target, counted in pooled fixed-size slots so that a test needs no device. The allocated terrain bytes (the blocks of the pool, index buffers, node records) SHALL be a pure function of the pool configuration and SHALL never exceed the share; a configuration whose fixed items exceed their reserve (0.125 GiB) or that holds no block is refused. The pool holds the compact format (ADR 0033 point 3, REND-016), the only tile format (ADR 0062): a block is one storage buffer of at most 128 MiB (ADR 0054; 14,146 slots of 9,488 bytes = 134,217,248 B at cells 32, GEN-025), so the mid share holds 15 blocks = 212,190 slots and the high share 23 = 325,358 (ADR 0033 point 3 re-cut: -8.2 % against 231,090 and 354,338; the driver block of GEN-074 grew ADR 0063's slot); the record and the climate block are part of the slot and charged per slot, so the fixed reserve holds no per-slot item and no tile size can overflow it with records (the share is a ceiling). A tile SHALL live in one slot of a pool with a free list; allocated, retiring and free slots SHALL always add up to the pool's capacity, and a slot that was given back SHALL NOT be reused until the frames in flight that may read it have retired. Resident tiles plus jobs in flight SHALL never exceed the pool's capacity in slots. When the share is smaller than the residency set R the manager SHALL degrade in this order and never touch the view first: tiles outside R and prefetch tiles are evicted first; then the leaves of R that are not in the draw set D, its ancestors or the roots are replaced by their ancestors (sibling groups merged), farthest first and, among equally far, finest first; only if D with its ancestors and the roots alone exceed the share (a budget failure, reported with the slot counts) are tiles of D replaced by their ancestors, farthest and finest first, as a declared exception to LOD-008's pixel guarantee. The result SHALL be a pure function of R, D and the capacity, and SHALL NOT depend on the order of the inputs. At the standard views the slots needed by R with its ancestors and by D with its ancestors SHALL stay within pinned counts and within pinned fractions of the share's slots.

The per-frame upload cap (ADR 0033 point 9) is a byte count of **tile data** written to the pool in one frame, counted per tile as the size of its upload in the sink's format (the compact slot of 9,488 B at cells 32; the cap is a configuration value, `UploadBudget::tile_bytes_per_frame`, starting at 1 MiB, about 110 compact slots at cells 32). Finished tiles the pool has not received SHALL wait in the order draw set D with its ancestors and the roots (ancestors before descendants, then plan order), then R with its ancestors, then the prefetch set, then the rest; a frame SHALL write the longest prefix of that order whose bytes fit the cap, SHALL stop at the first tile that does not fit (a tile is deferred to the next frame, never dropped, never overtaken by a later tile of any class), and SHALL always write at least one tile when any waits, so a tile larger than the cap is written alone and cannot starve; the cap is exceeded only by that one tile and by the start-up load of the roots (the tiles at the render floor, LOD-011), which is written in the first frame whatever its bytes, before the cap and the one-tile rule govern the tiles behind it (a declared exception, STRM-001, ADR 0064). A tile that is evicted while it waits is forgotten. The tiles that wait are not drawn: the frame draws their deepest resident ancestors or the cover (STRM-016), so no frame is blank for the deferral. The order and what is written are a pure function of the wants and the finished set, not of the thread count or the order the jobs finished (CON-14). The per-frame node records (the indirect path's `indirect_frame_bytes`) are a CPU write into a ring, cannot be deferred and are not tile data: they are reported as their own line (`node_bytes`) and count against CON-21's 2 ms (owner's decision, ADR 0033), outside the tile cap unless the configuration puts them inside (`node_records_in_cap`: the tile allowance is the cap minus the node bytes of the previous drawn plan). The cap of 1 MiB per frame is accepted (ADR 0033 point 9); changing it is a configuration change. The RAM split (point 8) is STRM-033.

Verify: A, C
Status: active
Source: CON-14, CON-20, CON-21, ADR 0031 point 4, ADR 0033 points 1-3, 9 (accepted 2026-10-16) and 10, ADR 0054, ADR 0032, STRM-002, STRM-016, STRM-017, GEN-025, ADR 0063, ADR 0062 point 4, SPIKE-12 Q3

#### Scenario: Terrain share
- **WHEN** the pools are configured for a tier
- **THEN** the allocated bytes (all blocks, index buffers, node records) are at most the tier's terrain share, one more block would not fit, and the real GPU allocation equals the model for the blocks in use

#### Scenario: Slot accounting
- **WHEN** slots are acquired and released in any order over many frames
- **THEN** in use plus retiring plus free always equal the capacity, no slot is handed out twice, and a slot released at frame f is not acquired before frame f plus the frames in flight

#### Scenario: Cap
- **WHEN** residency demand exceeds the pool
- **THEN** resident tiles plus jobs in flight stay at or below the capacity in slots, tiles outside R and prefetch go first, then R tiles farthest and finest first for draw requests, and D with its ancestors and the roots are never evicted

#### Scenario: Small share
- **WHEN** the share is smaller than R
- **THEN** the draw set and the roots stay at full detail, the surroundings are replaced by ancestors (farthest and finest first), every direction is still covered by some resident level, and the slots in use never exceed the share

#### Scenario: Budget failure
- **WHEN** D with its ancestors and the roots exceed the share
- **THEN** the report says so with the slot counts, the farthest and finest draw tiles are asked for as their ancestors, and the roots stay

#### Scenario: Headroom
- **WHEN** the ten standard views are planned over the default world, against the slots of `tier_compact_pool` (the pools the renderer allocates)
- **THEN** R with ancestors is at most 10 % and the ground-1m D with ancestors at most 6 % of the mid tier's 212,190 compact slots (R at most 10 % of the high tier's 325,358), the allocation model stays inside both shares, and the compact slot grew from 9,168 B to 9,488 B (`MESH_VERSION` 10: the driver block, GEN-074), so the pins are the same fractions of 212,190 and 325,358 slots

#### Scenario: Compact pools
- **WHEN** the tier pools are configured for the compact format at cells 32, 64 and 16
- **THEN** at cells 32 the mid and high shares hold 15 and 23 blocks of at most 128 MiB (14,146 slots at cells 32) and one more block would not fit, the fixed items (index buffers, node records) stay inside their reserve, and a tile size with many small slots per block gets fewer blocks

#### Scenario: Per frame through the loop
- **WHEN** the managed frame loop (STRM-017) turns through eight directions with a share much smaller than R
- **THEN** in every frame resident tiles and jobs in flight stay at or below the capacity, in use plus retiring plus free equal the capacity, no frame reports a budget failure, each view ends at full detail as the synchronous plan, and the frames report the tiles degraded and evicted

#### Scenario: Upload cap
- **WHEN** the managed frame loop (STRM-017) runs the dive script and a teleport on fakes with tiles of 58,608 bytes (larger than the compact slot, so that few tiles fit a frame's cap) and the default cap, tiles finishing faster than the cap lets them in
- **THEN** in every frame the bytes written are at most the cap or exactly one tile, a frame that defers is full (the next tile did not fit), the tiles written are in class order with ancestors first and nothing that waits ranks before the last tile written, the streamer's resident tiles are the sink's plus the waiting ones, and no frame is blank

#### Scenario: Larger than the cap
- **WHEN** every tile is larger than the cap
- **THEN** one tile is written per frame while any waits, the roots first, and the view is never blank once the roots are in

#### Scenario: Upload order on any thread count
- **WHEN** the same script runs with tile jobs finishing on 1, 2 and 8 threads
- **THEN** the tiles written, their bytes and the deferred counts of every frame are identical

#### Scenario: Fill time at the cap
- **WHEN** the 8,763 slots of a full ground-1m fill wait at the default cap
- **THEN** it takes 80 frames of 110 compact tiles (9,488 B each)

#### Scenario: Node records
- **WHEN** a frame is drawn
- **THEN** its node-record bytes are reported apart from the tile bytes, and shrink the tile allowance only when the configuration counts them inside the cap

#### Scenario: The record is charged per slot
- **WHEN** the tier pools are configured with the slot of 9,488 B and the fixed reserve of 0.125 GiB
- **THEN** the mid share holds 15 blocks and 212,190 slots and the high share 23 blocks and 325,358 slots, the fixed items stay inside their reserve

### Requirement: STRM-007 Resident capacity
The streamer SHALL count its capacity in pooled slots (STRM-006): a slot is reserved when a job is dispatched and given back when the tile is evicted, cancelled or failed, so resident tiles plus jobs in flight SHALL NEVER exceed the capacity. It SHALL evict only to make room for waiting requests, in this order: tiles in none of D, R and the prefetch set, and prefetch-only tiles, least recently used first; for draw requests only, then tiles of R that are not in D, farthest first and finest first. It SHALL NEVER evict a root (a tile at the render floor level, LOD-011) or a tile of the current draw set or one of its ancestors, SHALL NOT let a residency or prefetch request evict a tile of R, and SHALL retry failed tiles only after a configured delay measured on the injected clock. A tile that left R stays resident until the capacity needs its slot. A loop that mirrors the resident set into a renderer SHALL place each tile that becomes resident in the slot the streamer assigned to it and never choose a slot itself, SHALL forget each evicted tile before any other tile is written to its slot, and SHALL hold exactly the streamer's resident set; there is no exception (no tile lives outside its streamer slot, ADR 0062).

Verify: A
Status: active
Source: CON-04, CON-20, ADR 0031 point 3, ADR 0033 point 3, LOD-011, ADR 0062

#### Scenario: Interest moves away
- **WHEN** the wanted set moves to other tiles and the capacity is full
- **THEN** the oldest unused tiles are evicted to make room, roots and the draw set stay, the capacity is never exceeded, and a failed tile is not retried before the delay

#### Scenario: Order of eviction
- **WHEN** a draw request finds no free slot and the pool holds stale tiles, prefetch tiles and R tiles
- **THEN** the stale tile goes first, then the prefetch tile, then the farthest R tile, and the draw set and the roots are untouched

#### Scenario: No eviction by R
- **WHEN** a residency request finds no free slot and only R tiles could be evicted
- **THEN** it waits and nothing is evicted

#### Scenario: Reuse after retirement
- **WHEN** a tile is evicted for another
- **THEN** the other tile takes that slot only after the frames in flight have retired

#### Scenario: Placement follows the manager
- **WHEN** a loop mirrors the streamer's resident set into a renderer's slots over many frames of eviction and refill
- **THEN** after every frame the renderer holds exactly the streamer's resident tiles, each in the slot the streamer assigned, no slot holds two tiles, and a tile is written to a slot only after the streamer has evicted its previous tile

### Requirement: STRM-015 Residency set
The residency set R SHALL be a pure function of the camera position, the LOD parameters (LOD-008 distances and per-node height bounds) and the horizon cut. The view direction is not an input: yaw and pitch do not change it. R SHALL hold the selected leaves in all directions (the leaves of the selection clamped to the ceiling of the tiles too steep to morph, LOD-008, ADR 0074) and all their ancestors down to the render floor (a tile above the floor is never a member, LOD-011), in ascending tile-id order, each with a distance, and SHALL contain the draw set (LOD-007) of every view from the same position. In a frame loop R SHALL be computed on the plan worker from the camera position only, after the plan of the same job has been delivered (no frame waits for it), on the plan's own selection cache, so that R evaluates no origin height and no mesh key and adds only the height volumes of the tiles the plan's frustum cut did not visit (where a field can be too steep to morph the plan's own job walks the position-only selection too, to find the ceiling of LOD-008, so those volumes are then the plan's); a request with the same position and parameters as the previous R SHALL reuse that set without selecting anything.

Verify: A
Status: active
Source: ADR 0031 points 1 and 6, LOD-007, LOD-008, CON-03, LOD-011, ADR 0062

#### Scenario: Yaw independent
- **WHEN** the draw set is selected from one position for many yaw angles
- **THEN** every draw-set tile is in R, and R is identical for all of them

#### Scenario: Closed under parents
- **WHEN** R is computed
- **THEN** every ancestor of every member that is not above the render floor is a member, in ascending tile-id order, and no member is above the floor

#### Scenario: R on the worker shares the plan's cache
- **WHEN** a plan and then R are made from one position on one selection cache
- **THEN** R evaluates no origin height and no mesh key, adds the height volumes of the tiles outside the frustum, and a repeat of R or R for a turned view at the same position evaluates the field zero times and equals the first R

#### Scenario: Thread count and reuse
- **WHEN** the worker makes R for three requests (a position, a turn at that position, another position) on 1, 2 and 8 threads
- **THEN** every R equals the pure function of its position, the turn gets the very same set, and only the other position selects a new one

### Requirement: STRM-016 Frame pipelining
A frame loop SHALL never wait for the plan of its camera and never wait for the GPU of the frame it has just submitted. Plans SHALL be computed on a worker, one at a time, a request that arrives while one runs replacing any request that waits; a plan SHALL be a pure function of its request (camera, LOD parameters, frame setup, field), so the plan made for a camera is the same for any thread count, spawner and timing, and equals the synchronous plan of that camera; the origin heights of the nodes a plan cache has not seen SHALL be evaluated on worker threads, once per tile. The frames in flight SHALL be counted and SHALL NEVER exceed the bound `MAX_FRAMES_IN_FLIGHT` (a configuration constant, 2): at the bound the loop waits for the oldest frame, the only wait it knows, and counts it; a slot the renderer releases SHALL be reusable only that many frames later. Frame N SHALL draw the newest completed plan if it is usable for frame N's camera (same frame size, view, grid and lens, the camera moved at most a quarter of the split distance of the plan's finest level and turned at most 0.5 degrees), rebased to that camera and covers the frame's view, otherwise the cover. A plan made for another camera covers the frame's view when, for every tile of the frame's own cover, it draws that tile or finer tiles filling every child of it that is in the frame's view (recursively), and none of its nodes is blank; a coarser ancestor does not count (the cover can show more detail), and a tile it does not draw at all is a sliver that turned into view (`plan_covers_view`, a pure function of the camera, the cover, the drawn set and the cached tile bounds); a plan made for exactly the frame's request needs no test. Every drawn node whose tile (with its mesh key) is not resident SHALL be drawn by its deepest resident ancestor as a stand-in over the quadrant of the ancestor that holds the node, and a resident tile under a quadrant that a stand-in draws is not drawn (no part is drawn twice) while the resident tiles beside it are, so a tile that has not streamed in does not take its resident siblings back to their parent; a node with no resident ancestor SHALL be counted blank (zero while the roots, the tiles at the render floor of LOD-011, are resident; a floor tile that has not arrived is blank, there is no coarser stand-in). A node SHALL carry the quadrants of its tile that it draws, as a mask of four bits (bit k for child k of the tile's children, `0b1111` the whole tile, the default); the fragment stage SHALL discard the fragments of a terrain or water node whose tile coordinates lie in a quadrant the mask leaves out, after the derivatives of the fragment, and a node drawn whole SHALL give the same frame as before the mask existed, drawn by pipelines whose fragment stage holds no `discard` for the mask (the nodes drawn over some quadrants go in their own draws, after the whole ones, through pipelines made with the cut on); the text form of a plan SHALL carry the mask only when it is not the whole tile. The cover SHALL be the plan's own selection with "resident" as the condition for descending, a pure function of the camera, the LOD parameters, the resident set and the cached bounds of the tiles of the newest R (each tile's geometry and height-bounds volume as the plan computed them; the frame path evaluates no height field): from the resident roots (floor tiles), a tile is split while the plan's LOD test says so (the distance to its volume below the level's split distance) and at least one child in view is resident, otherwise the tile is drawn whole; the resident children in view go on down the walk and the tile stands in over the quadrants of the children in view that are not resident; in view means what the plan means (not beyond the horizon, not outside the frustum, not outside the tight cut of LOD-007), and a child that the horizon, the frustum or the cut culls never stops its parent's descent, nor does a tile that is not in R when the bounds are those of R for exactly the camera's position and parameters (the plan never selects it); the leaves, the missing children among them, are then balanced as the plan's are (2:1), and a leaf that is not resident is drawn by its nearest resident ancestor over the quadrant that holds it; the tiles of the cover are the stand-in parent and its resident children side by side, and `plan_covers_view` counts a tile drawn over some quadrants as its drawn children. Known limits, until the missing tile has streamed in: a hairline crack can open along the cut between a stand-in's quadrant and a finer resident sibling (the stand-in has no skirt there), and the level difference across the cut is not held to 2:1 where the missing child lay beside finer tiles. With every tile of R resident the cover of a camera SHALL equal the plan of that camera, node for node and level for level, whatever the camera's direction, every tile drawn whole; with tiles missing it is the deepest resident tiles and never blank, and always a subset of the resident tiles. A plan made for exactly the frame's request with every node resident SHALL be drawn unchanged, so once plans have settled the frame equals the synchronous frame. The wants of a frame loop SHALL be the nodes of the newest delivered plan (the draw set) and the newest delivered R, handed to the residency manager (STRM-017); a loop that has not yet received R asks for the draw set alone.

The cover SHALL apply the ceiling of the residency set's bounds (LOD-008, ADR 0074) as the plan does: a tile whose children the ceiling forbids is a leaf (it is drawn when it is in view), and the leaves are clamped to the ceiling after the balance, so that with every tile of R resident the cover is the plan node for node on a field with tiles too steep to morph.

Verify: A, C
Status: active
Source: ADR 0032 point 5, ADR 0031 point 3, optimization review item A, STRM-002, LOD-007, CON-03, CON-14, CON-22, LOD-011, ADR 0062, SPIKE-18 (2026-10-20)

#### Scenario: Same plans as the synchronous loop
- **WHEN** a fixed camera script runs with a worker that finishes one plan between frames
- **THEN** every plan delivered equals the synchronous plan of the camera it was requested for, in request order, and with the camera still the frame is the synchronous plan

#### Scenario: Thread count
- **WHEN** the same cameras are planned on 1, 2 and 8 worker threads
- **THEN** the plan hashes are equal, and equal to the synchronous plans

#### Scenario: Latest request wins
- **WHEN** four requests arrive while the first plan runs
- **THEN** the first and the last are planned, the two between are counted as superseded

#### Scenario: Frames in flight
- **WHEN** a loop submits frames to a GPU that finishes a frame every third submission
- **THEN** frames in flight never exceed the bound, the peak reaches it, the waits are counted, and a GPU that keeps up never makes the loop wait

#### Scenario: Stand-ins
- **WHEN** some tiles of a plan are not resident
- **THEN** each such node is drawn by its deepest resident ancestor over the quadrant that holds it, no part is drawn twice (a tile lies under another only beside the quadrants the other draws), the tiles are in ascending id order, and origins and matrix follow the frame's camera

#### Scenario: Cover
- **WHEN** only the roots are resident, or part of a plan's tiles with their ancestors
- **THEN** the cover holds resident tiles only, none under a quadrant another draws, serves every node of the full plan from above or below, and is the same for the same inputs

#### Scenario: A missing tile keeps its siblings
- **WHEN** the cover is made with R resident except one leaf in view and what is under it
- **THEN** its resident siblings are drawn at their level or finer, their parent is drawn over the missing leaf's quadrant only, and with R complete every tile is drawn whole

#### Scenario: The quadrants on the screen
- **WHEN** a node is drawn over one quadrant, over each of the four and over two
- **THEN** the pixels of the four single quadrants partition those of the whole node, each covers the centre of its own quadrant (child k = 2 times the x half plus the y half) and no other, and two quadrants cover the union of theirs

#### Scenario: Jump
- **WHEN** the camera jumps with the plan of the new place still on the worker
- **THEN** frame 0 draws a cover without evaluating the field, the worker's plan arrives later and is drawn with stand-ins, and once its tiles are resident the frame is the synchronous plan

#### Scenario: Stale plan
- **WHEN** the camera moves by 0.9 or 1.1 times the limit, or turns by 0.4 or 0.6 degrees, or the frame size or view differs
- **THEN** the plan is usable below the limit and unusable above it, and for a different size or view

#### Scenario: Origin heights on workers
- **WHEN** a cold plan is made with four threads
- **THEN** its origin heights are evaluated on more than one thread, once per node, a warm plan evaluates none, and the plan equals the single-threaded one

#### Scenario: Pipelined image
- **WHEN** the terrain scene runs through the pipelined loop from a cold start, and after a jump to another view
- **THEN** the first frame is a cover and the image the loop settles on equals the synchronous path's

#### Scenario: The cover is the plan with R resident
- **WHEN** every tile of R of a position is resident and the cover is made for the ten standard views and for a rotation script (0.2 to 15 degrees a frame, pitch changes) at that position
- **THEN** the cover equals the plan of the same camera, node for node and level for level

#### Scenario: Partial residency
- **WHEN** a third of R's leaves (with their ancestors) is resident, with or without cached bounds
- **THEN** the cover holds resident tiles only, no tile under a quadrant another draws, and every node of the plan has a cover tile at it, above it or below it

#### Scenario: A plan that does not cover the frame
- **WHEN** a frame's camera is turned away from the plan's camera so that the plan does not cover its view, and again by small steps
- **THEN** the frame draws the cover (the plan of its own camera) where the plan does not cover the view and the plan where it does, exactly as `plan_covers_view` says, and no node of the frame's own plan is drawn coarser than the plan draws it or missing

#### Scenario: Cold start at the floor
- **WHEN** the orbit view is drawn from a cold start without a pre-fill, and then with the pre-fill of STRM-021
- **THEN** the start-up load of STRM-001 makes all 384 floor tiles resident before frame 0 (from the pack with the pre-fill, generated without it, a visible cold-start cost that is reported), so no frame is blank in either case

### Requirement: STRM-017 Managed frame loop
The frame loop SHALL be driven by the residency manager. Each iteration SHALL (1) hand the manager the wants (STRM-016): the draw set D of the newest delivered plan and the residency set R of the newest delivered worker result, and the prefetch set of the newest delivered worker result capped by the free share (STRM-018; empty when the loop has no prefetch source or the camera is not travelling); (2) execute the changes of the resident set on the renderer: a tile that became resident is written to the slot the manager assigned (`upload_at`) within the per-frame upload budget (STRM-006: the finished tiles wait in class order and the rest stay finished-but-not-placed for a later frame), an evicted tile is forgotten, and the renderer decides nothing about placement; (3) draw from the tiles the renderer holds. The manager's capacity SHALL be the renderer's pool, in slots (STRM-006), so that what the manager places the renderer can hold. The loop SHALL report, for every frame it draws, the counts of its iteration after the time columns (TEST-012): `resident`, `in_flight`, `queued`, `dispatched`, `cancelled`, `blocked`, `evicted`, `uploaded`, `degraded`, `upload_bytes` (tile bytes written), `deferred` (finished tiles waiting for the budget) and `node_bytes` (the frame's node-record bytes, STRM-006). The prefetch counts of a frame (wants after the cap, prefetch requests cancelled) are carried in the frame's counts but are not log columns. The one-shot streamed path (`--stream`) SHALL use the same manager with D from the plan and R of the camera position selected on the plan's cache, and SHALL draw the same meshes as the synchronous path; it has one camera and no travel, so its prefetch set is empty (STRM-018). No golden, plan count or mesh version changes with the wiring: once the plans have settled, the image of the managed loop equals the synchronous frame.

Verify: A, C
Status: active
Source: ADR 0031 points 2, 3, 4 and 6, ADR 0032 point 5, ADR 0033 points 1-3 and 9, STRM-005, STRM-006, STRM-007, STRM-015, STRM-016, CON-03, CON-04

#### Scenario: Wants from the worker
- **WHEN** the plan and R of a position are delivered
- **THEN** the next iteration's wants are exactly that plan's nodes in plan order and that R's tiles with their distances, and a rotation that delivers only a new plan leaves the residency wants unchanged

#### Scenario: Per-frame counts
- **WHEN** the pipelined terrain scene runs on the software adapter with `--out-dir`
- **THEN** its per-frame log has the time columns followed by the twelve count columns (the nine of the scheduler, then `upload_bytes deferred node_bytes`), the last frame shows resident tiles in slots and no blocked or degraded tile, and the run summary reports the slots resident of the pool

#### Scenario: Streamed equals synchronous
- **WHEN** the terrain scene runs with `--stream`
- **THEN** the image equals the synchronous scene's image bit for bit

#### Scenario: Pipelined equals synchronous
- **WHEN** the terrain scene runs through the managed loop from a cold start and after a jump
- **THEN** the loop settles on the synchronous frame of the last view

### Requirement: STRM-018 Prefetch along travel
The prefetch set (STRM-005 class 3) SHALL be computed from the camera's recent translation only, on the injected clock: the position history of the last `window_ms` gives a velocity, the camera is predicted `look_ahead_ms` ahead along it, and the set is R of the predicted position minus R of the current position, ordered coarse before fine and nearer to the predicted position first. The prediction SHALL be a pure function of the sequence of `(time, position)` samples: no view direction, yaw or pitch rate enters it, so a rotation at a fixed position (zero velocity exactly) and a stationary camera produce an empty set and no request. The predicted displacement SHALL be capped (`max_distance_m`), a descent SHALL NOT be extrapolated below `min_altitude_fraction` of the current altitude, and a speed below `min_speed_mps` SHALL predict nothing. A step between consecutive samples larger than `teleport_base_m + teleport_altitude_fraction * altitude` SHALL be a teleport: the history is cleared and nothing is extrapolated from the old direction. The set SHALL be computed on the plan worker after R, on the plan's cache, never on the frame, and SHALL be a pure function of the two positions (the same for any thread count, CON-14). The managed loop SHALL hand the manager only as many missing prefetch tiles as there are free slots in the main pool (tiles already resident cost nothing), so a full share makes no prefetch request; the free count is that of the end of the previous iteration, so the bound that holds at every instant is the streamer's own (prefetch dispatches only into a free slot and the manager never evicts R or D for it, STRM-005). When the prediction changes the manager cancels the prefetch requests that left the set (`cancelled_prefetch`). The constants (`PrefetchConfig`: look-ahead 2000 ms, window 400 ms, minimum speed 0.5 m/s, distance cap 50 km, minimum altitude fraction 0.5, teleport step 500 m plus 10 % of the altitude) are conservative starting values; ADR 0031's owner question 2 (how far ahead) SHALL be decided from the first dive and teleport measurements by changing these constants only.

Verify: A
Status: active
Source: ADR 0031 points 2.3, 3 and 5, ADR 0032, STRM-005, STRM-015, STRM-017, CON-03, CON-14

#### Scenario: Rotation prefetches nothing
- **WHEN** a camera with its prefetch source on turns 360 degrees at a fixed position, R resident
- **THEN** the velocity is exactly zero, the prefetch set is empty and no request is made or cancelled

#### Scenario: Travel prefetches ahead
- **WHEN** the camera travels in a straight line at 600 m/s over a fixed script with the same machine speed, with and without the prefetch source
- **THEN** with it at most half as many draw-set tiles are first seen missing as without, and more are already resident when they first appear

#### Scenario: Free share only
- **WHEN** the share is too small for R and the camera travels
- **THEN** in every frame the missing prefetch tiles wanted number at most the free slots, none while no slot is free, and no frame is over the share or blank

#### Scenario: Direction change and teleport
- **WHEN** the camera turns round, or jumps farther than the teleport step
- **THEN** after turning round the queued prefetch requests that left the set are cancelled; after a jump the velocity is reset, nothing is predicted and the prefetch wants are empty within three frames

#### Scenario: Off the critical path and deterministic
- **WHEN** a frame is begun with a predicted position
- **THEN** it evaluates no height field; the set arrives with R from the worker and equals R-ahead minus R-here, identical on 1, 2 and 8 threads

### Requirement: STRM-019 Scheduler work follows changes
The frame thread's scheduler work in a frame (collecting completions, applying the wants, queueing requests, making room, dispatching) SHALL be proportional to what changed since the previous frame (the tiles added to or removed from a request class, the completions, the requests that start, the evictions they need), not to the size of the wants. A frame in which nothing changed SHALL touch no element (the scheduler counts the elements it reads in sequential passes and the elements it writes, `SchedWork`, CON-20); a frame in which the draw set turns (the plan changed, R did not) SHALL write no tile of the residency or prefetch class and read only the draw set; a frame in which c tiles join or leave R SHALL write a number of elements proportional to c (times the levels climbed), independent of how many tiles R holds, plus one sequential pass over the lists that differ; a priority change alone SHALL write no tile of a class and re-order no request that nothing reads. The requests, their order, the cancellations, the evictions and the answers of every frame SHALL be those of a scheduler that recomputes everything from the wants each frame (STRM-005, STRM-007, CON-14): the order does not depend on how the wants were handed over (all lists every frame, or only those that changed), the spawner or the thread count. The residency manager SHALL fit the wants to the pool (STRM-006, STRM-007) only when R with the closure of D could exceed it, and when the pool holds them SHALL report the numbers of the pure fit without running it. The frame thread's scheduler time (`scheduler_ms`, TEST-012) is reported per frame; its target is a p99 of at most 2 ms in every streaming scene, tracked in the perf window (a time budget is measured on the reference machine only, CON-20).

Verify: A
Status: active
Source: perf window 1 (2026-10-16), M4 fix F2, STRM-005, STRM-007, STRM-017, CON-14, CON-20, ADR 0031

#### Scenario: Settled frame
- **WHEN** every tile of R is resident, nothing is queued or running and the wants do not change, for R of N and of 4N tiles
- **THEN** a scheduler frame reads and writes no element, through the streamer and through the managed loop (still camera, 30 frames)

#### Scenario: Turn
- **WHEN** the camera turns at a fixed position with R resident (the plan changes every frame, R does not)
- **THEN** no tile of the residency or prefetch class is written in any frame, the reads are those of the draw set, and nothing is requested or cancelled

#### Scenario: Changes proportional to c
- **WHEN** c = 1, 10 and 100 finest tiles are removed from R, or c finer tiles are added below its leaves, with R of N and of 4N tiles
- **THEN** the elements written are at most 4c (removal) or 8c (addition, including the requests that start) and are the same for N and 4N, and the elements read are those of one pass over R and the c tiles

#### Scenario: Same decisions as the full recomputation
- **WHEN** the scripted runs (streamer alone with pools that hold R, that are smaller than R, with failures; the managed loop with prefetch, an upload budget and pools smaller than R) are replayed
- **THEN** the full per-frame record (answers, counts, dispatch order, resident changes, written tiles) equals the digests taken from the scheduler that recomputed everything every frame

### Requirement: STRM-020 Dispatch follows the generation pool
The number of tile jobs the scheduler lets run at once (`StreamConfig::max_in_flight`) SHALL equal the number of threads of the tile generation pool, derived from it (`GenWorkers`) and not stated separately; the default is the machine's hardware threads less a reserve of `GEN_RESERVE` = 2 cores kept for the frame thread, the plan worker and the driver's threads (perf window 2: with a worker on every core a frame's upload and scheduler steps wait for the OS to schedule them), never below one worker and never above the hardware threads (`GenWorkers::workers_for(cores, reserve)`, a pure function; a named default, not a measured optimum; 18 workers on the 20 threads of the reference machine), a parameter of the pipelined loop and the streaming perf scenes (`--gen-workers N` fixes the size outright and the reserve then decides nothing, `--gen-reserve N` changes the reserve, 0 giving a worker per hardware thread), and the plan worker's threads are not part of it. The reserve and the size change when jobs run, not what is requested or in which order, so no result depends on them (CON-14). While requests wait, every frame SHALL refill every free place so that the jobs in flight equal the limit, in the order of the request queue (STRM-001, STRM-005), and a worker that finishes between two frames SHALL NOT idle until the next frame: the scheduler hands the spawner `queue_ahead` jobs beyond the running ones (`--gen-ahead N`, default the worker count in the application, 0 in `StreamConfig::default()`), dispatched in queue order and cancelled like any other job (a queued job stops at its start). The jobs handed over at once are `max_in_flight + queue_ahead`; the order of requests SHALL be a function of the wants, the resident set, the failure times and that limit, not of the spawner or the thread count (CON-14), and the cancellation, teardown and failure semantics of STRM-005 are unchanged. With a held demand larger than the pool can finish, the tiles generated per second SHALL be at least 80% of workers divided by the job duration for 4, 8 and 20 workers, where refilling at frame boundaries alone reaches about half of it for jobs just longer than a frame. The generation workers SHALL run at below-normal OS priority by default (`--gen-priority below-normal|normal`; ADR 0046): each worker thread sets its own priority before its first job (`ThreadPool::with_priority`, carried by `GenWorkers::priority`; Windows `SetThreadPriority(THREAD_PRIORITY_BELOW_NORMAL)`, Linux `setpriority` with nice +5 on the thread; a platform without the call, or a refusal, leaves the worker at normal priority and the pool working), so that the OS gives the frame thread its core back when both want it, which a reserve of cores alone does not give it once other processes or the driver's threads take the reserved cores (the ablation of docs/learnings.md, "Generation workers at below-normal priority"). A priority changes when a thread runs, never what it computes: the size, the job limit, the request order and every result are the same at either priority (CON-14), the pool's workers read back the priority they were built with (`priority::current`), and the calling thread keeps its own. The session log records it in `protocol.gen`. The default limit depends on the machine, so a run that must be reproducible (CI, goldens, a perf comparison) pins `--gen-workers` and `--gen-ahead`.

Verify: A
Status: active
Source: perf window 2 (2026-10-16), M4 fix, perf window 3 and F10 (ADR 0046), STRM-001, STRM-005, TEST-018, ADR 0031, ADR 0032, CON-04, CON-14

#### Scenario: In flight equals the job limit, every worker busy
- **WHEN** a demand larger than the pool is held on N = 4, 8 and 20 model workers
- **THEN** from the first frame the jobs in flight equal N plus the queue ahead and N workers are busy in every frame in which requests wait, and from the second frame each frame dispatches exactly the jobs that completed since the last

#### Scenario: Throughput follows the generation model
- **WHEN** the same demand, and the dive script, run on workers of 18 ms a job with frames of 1/60 s
- **THEN** the tiles per second are at least 80% of N / 18 ms (N = 4, 8, 20 for the held demand; 8 and 20 for the dive), and with no queue ahead (the previous refill) fewer than 70% of it

#### Scenario: Same order for the same limit
- **WHEN** wants that move are run with limits 8 + 0, 4 + 4 and 2 + 6
- **THEN** the dispatch order is identical

#### Scenario: Teardown with a full pool
- **WHEN** the streamer is dropped with N running and as many queued jobs
- **THEN** no job loads a tile afterwards and none is left waiting

#### Scenario: Default pool size
- **WHEN** the pool size is derived for 1 to 64 hardware threads and reserves of 0 to 70
- **THEN** it is the threads less the reserve, one when the reserve covers them all, never above the threads; the default reserve is 2 (18 workers for 20 threads, 2 for 4), the machine's default equals that rule on its threads with as many jobs queued as workers, and `--gen-workers` overrides it

#### Scenario: Workers read back their priority
- **WHEN** a pool is built with below-normal priority, and one with the default, and each runs jobs that read the priority of their thread
- **THEN** on Windows and Linux every job of the first reads below normal and every job of the second reads normal, the thread that built the pool is unchanged, and `GenWorkers` of equal size give equal job limits at either priority

#### Scenario: Option
- **WHEN** the options are parsed without `--gen-priority`, with `below-normal`, with `normal` and with another value
- **THEN** the pool's priority is below normal, below normal, normal, and the last is an error naming both values

### Requirement: STRM-021 Pre-filled coarse tiles
The bake step SHALL also write the finished render tiles of levels `F..=N` (F the render floor, LOD-011, default 3, `--floor-level N` on `generate --prefill-dir` too; default `N = 4`, 1,920 tiles; `--prefill-levels N` names N, 0 disables, an N below F writes nothing and says so; F is the effective floor min(floor, max_level) of the run, as in LOD-011; at most 8) into the tile pack of the bake, as records of kind 2 (`Kind::Mesh`) with the existing record format and the ADR 0018 mesh key of the tile (`mesh_key_inputs`: field, tile, cells, skirt fraction, `MESH_VERSION` and the climate version word) plus one output parameter naming the form of the stored tile (the compact form, 1; a tile below the render floor is never stored or looked up, and a floor below 3 is refused, because levels 0-2 have no compact form), so that no existing key, record format or version changes. The payload SHALL be the tile exactly as the generators build it, every `f32`, `u16` and `f64` bit (`codec version | form | cells | raw length | LZ4 block`), and SHALL decode to a tile equal to the generated one (payload version 3 since MESH_VERSION 8: the compact form is the slot of GEN-025; a payload of the former vertex form (form byte 0) is refused naming it; a version-1 or version-2 payload is refused naming the version, and records written under MESH_VERSION 7 are unreachable because `MESH_VERSION` is part of the key, so a pack pre-filled under version 7 yields only misses, `missing_tiles` lists every tile and the pre-fill writes the tiles again; the stale records stay in the pack until compaction, ADR 0029); a payload that is short, of another version or form, of cells that are not a power of two from 2 to 1024, of a wrong raw length or damaged SHALL be refused naming what is wrong, and the decoder SHALL NOT allocate more than the cells allow. A pre-filled tile SHALL equal the tile the streamer generates on demand for the same field, cells and finest level, bit for bit, whatever the number of threads that wrote it (CON-14): tiles are generated on any number of threads, coarsest first, and appended in the fixed order level, face, row, column, so the bytes of the pack do not depend on the thread count. A tile at or below the run's finest level SHALL NOT be stored or looked up (until ADR 0062 its key was the finest-level class of GEN-010; nothing was ever stored under it, so the removal of the class cannot make a record answer for a tile whose key changed), and a pre-fill that reaches it, or goes deeper than 8, SHALL be refused before it writes. The pre-fill SHALL be idempotent: a tile the pack already holds under the current key whose payload decodes is skipped, a run that finds none missing opens no writer and changes no byte or file time, a deeper run adds only the new levels, and a record that does not decode counts as missing and is written again (the newer record wins). `generate --global-dir D` SHALL pre-fill after the bake and print the tiles, payload bytes, growth of the packs and wall time; `generate --prefill-dir D --global-res N [--prefill-levels L] [--threads T] [--cells C] [--finest-level F] [--tile-format F]` SHALL pre-fill an existing bake in place and refuse a store that holds no bake of that definition and size. The streamer's tile source (`MeshSource`) SHALL take a tile of level 8 or coarser from the pack when the pack holds it under the current key and generate it otherwise (a cache degrades to a miss: a damaged record, another version, another field, other cells or the other form is a generation, never an error), and a world opened `cold` SHALL never read a tile from the pack (the cold reader of `dive-cold`, TEST-019). Pre-filled tiles are caches evicted with their bake (ADR 0045 point 1) and are not read by `verify-cache` or `verify-global`, which report them as records of another kind.

Verify: A
Status: active
Source: owner (2026-10-16, option A), ADR 0045 point 3, ADR 0018, ADR 0019, STRM-008, STRM-003, CON-05, CON-13, CON-14, M4 fix queue (F9), LOD-011, GEN-025, ADR 0062

#### Scenario: Codec
- **WHEN** compact tiles (level 4, cells 16 and 32) are encoded and decoded, a payload of the former vertex form is decoded, and hostile payloads are decoded
- **THEN** every bit of the tile comes back, and a short payload, another version or form, bad or huge cells, a wrong raw length and a damaged block are each refused naming the cause without allocating by the claim

#### Scenario: Keys
- **WHEN** the key of a tile is made for another field, other cells or another tile
- **THEN** it differs

#### Scenario: Idempotent and thread-independent
- **WHEN** levels 3-4 are pre-filled on one thread and on five, the pre-fill is run again, and a deeper one is run
- **THEN** the pack files of the two runs are byte-identical, every stored tile equals the generated one, the second run generates and writes nothing and touches no file, and the deeper run adds only the new levels

#### Scenario: Limits and misses
- **WHEN** a pre-fill reaches the run's finest level or goes deeper than 8, and a store pre-filled for one field is read for another seed, other cells or a finest tile
- **THEN** the first is refused before writing anything and the others are misses

#### Scenario: Damaged record
- **WHEN** a record under a tile's key holds a payload that does not decode
- **THEN** the tile counts as missing, a repeated pre-fill writes it again and the newer record is the one read

#### Scenario: The streamer reads the pack
- **WHEN** a bake of an eroded-field world is pre-filled to level 4 through `generate`, and the streamer's tile source loads tiles of levels 3-4 and a level-5 tile
- **THEN** the level 3-4 tiles are taken from the pack and equal the generated ones (also through the staged, cancellable path), the level-5 tile is generated, a cold reader of the same store reads none from the pack and builds the same tiles

#### Scenario: The commands
- **WHEN** `generate --global-dir` bakes with `--prefill-levels 3`, `--prefill-dir` pre-fills an existing bake to level 3, then 4, then 4 again, and the options are misused
- **THEN** the reports count 384 then 1,920 tiles with 384, 1,536 and 0 written, the last run grows the packs by 0 bytes, `--prefill-levels 0` is disabled, and a missing `--global-res`, a store of another size, a depth over 8, a finest level inside the pre-fill and a clash with another command are each refused naming the cause

#### Scenario: A pack pre-filled under an older mesh version
- **WHEN** a pack holds records written under MESH_VERSION 7 keys with version-2 payloads and the tiles of levels 3 to 4 are looked up and pre-filled
- **THEN** every lookup is a miss, a version-2 payload is refused even under the current key, the pre-fill writes every tile again and the new records decode to the generated tiles

#### Scenario: Floor and levels
- **WHEN** a pre-fill with floor 3 and N = 4 runs, and one with floor 0 is requested
- **THEN** the pack holds 1,920 tiles, none of levels 0-2, every stored tile equals the generated one, and the floor 0 pre-fill is refused before it writes (levels 0-2 have no compact form)

#### Scenario: The floor on the command line
- **WHEN** `generate --prefill-dir D --global-res N --floor-level 0 --prefill-levels 1` and the same with `--floor-level 3` run, and `--floor-level 5` with `--finest-level 4` is given
- **THEN** the first is refused naming the floor below level 3 and the compact format, the second (N below F) writes nothing and says so, and the third is refused naming 5 and 4 (LOD-011)

### Requirement: STRM-022 Tile load counters
The streamer, the block source, the tile pack and the streaming perf scenes SHALL record what a tile's load costs and where it goes, as readings that no decision reads (the request order, the dispatch, the cache contents and every result stay a function of the wants, the resident set, the failure times and the limits, CON-14): (1) the streamer SHALL log every tile job that ran its load (`Streamer::job_log`, `JobSample`): the microseconds from dispatch to start, the microseconds of the load, the number of loads inside their source when it started (itself included), the tile's level, and whether it produced a tile; a job cancelled before it starts is not logged; (2) the stored-bake reader (`StoreSource::stats`) SHALL count its shared-cache lookups and hits, the blocks pushed out of the cache for the budget, the lookups that found a block another thread was still decoding, the decodes, and the nanoseconds all threads together spent on the cache lock, fetching a block's record and decoding it, with `shared_lookups - shared_hits = decodes` when no read fails and `evictions = decodes - budget` once the budget is exceeded, and SHALL expose per thread the nanoseconds spent getting blocks and the blocks it decoded (`thread_block_wait_ns`, `thread_block_decodes`); a read inside the block the thread read last SHALL touch no shared counter or lock; (3) the tile pack SHALL count its lookups, those that found a record and the nanoseconds they took (`TileStore::read_stats`); (4) the world's terrain SHALL count the pre-filled tile lookups that reached the pack, those that found the tile, the lookups skipped for a tile too deep to be stored, their nanoseconds, the fields built, and the jobs' own block wait and decodes (`WorldTerrain::counters`); (5) each streaming perf case SHALL write into its log, for the measured frames, the `load_` counts of those readings (`load_jobs`, the median, p99, maximum and sum of the load time and the median, p99 and sum of the queue wait, the median load time by concurrency band and by level band, the block cache's decodes, lookups, hits, evictions, waits, lock, fetch and decode time, residency and budget, the decodes and lookups per 1000 tiles, the pack's and the pre-filled lookups' counts and time, the jobs' block wait and decodes, the fields built, and the start-up load of the roots before the first frame: `load_startup_roots`, their number, and `load_startup_roots_ms`, the milliseconds it waited for them, a reading of the cold start) and, in a run that is not a smoke run, a CPU calibration before and after the case (`load_calib_before_*`, `load_calib_after_*`: a fixed ALU kernel's time alone, and its median and maximum time on as many threads as the generation pool has at once, the same value computed on every call); (6) the regression comparison SHALL record these counts and SHALL NOT report a change of any count named with the prefix `load_`, nor judge any budget by one.

Verify: A, C
Status: active
Source: perf window 3 (2026-10-16), M4 fix F11, STRM-014, STRM-020, STRM-021, TEST-012, TEST-018, CON-14, CON-20, LOD-011

#### Scenario: The job log counts jobs, levels, outcomes and concurrency
- **WHEN** jobs run on a manual spawner, with a source that fails one level, with a job cancelled before it starts, and on a pool of four workers whose loads meet at a barrier
- **THEN** one sample is logged per job that ran (the dispatched count), the levels are those of the dispatched tiles, a failed load is not `done`, a cancelled job is not logged, loads one at a time have concurrency 1 and four loads inside the source together give a sample with concurrency 4 and none above

#### Scenario: The block source counts its cache exactly
- **WHEN** one thread reads a grid through a cache of five blocks, and eight threads read it through a cache that holds every block
- **THEN** the lookups are the hits plus the decodes, the evictions are the decodes beyond five, nothing waited for another thread, the thread's decodes equal the source's, and with the roomy cache every block is decoded once with no eviction and no more waits than hits, every read returning the stored bits

#### Scenario: The pack counts its lookups
- **WHEN** blocks are decoded through a store and a key it does not hold is asked for
- **THEN** the store counts one lookup that found a record per decode, and the missing key one lookup that found none

#### Scenario: The pre-filled lookups are counted
- **WHEN** pre-filled tiles of levels 3-4 are loaded through the streamer's source, then a level-5 tile and a level-9 tile
- **THEN** each tile loaded on the pack's world is a lookup and a find, the cold reader counts none, the level-5 tile is a lookup without a find, the level-9 tile is skipped without a lookup, and the pack saw at least those lookups

#### Scenario: The load counts are a pure summary
- **WHEN** fixed jobs and counters are summarised
- **THEN** the medians, tails, sums, bands and per-1000 figures are those of the inputs, every name has the prefix `load_` and none repeats, no jobs give zeros, the calibration kernel gives the same value for the same work and reports its thread count, and a comparison of two logs that differ only in `load_` counts reports no changed count

#### Scenario: A smoke log carries the counts
- **WHEN** the streaming scenes run in smoke mode on the software adapter
- **THEN** every log has the job, block, pack, pre-fill and field counts, the pack's lookups are at least the block decodes, and no calibration

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

### Requirement: STRM-024 Helper threads of a tile job keep the job's priority
Threads a tile job starts (the parallel block reads of `WaterBook::fill`) SHALL set the priority the job runs at before they read; a new thread starts at normal priority and would otherwise take cores from the frame thread that the below-normal workers leave it (STRM-020). A platform without priorities leaves them at normal.

Verify: A
Status: active
Source: perf window 7 dive frames 65-112 (spikes of 53-61 ms with 36 jobs in flight), STRM-020

#### Scenario: A fill in a below-normal job
- **WHEN** a thread at below-normal priority fills the water of a tile
- **THEN** every field query of the fill runs on a thread at below-normal priority

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

### Requirement: STRM-026 Refresh of resident tiles
When an item edit changes what some tiles read (the footprint find of the edit, WDEF-007), the streamer SHALL request each resident tile of that set again without dropping it (`Streamer::refresh`): the tile stays resident and served from its slot until the new version arrives, the new version takes a second slot, replaces the old one when its job completes (the loop is told with a new `Added` change) and gives the old slot back. The request SHALL be of the tile's class (draw, residency or prefetch) and, in the residency and prefetch classes, ahead of the other requests of the class (rank 0 instead of the level: an exception to the order of STRM-005, "coarse before fine", for edited tiles only), so edited tiles are not queued behind the rest of R; a draw request has no rank to go ahead of. A resident tile in no class SHALL be evicted instead of refreshed. A tile whose job is running SHALL be told to stop and its result SHALL be dropped. A resident tile that has not been replaced yet SHALL stay marked stale and SHALL be requested again whenever nothing is running for it and something wants it (when its job ended without replacing it, after the retry delay of a failed job, or when it is wanted again after it lost every class), so no tile ever holds a version made from the field before the edit and a failed refresh is never forgotten. A refresh request that is still queued when its tile leaves every class SHALL be dropped and counted as cancelled (the class it was queued under no longer lists the tile); the tile stays resident and stale. While a tile is refreshed it holds two slots (an exception to STRM-006 and STRM-007 that lasts until the new version replaces it, the old slot then returning to the pool); a refresh that finds no free slot waits, and the old version is served meanwhile. The frame loop's edit event (`ManagedLoop::edit`) SHALL forget the plan cache entries (origin height, mesh key, volume) of the found tiles, refresh them and ask for a new plan at the next frame although the camera did not move.

Verify: A
Status: active
Source: SPIKE-13 variant D, ADR 0066, ADR 0021 point 2, M6 plan row E1, window 13 (2026-10-19)

#### Scenario: The old version is served
- **WHEN** a resident tile is refreshed and the job has not finished
- **THEN** the tile is resident in its old slot with its old payload, a second slot is in use, and when the job completes the payload is the new one, in the second slot, and the old slot is back after the frames in flight retire

#### Scenario: Nothing to refresh
- **WHEN** a tile that is resident but in no class, or was never loaded, is named by a refresh, also while a job of the resident one runs
- **THEN** the first is evicted (its job dropped) and the second is left alone, and no request is queued

#### Scenario: A running job
- **WHEN** a refresh names a tile whose job is running, once for a first load and once for a refresh job
- **THEN** the job is told to stop, its result is dropped, and the tile ends with the version of the last field, served all the time if it was resident

#### Scenario: Ahead of the rest of the class
- **WHEN** a refresh of a fine tile and a request of a coarser tile of the residency class are both waiting
- **THEN** the refresh starts first

#### Scenario: A failed refresh
- **WHEN** the job of a refresh fails
- **THEN** the old version is served meanwhile and the refresh is requested again after the retry delay, until the new version replaces the old

#### Scenario: Quiet
- **WHEN** nothing is stale and the wants did not change
- **THEN** a frame scans and updates nothing, also after a refresh has completed, and a refresh counts one unit of work per request it queues (STRM-019)

#### Scenario: The edit event
- **WHEN** an edit event names the resident tiles it changed while the camera stands still
- **THEN** a new plan is delivered although the request did not change, the old tiles are drawn until their refreshes finish, and the frame is the synchronous frame of the new field again afterwards, with every requested refresh counted as a replacement

#### Scenario: The tile leaves every class
- **WHEN** a refresh is queued for a resident tile of a residency set made from a selection (closed under its ancestors) and the wants then change so that the tile is in no class before the request starts
- **THEN** no job starts, the request is dropped and counted as cancelled, the tile stays resident with its old payload, and when it is wanted again the new version replaces it

### Requirement: STRM-028 The terrain reads the current definition
A world's terrain SHALL hold its definition as a replaceable shared value and SHALL build every field from a snapshot of it taken when the field is built; a field built before a replacement SHALL keep the definition it was built from (a plan or tile job in flight finishes on the definition it started with, and its keys name it through the item revision and the footprint entries). A replacement SHALL be refused, naming both hashes, when the new definition's base hash differs from the current one's, because the bake the fields read was made from the base. The field of a definition that holds stamps SHALL be the base field with one stamp stage (GEN-026) per layer that holds a stamp, lowest layer first (the layer stack of ADR 0007, then other layer names by name), each at its place in the stack (layers outside it follow the stack's seven, in name order), below the water bound; the field of a definition without stamps SHALL be what it was. The plan worker's residency set SHALL be selected again when the definition's revision changes, whatever the camera did. A caller that edits items SHALL replace the definition first and then name the edited tiles to the managed loop.

Verify: A
Status: active
Source: ADR 0066 points 4 and 5, M6 plan (live definition)

#### Scenario: Swap
- **WHEN** the definition of a shared terrain is replaced by one with a stamp, then by one with a stamp in a second layer, then by one without items
- **THEN** each next field has the stamps of its definition, a field built before the replacement keeps its heights, and the last field equals the first

#### Scenario: Another base
- **WHEN** a definition with another radius is offered
- **THEN** the call is refused and the definition and its revision are unchanged

### Requirement: STRM-027 Authored stage keys
The height key of a tile of an authored stage SHALL chain the stage below (ADR 0066 point 4; the chained height key is the stage key of that point, and `stage_key` of STRM-011 is not used because a stamped tile is a `Kind::Height` record that the mesh key and the tile store chain on): its source SHALL be the key of the same tile, at the same resolution, of the stage it reads (the base first, preferred over a field-wide source key), its generator version SHALL be the stage version, its implementation id the stage's, its output parameters SHALL include the layer name (tag 8) and the layer's index in the stack of ADR 0007 (tag 9, not its priority), and its reads SHALL be the layer's footprint or region summary entries of the tile (WDEF-007, WDEF-008) plus the nodes the stage reads. The mesh key of the tile SHALL chain the stage's height key as it chains any height key. A change of the base SHALL re-key every tile of the stage; a change of the stage version, the layer name or its index SHALL re-key the tiles of that layer and of the layers above it that the stage acts on (a tile it passes through is keyed as the stage below, STRM-029); an edit of an item SHALL re-key the tiles whose footprint or summary entries change (at least those its reach touches, none beyond the margin of the query). Inserting a layer into the stack SHALL bump the stage version of every layer above it. A layer name outside the stack of ADR 0007 SHALL take the index after the stack's last, in name order among such layers (the app's field stacks its stages so). Every wrapper of a height field SHALL forward the chain (`upstream`, `stage_params`, `item_revision`, `passes_through`) with its other keying methods. A key without a stage SHALL be byte for byte what it was. The plan cache SHALL hold the origin height, mesh key and volume of a tile of a stamped field under the tile's footprint hash (the field's `inputs_hash_for`), taken again only when the field's item revision changes (a number that differs after every edit of the definition and between any two definitions of the process, a clone included), so a persistent cache plans what a fresh one plans after any put, change, delete, undo or reload, whoever made the edit; the explicit invalidation of the edited tiles stays as the fast path.

Verify: A
Status: active
Source: ADR 0066 point 4, ADR 0068, ADR 0006, CON-08

#### Scenario: Chain
- **WHEN** the base seed, the stage version, the layer name or the stack index changes, and when a stamp far from the tile changes
- **THEN** the stage's tile key, for a tile the stage acts on, changes in the first four cases and not in the last, and the mesh key follows the height key

#### Scenario: Edit locality
- **WHEN** a stamp is added, moved, edited or removed
- **THEN** the keys of the tiles its footprint finds change, none beyond the query's margin does, and removing it restores every key

#### Scenario: Two layers
- **WHEN** a base change and then an edit in the lower layer are made under a two-layer stack
- **THEN** the first re-keys the tiles of both layers and the second re-keys the upper layer's tiles of that place and no other

#### Scenario: Wrappers
- **WHEN** the source files of the code base are scanned for wrappers that forward `source_key`
- **THEN** each also forwards `upstream`, `stage_params`, `item_revision` and `passes_through`

#### Scenario: Plan cache
- **WHEN** a stamp is put, changed, deleted and put back through a shared definition, or the definition is replaced by another with the same number of edits, between plans of one persistent cache
- **THEN** each plan equals the plan of a fresh cache, and a plan after no edit takes no footprint hash

#### Scenario: Unstaged keys
- **WHEN** the keys of a base field are computed
- **THEN** they equal the pinned known answers of the key tests, which are unchanged

### Requirement: STRM-030 The roots reach the GPU before frame 0
The frame loop SHALL offer a pre-frame step that writes the roots to the tile sink (`ManagedLoop::upload_roots`): every resident tile at or above the streamer's render floor, in one batch, outside the per-frame cap (STRM-001, ADR 0064), recorded as resident exactly as the first frame records them. A harness that has loaded the roots SHALL call it before its first measured frame, so that frame 0 writes no root and obeys the cap of STRM-006 and the limits of CON-21; the perf rig (`Rig::new`) SHALL call it at the end of its start-up. A caller that does not call it SHALL see the old behaviour: the first frame writes the roots. The step SHALL change no other tile, no count and no key, and the first plan draws the roots as it did.

Verify: A, C
Status: active
Source: STRM-001, ADR 0064, window 13 (2026-10-19)

#### Scenario: Before frame 0
- **WHEN** the roots are loaded and `upload_roots` is called, then the first frame is begun
- **THEN** the sink holds every root in its slot, the first frame uploads none of them (uploaded 0, upload bytes 0) and writes nothing more, and the loop knows every root as resident

#### Scenario: Without the call
- **WHEN** the first frame is begun without `upload_roots`
- **THEN** it writes the roots, beyond the cap, as before (ADR 0064)

#### Scenario: A measured frame
- **WHEN** the streaming scenes run in smoke mode
- **THEN** frame 0 of every scene's frame log writes at most the per-frame cap

### Requirement: STRM-031 Pre-emption of stale refreshes
Refresh work made stale by a newer edit SHALL NOT be computed (ADR 0072). When `Streamer::refresh` (STRM-026) names a tile again: a refresh of it that is still waiting, queued or held, SHALL stay one entry and be counted as `refresh_coalesced`; a job running for it SHALL be told to stop (once; STRM-005 polling) and be counted as `refresh_cancelled`; and a result that arrives for a job stopped by a refresh SHALL be dropped before the tile is made resident or uploaded and be counted as `refresh_dropped`. The three counts SHALL be part of the stream statistics. `Streamer::set_refresh_settle(ms)` SHALL set a settle window on the injected clock: with `ms` above 0 a resident tile that a refresh named is not requested until `ms` have passed since the latest refresh that named it, it stays resident and stale and is served from its old version meanwhile (STRM-026), and the request is made at the first step after that; a tile that is no longer in any class when the window ends is not requested (it stays stale, as in STRM-026). With `ms` 0, the default, nothing is held and the streamer behaves as before this requirement. A refresh SHALL be dispatched in its class (STRM-026) and the draw class goes before the residency and prefetch classes, so the refreshes of the tiles the plan draws start before those of tiles it does not draw, whatever the order the edit names them in. `ManagedLoop::set_refresh_settle` SHALL forward the setting and the editor window SHALL set 25 ms (above one 60 Hz drag interval, about 17 ms, so a drag still coalesces). Twenty edits of one resident tile, each a call of `ManagedLoop::edit` less than the settle window after the one before, SHALL start at most two refresh jobs for it (the one that may already run at the first edit, and one after the last).

Verify: A
Status: active
Source: ADR 0072, owner (2026-10-20), STRM-026, STRM-005

#### Scenario: Coalesced
- **WHEN** a tile is named by three refreshes while its refresh request waits in the queue, and again while the settle holds it
- **THEN** one request is dispatched when it is its turn or the window ends, and the coalesced count is the number of refreshes that found an entry waiting

#### Scenario: Cancelled and dropped
- **WHEN** a refresh job is running for a tile and a later edit names it, and the job ends with a result
- **THEN** the job was told to stop once and counted as cancelled, its result is dropped before the tile is made resident (the old version is still the resident one, and nothing is uploaded) and counted as dropped, and the tile is refreshed once from the newest definition

#### Scenario: Settle
- **WHEN** twenty edits of one resident tile come 10 ms apart with a settle of 40 ms, and a single edit comes with a settle of 40 ms and of 0
- **THEN** the twenty start at most two jobs and the last result is made from the last definition, the single edit's job starts 40 ms after it with the settle and at once without it, and a streamer whose settle was never set behaves as before

#### Scenario: Drawn first
- **WHEN** an edit names resident tiles of the draw, residency and prefetch classes in the opposite order
- **THEN** their refreshes are dispatched draw first, then residency, then prefetch

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

### Requirement: STRM-029 A tile a stage passes through keys as its source
By exception to STRM-027, a tile a stage passes through (GEN-027) SHALL have the keys of the stage below it, in both key domains (the 128-bit persistent height and mesh keys and the 64-bit height and mesh keys of the plan, the batch keyer included), whatever the number of stages and whatever wraps them; a tile the stage acts on SHALL be keyed as STRM-027 says. So the first stamp of a world, in any layer, re-keys only the tiles its footprint touches, the tiles out of its reach keep the keys they had without the stage, and the pre-filled tiles of the plain world serve them.

Verify: A
Status: active
Source: ADR 0068, ADR 0066 point 4

#### Scenario: Both domains
- **WHEN** a stamp stage with an item near one tile is keyed at a far tile and a coarse far tile, alone, under a second stage, and wrapped in the water bound, and at the tile under the stamp
- **THEN** the far tiles' height, mesh and 64-bit keys and the batch keyer's equal the plain field's in every form, and the tile under the stamp differs in every form

#### Scenario: First stamp of a world
- **WHEN** the first stamp is put into the definition of a world through `set_definition`, and then a stamp in a second layer
- **THEN** a tile out of reach keeps the key it had in the plain world, and the tile under the stamp is re-keyed

### Requirement: STRM-033 The RAM share of terrain is capped by counts
Terrain's RAM share SHALL be 3 GiB of the 8 GiB of CON-21. Its parts SHALL be capped by counts asserted at tier A (CON-20): the bake blocks alive at most 0.5 GB (20 blocks of 1024 cells while baking; the runtime reader's cache holds at most 16 blocks), the tile pack index at most 1 GB, staged and in-flight tile bytes at most 0.1 GB, plan caches at most 0.1 GB, the chunk horizon's height tiles at most 64 (96.5 MB at 614 cells, STRM-059), and generated but deferred tiles at most 2,048. The process working set is measured in the dive perf scene (TEST-019), not in a test.

Verify: A
Status: planned
Source: ADR 0033 point 8 (accepted 2026-10-16), opus-planner (2026-10-22, M7 H2: a cap a test asserts becomes active; the rest stay planned), STRM-059

#### Scenario: A cap
- **WHEN** a part of the share reaches its cap
- **THEN** the count stops at the cap (blocks evicted, tiles deferred or refused) and the test names the part, its count and its cap

### Requirement: STRM-034 The plan cache keeps what an edit cannot reach
`SelectionCache::note_item_change(from, to, changes)` SHALL announce that the field's item revision went from `from` to `to` and where the items that differ were and are (`ItemChange`). The next plan SHALL keep, as checked at the new revision, the footprint hashes of tiles whose reads the changes cannot reach (the tile's cap with its halo, or the cover of a coarse tile, widened by the change's reach; never a tile that reads the whole body) and the certificates and cap bounds beyond eight radii of a change; the others SHALL be checked again. A revision that is not announced, or whose chain of announcements is broken, SHALL start the checks from all tiles. Plans SHALL equal a fresh cache's, node for node.

Verify: A
Status: active
Source: opus-planner (2026-10-22, SPIKE-29: the plan after an edit took 70 to 75 % of a cold plan), STRM-027, STRM-026, ADR 0074

#### Scenario: The oracle
- **WHEN** ten random edits (put, change of place and size, removal; at the view, at a cube corner and a face edge, elsewhere) are announced one by one to two caches, each planned after the edit and compared with a fresh cache's plan
- **THEN** every plan equals the fresh one, and the persistent cache computes fewer footprint hashes than the fresh one in every plan

#### Scenario: Chains and gaps
- **WHEN** two edits are announced one after the other and planned once, then an edit nobody announced, then an announcement to a revision the field does not have
- **THEN** each plan equals the fresh one, and after the unannounced edit the tiles are checked again from all (at least half of a fresh cache's checks)

#### Scenario: The reach test
- **WHEN** tiles of levels 0 to 17 round random edits (including the largest reach, cube corners and face edges) and elsewhere have their reads hashed before and after the edit
- **THEN** every tile the test spares has the same hash before and after, and it spares at least a third of the tiles

### Requirement: STRM-035 The reach of a change
A change at direction `d` with reach `r` can alter the reads of a tile when the angle from the tile's centre to `d` is within `reach(tile) + r / R` (`R` the definition's radius), widened as WDEF-007's touch test widens its limit (twice its margins) and by 1e-7 rad. `reach(tile)` is the cap of the tile with halo 1 for a fine tile; for a coarser one, its angular radius plus `(rings + 2) * 1.5` nominal cell edges (`summary::margin_rings` at halo 1), infinite when the margin needs more rings than the index allows. A tile so reached SHALL be taken as changed without its hash being made. 
Verify: A
Status: active
Source: opus-planner (2026-10-22, SPIKE-29), STRM-034, LOD-012

#### Scenario: Just beyond the reach
- **WHEN** an item is put just beyond the reach the cache gives a tile (by 10 micro-radians to 0.02 radians, with reaches of 1 m to 40 km, in a random direction), for tiles of levels 0 to 17 at 60 places
- **THEN** the cache spares the tile and its read hash is the same as without the item

### Requirement: STRM-036 The window announces its edits
Every definition the editor hands to the generator (a put, a change, a delete, an undo, a redo, a load) SHALL be announced to the plan cache before the next plan, with the places of the items that differ from the definition it replaces (`WorldDefinition::item_changes_from`) and the field's item revisions before and after.

Verify: A
Status: active
Source: opus-planner (2026-10-22, SPIKE-29), STRM-028, STRM-034

#### Scenario: Put, undo and redo
- **WHEN** a stamp is put, undone and redone in the editor, and each definition it hands over is compared with the one before
- **THEN** each gives the same one place (the stamp's), once as it was and once as it is, so the cache is told every change

### Requirement: STRM-037 The reach of a change on bounds
Certificates and cap bounds of a tile SHALL be dropped when the angle from its centre to a change is within 8 angular radii of the tile plus `r / R`, widened the same way. Certificates SHALL also be dropped when the set of levels that read height bounds (`reads_bounds`) differs, not when the field's largest height does. At most 32 announcements SHALL be kept.

Verify: A
Status: active
Source: opus-planner (2026-10-22, SPIKE-29), STRM-034, LOD-012

#### Scenario: The oracle
- **WHEN** the random edits of STRM-034 are planned
- **THEN** every plan equals a fresh one, with the certificates and cap bounds kept beyond the reach of each change

### Requirement: STRM-043 The coupled run is stored as a channel
The result of the global coupled run (GEN-040) SHALL be stored as a `TileChannel` of kind 7 in blocks of the canonical layout of its lattice (`GlobalWorld::layout(n)`): per cell the height, the drainage area in m2 and the receiver's global cell index, one `B x B` block per tile, as byte-shuffled planes in an LZ4 block. The key SHALL carry the lattice size, the steps, every run parameter, the base resolution, the algorithm version and a hash of the run's inputs, so a changed value gives every block a new key. A block read back SHALL equal what was written bit for bit; a block of another `B`, a short or damaged payload SHALL be an error naming the block.

Verify: A
Status: active
Source: opus-planner (2026-10-23, M8-1 part c), ADR 0077, R5

#### Scenario: Round trip and keys
- **WHEN** a run on a small lattice is written and read back, and a run parameter, the steps, the base resolution or an input changes
- **THEN** the arrays are equal bit for bit, every key differs from the first run's, and a payload cut short or of another `B` is refused naming the block

### Requirement: STRM-045 The coarse horizon is stored as a channel
The coarse horizon (GEN-045) SHALL be stored as a `TileChannel` of kind 9 in blocks of the canonical layout of its observer lattice (`GlobalWorld::layout(m)`): per observer cell the record of GEN-045 (`2 a + 2` bytes for `a` azimuths), one `B x B` block per tile, as an LZ4 block with the record length in its header. The key SHALL carry the observer lattice size, every parameter of the march and its version, and the whole coupled run it is computed from (STRM-043's spec and inputs hash), so a changed value gives every block a new key and no key equals a coupled-run block's.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-5), STRM-043

Detail: A block read back SHALL equal what was written bit for bit; a block of another `B`, another azimuth count, a short or damaged payload, or one missing under the current key SHALL be an error naming the block. A world with a store SHALL read the stored channel on a thread of its own when it gets its chunk stage and never wait for it on the frame path (CON-22): a world without a stored map shades with the chunk horizon alone and says once how to make it (`planet generate --coarse-horizon <store dir> --global-res N`, which reads the stored coupled run, marches and stores the channel, printing a progress line and its wall time); a stored map that the renderer cannot read (not 16 azimuths, fewer than 2 cells a face edge) is a failure naming the counts.

#### Scenario: Round trip and keys
- **WHEN** the coarse horizon of a small run is written and read back, and a march parameter, the observer lattice or the coupled run changes
- **THEN** the map is equal bit for bit, the block of a tile holds the observer cells under it, every key differs from the first one's and from the coupled run's, and a payload cut short, of another `B` or azimuth count, or missing is refused naming the block

### Requirement: STRM-048 The chunk horizon follows its chunk
`streaming::horizonjobs::HorizonJobs` SHALL run the horizon map of a chunk (GEN-044) as a job that starts only once its `ChunkSource` reports the chunk ready, on the injected spawner, at most `workers` at once, nearest first, and never waits for one (CON-22): until its map lands a tile reads the coarse horizon alone (`horizon(key)` is `None`).

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-5; coalescing added the same day), STRM-064, CON-22, ADR 0086

Detail: Each map carries a state word, the hash of the march and of the results of the chunk and of every chunk its bake reads (0 for one that is not ready). A map whose word differs from the current one is baked again, with the state word checked at dispatch and not at each landing, and only once the word has been unchanged for `settle_ms` of the injected clock, so the landing of eight neighbours one after another costs one re-bake, not eight; the number of re-bakes is in the stats. A map is dropped when its chunk leaves the source. With a disk cache a map is looked up by its tile and its state word before it is baked, and stored after: a map made against other neighbours, or another march, is a miss, and a failing entry is a logged miss.

#### Scenario: Landing and coalescing
- **WHEN** a chunk becomes ready in the source, then its horizon job runs, then its eight neighbours land one every 100 ms, then the jobs are asked after the settle window
- **THEN** no job starts before the heights, the update returns with the job waiting, the tile reads no map until it lands, the neighbours' landings start no bake until the neighbourhood has settled, and then one re-bake reads all nine chunks (two bakes, one re-bake in all)

#### Scenario: Eviction, the march and the disk
- **WHEN** the source evicts a chunk, the march changes, and a second world starts from the same disk cache
- **THEN** the chunk's map is dropped with it, a map of another march is not taken for the first, the same chunk with the same neighbourhood reads its map from the disk with no bake, and one with another neighbourhood is a miss

### Requirement: STRM-049 The world bakes the chunk horizon
A world with a chunk stage SHALL be able to enable the horizon jobs (`WorldTerrain::enable_horizon`, refused without the stage) with a baker that samples the world's own field over the chunk and a halo and bakes its horizon (GEN-044). `horizon_frame` SHALL run one frame of the jobs for the chunks the plan's tiles read, nearest first, and `horizon_map` SHALL return the landed map or none. The same world SHALL give the same map.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-5), STRM-048, STRM-026

Detail: A world with a chunk stage SHALL be able to enable the horizon jobs (`WorldTerrain::enable_horizon`, refused without the stage), with a baker that samples the world's own field (the base, the stamps and the changes of the chunks that are ready) at the cell directions of the chunk lattice over the chunk and a halo of `ceil(range / cell) + 2` cells (past a face edge on the extended cube plane), bakes the horizon of the chunk's cells (GEN-044) and reads the chunk and the chunks around it, nine in all and eight at a cube corner (STRM-048's state word), which holds only while the halo fits in one chunk edge: `enable_horizon` refuses a range whose halo is longer, and a march the bake would refuse, naming the cause (`chunk_horizon_params` gives the world's own, the range cut to a chunk edge less two cells). A world that is gone or a field that cannot be built gives a map of zeros of the right length, never a short one. `horizon_frame` SHALL run one frame of the jobs for the chunks the plan's tiles read, nearest first, and `horizon_map` SHALL return the landed map or none. The march's hash SHALL carry the lattice and the bake version; the edits enter through the chunk's edit word (STRM-057). The same world SHALL give the same map. The cell is the chunk lattice's (8 m at the A-net default).

#### Scenario: A flight
- **WHEN** a world with the stand-in chunk stage and a small lattice flies 12 s on a fake clock, then stands still, then gets an edit
- **THEN** every ready chunk has its map after its heights, the maps are one byte per azimuth per cell, the same world gives the same maps, the re-bakes stay within two per chunk, an unchanged world bakes nothing more, and an edit at the chunks bakes the maps it touches again

### Requirement: STRM-050 The chunk horizon on the pack disk cache
The chunk horizon map SHALL be stored on the tile pack under its tile, the march and its state word (the `HorizonDisk` of STRM-048), so a second run of a world reads a map made against the same neighbourhood instead of baking it. It needs a record kind of its own in the pack.

Verify: A
Status: planned
Source: opus-planner (2026-10-24, M8-5), STRM-048

#### Scenario: A second run
- **WHEN** a world flies over the same chunks twice with a pack on disk
- **THEN** the second run reads the maps from the pack and bakes none

### Requirement: STRM-046 The viewer runs A-net chunks at runtime when asked
No app code starts a chunk stage (the flag `--chunks` and the world's stage are removed, ADR 0086), so every tile the viewer, the editor and the scenes draw SHALL be the base. No frame SHALL wait for a bake (CON-22).

Verify: A
Status: active
Source: D7 (runtime generation, disk cache), ADR 0077 point 7, STRM-043, owner fallback 2026-10-28, ADR 0086

#### Scenario: Off by default
- **WHEN** the window opens a world with a stored coupled channel
- **THEN** it starts no chunk job and computes no chunk

### Requirement: STRM-055 The job log tells a refresh and its waits
The job log (STRM-022) SHALL record for each job the milliseconds from the request of its tile to its dispatch (the wait in the streamer's queue) and whether it rebuilds a stale resident tile (a refresh, STRM-026). `refresh_summary` SHALL put the refresh jobs of a slice in one line: their count and the median, 90th percentile and largest of the streamer-queue wait, the spawner-queue wait and the load, in milliseconds; none without a refresh. The viewer SHALL write that line every 10 s while there are refreshes. The readings decide nothing (CON-14).

Verify: A
Status: active
Source: m8-refresh-priority, STRM-022, STRM-026

#### Scenario: A refresh after a wait
- **WHEN** a resident tile is refreshed and 250 ms of the clock pass before the step that dispatches it
- **THEN** its sample is a refresh that is done with a streamer queue wait of 250 ms, the samples of the missing tiles before it are not refreshes with a wait of 0, and the summary counts the one refresh

### Requirement: STRM-044 The world loads the stored coupled run or makes it
A world SHALL take its coupled run (GEN-040) from the stored channel (STRM-043) beside its bake when that holds the run, and otherwise make the run and write it there; a world over an in-memory bake makes it and stores nothing. The run's lattice is `min(bake resolution, 2048)` cells per face edge and its input is the bake's elevation reduced onto that lattice (GEN-056) with a constant uplift and erodibility, and its spec (lattice, steps, bake resolution, parameters, identity of the input) is the key of STRM-043, so another bake, lattice, step count or parameter is another run and never reads the stored one; finding a stored run reads nothing of the bake.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-1 part c), ADR 0077

#### Detail
The load or run SHALL happen once on a thread, never in a frame (CON-22); until it is ready the fields stay plain, and a failure is a message naming the cause, never a default.

#### Scenario: Made once, then loaded
- **WHEN** the run of a small world is asked for twice from a store directory, then once with another step count
- **THEN** the first call makes and stores it, the second reads it back with identical maps, the third makes another run with another identifier, and a world without a store makes the run in memory

### Requirement: STRM-051 The coupled run is made by an explicit command with progress
`planet generate --coupled-run <store dir> --global-res N [--world F | --seed S]` SHALL make the coupled run of the bake stored in the directory (the viewer's `load_or_run`: the stored channel is read when it is there, else the run is made and stored) and print the stages and the wall time. The reduction of the bake onto the run's lattice SHALL report the bands it has done (`reduce_bake_with`, called once per band with the count done and the total, from any thread); the run SHALL print a line at most every 10 s with the share done, the seconds so far and the projected total. The reduced heights SHALL NOT depend on the reporting or the thread count (CON-14).

Verify: A
Status: active
Source: opus-planner (2026-10-25, M8-R rebuild)

#### Scenario: Progress
- **WHEN** a bake is reduced with a progress callback on 1 and on 4 threads
- **THEN** the callback is called once per band with counts that reach the total, and the heights equal those of `reduce_bake`

### Requirement: STRM-057 A chunk's horizon is baked again only when an edit touches it
The state word of a chunk's horizon (STRM-048) SHALL carry, instead of the definition's revision, the hash of the items of every stamp layer that touch the chunk and the halo the march reads around it (`HorizonBaker::edit_word`), 0 for none. An edit that touches none of a chunk's items SHALL NOT bake its map again; one that does SHALL, once the neighbourhood has settled, at most once per chunk.

Verify: A
Status: active
Source: m8-chunk-version, STRM-048, STRM-049

#### Scenario: An edit far from the flight
- **WHEN** a world with the horizon jobs has every map landed and gets an edit outside every chunk's halo
- **THEN** no map is baked again; an edit at the chunks bakes at most one map per chunk it touches

### Requirement: STRM-058 The main thread never takes the plan cache's lock
The plan job holds the plan cache's lock for the whole plan, which can take a second while chunk and horizon jobs hold the cores, so the main thread SHALL NOT take it. Everything the main thread asks of the cache (the tiles of an edit, a change of the item revision) SHALL wait in one ordered queue that the plan job applies, under the lock, before it plans. The queue SHALL stay bounded: a clear replaces what was queued before it, tile drops merge without duplicates, and a queue that outgrows the cache becomes a clear.

Verify: A
Status: active
Source: SPIKE-44 step 2, CON-22

#### Scenario: The queue is bounded
- **WHEN** two thousand overlapping tile drops are asked, more item changes than the cache keeps follow, and a clear follows
- **THEN** the queue holds one merged tile drop with no tile twice past 4,096 tiles, the item changes the cache keeps, and after the clear one entry that makes the tile drops after it moot while an item change after it is still queued in order

#### Scenario: A landing waits for the plan job
- **WHEN** a chunk lands while the plan cache holds the origin heights of tiles that read it and of tiles that do not, and two landings come together
- **THEN** the landing runs nothing on the calling thread, and a plan that starts after it finds no origin height of a tile that reads a landed chunk and all the others

### Requirement: STRM-059 Chunk horizons share height tiles
The chunk horizon baker (STRM-049) SHALL assemble the height raster of a chunk from chunk-sized blocks of heights that chunks share in a bounded cache, and the map SHALL be byte-identical to one baked from a raster evaluated whole.

Verify: A
Status: active
Source: opus-planner (2026-10-26, criterion 6), STRM-048, STRM-049, GEN-085, ADR 0083 point 4

Detail: Blocks inside the face are kept; a block past a face edge is evaluated each time. The cache is bounded (`HEIGHT_TILES_MAX`, 64: 96.5 MB at 614 cells) and evicts the least recently used. A block is kept under a key of everything its heights depend on: the bake version, `ANET_VERSION`, the chunk stage version, the lattice, the world's baked input, the definition apart from its items, the coupled lens and its readiness, the items that touch the block with the stamp stage's halo, and the hash of its ready result. A block whose chunk result changed while the field was built SHALL NOT be kept. The result part of the key is that of every chunk the block's heights read (its own and the neighbours whose cells the spline reads across its edge). Limit: the position of the chunk stage of a layer outside `LAYER_STACK` is not in the key.

#### Scenario: Neighbours and edits
- **WHEN** a chunk and then its eastern neighbour are baked, a hill is put in the neighbour, and the chunk is baked again
- **THEN** the second bake evaluates the three blocks it lacks and reads six, the third evaluates the blocks the hill reaches and no others, and every map equals the uncached one

### Requirement: STRM-064 The horizon maps are baked without a chunk stage
With no chunk stage the horizon jobs SHALL bake the maps of the chunks of the plan's tiles (chunk-grid level 11 and finer) from the base terrain and the stamps, in the format, march and slots of a run with a stage, and SHALL bake a map again when an edit or a change of the world word reaches it.

Verify: A
Status: active
Source: owner fallback 2026-10-28, opus-planner M8-F step 2, STRM-048, STRM-060, finding 16 of the owner test 2026-10-27

Detail: Every chunk counts as ready with no change. The bakes run on a pool of their own in the idle class, at most 8 at once (half the generation workers) and one thread a bake: a map takes about 6 s on one thread and the march scales poorly inside a map (4 threads: 1.5x) and linearly across maps; bakes that need the same height tile at once wait for the first to evaluate it. The result does not depend on how many bakes run at once (CON-14). The jobs keep the maps of the wanted chunks (at most 24) and, of the others, the most recently wanted up to the configured number beyond them. An edit that reaches a chunk or the halo of its march, a neighbour included, changes its edit word; a change of the world word (the coupled lens, the base of the definition, the stage) changes every word; the map is baked again once its word has been unchanged for the settle window. A height tile of the bake is kept only if no edit was published since the field it was evaluated from was built.

#### Scenario: Base-only maps
- **WHEN** the horizon jobs run over a source with no chunk stage and two chunks are wanted
- **THEN** both start at once, land, and are baked again after an edit word change and the settle window; maps of chunks no longer wanted go beyond the cap, the oldest first

#### Scenario: Window without a stage
- **WHEN** the window runs lit over a store
- **THEN** `chunk jobs` stays `None` and the horizon jobs bake at least one map (judged by the real-store gate `cargo xtask gate`; the small-world run reports it, because a software adapter is too slow to wait for a bake)

### Requirement: STRM-060 Background jobs run in classes and a bake never holds a tile worker
The shared job pool SHALL run a waiting job of a better class before any waiting job of a worse class (`JobClass`: visible streaming and edits, prefetch, idle; ADR 0083 point 2), in submission order within a class, and SHALL NOT preempt a running job. Idle jobs SHALL run on at most `idle_cap` workers of the pool. The chunk horizon jobs (STRM-049) SHALL be idle jobs, so that no other job waits for a horizon bake.

Verify: A
Status: active
Source: opus-planner (2026-10-27, window 8), ADR 0083 point 2, STRM-049, ADR 0086

#### Scenario: A queued job against waiting bakes
- **WHEN** a one-worker pool is busy and idle, prefetch and visible jobs are submitted in that order
- **THEN** the visible jobs start first, then the prefetch job, then the idle jobs; with an idle cap of 1 on three workers, a second idle job does not start while the first runs and two visible jobs run beside it
