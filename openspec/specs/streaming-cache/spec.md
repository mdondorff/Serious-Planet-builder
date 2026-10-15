# streaming-cache Specification

## Purpose
Getting generated data to the renderer without blocking a frame (CON-22): injected time, I/O and spawning (CON-04), content-addressed persistent keys, the tile pack store with verification and compaction (CON-05, ADRs 0018, 0019, 0024, 0025, 0029), the persisted global bake blocks and stage outputs (ADR 0026), and the scheduling, cancellation and memory budgets of the residency system (CON-20, CON-21).

## Requirements

### Requirement: STRM-001 Injected time, I/O and spawning
Streaming and scheduling code SHALL receive time, I/O and task spawning through injected interfaces, and tests SHALL be able to run it with a fake clock, fake I/O with injectable latency and failures, and a deterministic single-threaded spawner.

Verify: A
Status: active
Source: CON-04

#### Scenario: Fake clock
- **WHEN** a scheduler test advances a fake clock by 5 s
- **THEN** exactly the jobs due within 5 s run, in a reproducible order

### Requirement: STRM-002 Never block a frame
For every visible node some level of detail SHALL be available: missing tiles SHALL be requested parent-first, and a frame SHALL never wait for generation or I/O.

Verify: A
Status: active
Source: CON-22

#### Scenario: Fast dive
- **WHEN** a scripted orbit-to-ground dive runs with fake I/O of 200 ms latency
- **THEN** every frame has a resident tile for every visible node

### Requirement: STRM-003 Content-addressed keys
A persistent tile cache key SHALL be the first 16 bytes of BLAKE3 (`derive_key` mode, context `planet-builder cache key v1`) over a canonical encoding of everything the artefact depends on (ADR 0018, clarified by ADR 0023): the format tag, the kind, the hash version and generator version, the implementation id, the face mapping id, the body id and the 128-bit body parameter hash (id, seed, radius, reference surface), the tile id, the key of its source artefact with a presence byte (a mesh from its heights), its output parameters (resolution, grid size, skirt fraction, mesh version) in ascending tag order, and the read set as entries (a node entry with the 128-bit content hash of the node read, and a query entry with the parameters and the 128-bit result hash of each spatial query, an empty result included; a result hash covers the matching items' ids, content hashes and direction bits in id order), nodes sorted by path then queries by kind and parameters, duplicates collapsed. Every variable-length part SHALL be length-prefixed and every list preceded by its count, every hash that feeds a key SHALL have its own `derive_key` context, a change of any input SHALL change the key, equal keys SHALL imply equal tile bytes (checked by STRM-004, `cache-verify`), and the encoding SHALL have a version tag and pinned known answers that are the same on every platform.

Verify: A
Status: active
Source: CON-05, CON-06, CON-14, ADR 0006, ADR 0018

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
Every record of a world SHALL carry a node set id: the first 16 bytes of BLAKE3 `derive_key` (context `planet-builder node set v1`) over the paths of the field's field-wide reads and of the body's radius node (`body.0.radius_m`), deduplicated, sorted by UTF-8 byte order and concatenated, each as its byte length (u32 little endian) followed by its UTF-8 bytes, with no count prefix (ADR 0025 point 3). It SHALL depend on which nodes are read and not on their values, SHALL be equal for the records of one world, and its encoding SHALL have a pinned known answer that is the same on every platform. It is per world until a cell-reading field is stored; then it SHALL become per tile (ADR 0025 point 3).

Verify: A
Status: active
Source: CON-05, CON-14, ADR 0019, ADR 0025

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
- **WHEN** tiles of the eroded field are generated over a stored bake at 1, 4 and 20 threads
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
Requests SHALL be in three classes, highest first: the missing nodes of the draw set D (ancestors before descendants; within the class by screen-space error times visibility), the rest of the residency set R (coarse before fine, nearer before farther; the view direction may order within this class but not change its membership), and prefetch (R at a position predicted from the camera translation only; yaw and pitch rates are never extrapolated). Prefetch SHALL be requested only while no request of the other two classes is waiting or running, and only into free capacity, and SHALL never evict a tile of R or D. A queued or in-flight request SHALL be cancelled only when its tile is in none of D, R and the prefetch set (with their ancestors); leaving the view frustum is never a reason. When the prefetch set changes (the travel direction changed), its requests that are no longer in it are cancelled first. An in-flight job SHALL stop at its next stage boundary, and the tile of a cancelled job SHALL NOT become resident (a cancelled job is never revived: if its tile is needed again it is requested anew after the job ends). The order of requests SHALL be a pure function of the inputs, independent of the thread count (CON-14).

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

#### Scenario: Stage boundary
- **WHEN** an in-flight job is cancelled
- **THEN** it stops before its next stage and its tile does not become resident

### Requirement: STRM-006 Upload and memory budgets
Terrain SHALL have its own share of the VRAM cap (2 GiB at the 8 GiB mid tier, 3 GiB at the 14 GiB high tier, configurable), a ceiling and not a target, counted in pooled fixed-size slots so that a test needs no device. The allocated terrain bytes (the blocks of the main pool, the small vertex pool for levels 0-2, index buffers, node records) SHALL be a pure function of the pool configuration and SHALL never exceed the share; a configuration whose fixed items exceed their reserve (0.125 GiB) or that holds no block is refused. A tile SHALL live in one slot of a pool with a free list; allocated, retiring and free slots SHALL always add up to the pool's capacity, and a slot that was given back SHALL NOT be reused until the frames in flight that may read it have retired. Resident tiles plus jobs in flight SHALL never exceed the pool's capacity in slots. When the share is smaller than the residency set R the manager SHALL degrade in this order and never touch the view first: tiles outside R and prefetch tiles are evicted first; then the leaves of R that are not in the draw set D, its ancestors or the roots are replaced by their ancestors (sibling groups merged), farthest first and, among equally far, finest first; only if D with its ancestors and the roots alone exceed the share (a budget failure, reported with the slot counts) are tiles of D replaced by their ancestors, farthest and finest first, as a declared exception to LOD-008's pixel guarantee. The result SHALL be a pure function of R, D and the capacity, and SHALL NOT depend on the order of the inputs. At the standard views the slots needed by R with its ancestors and by D with its ancestors SHALL stay within pinned counts and within pinned fractions of the share's slots.

The per-frame upload cap (ADR 0033 point 9) and the RAM budget (point 8) are proposed and not part of this requirement yet.

Verify: A, C
Status: active
Source: CON-20, CON-21, ADR 0031 point 4, ADR 0033 points 1-3, 10

#### Scenario: Terrain share
- **WHEN** the pools are configured for a tier
- **THEN** the allocated bytes (all blocks, the vertex pool, index buffers, node records) are at most the tier's terrain share, one more block would not fit, and the real GPU allocation equals the model for the blocks in use

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
- **WHEN** the ten standard views are planned over the default world
- **THEN** R with ancestors is at most 10 % and the ground-1m D with ancestors at most 6 % of the mid tier's slots of the compact tile, and the vertex-format fractions are pinned at 40 % and 12 % of the mid tier (R at most 26 % of the high tier's slots, 10 % of its compact slots)

### Requirement: STRM-007 Resident capacity
The streamer SHALL count its capacity in pooled slots (STRM-006): a slot is reserved when a job is dispatched and given back when the tile is evicted, cancelled or failed, so resident tiles plus jobs in flight SHALL NEVER exceed the capacity. It SHALL evict only to make room for waiting requests, in this order: tiles in none of D, R and the prefetch set, and prefetch-only tiles, least recently used first; for draw requests only, then tiles of R that are not in D, farthest first and finest first. It SHALL NEVER evict a level-0 tile or a tile of the current draw set or one of its ancestors, SHALL NOT let a residency or prefetch request evict a tile of R, and SHALL retry failed tiles only after a configured delay measured on the injected clock. A tile that left R stays resident until the capacity needs its slot.

Verify: A
Status: active
Source: CON-04, CON-20, ADR 0031 point 3, ADR 0033 point 3

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

### Requirement: STRM-015 Residency set
The residency set R SHALL be a pure function of the camera position, the LOD parameters (LOD-008 distances and per-node height bounds) and the horizon cut. The view direction is not an input: yaw and pitch do not change it. R SHALL hold the selected leaves in all directions and all their ancestors, in ascending tile-id order, each with a distance, and SHALL contain the draw set (LOD-007) of every view from the same position.

Verify: A
Status: active
Source: ADR 0031 points 1 and 6, LOD-007, LOD-008, CON-03

#### Scenario: Yaw independent
- **WHEN** the draw set is selected from one position for many yaw angles
- **THEN** every draw-set tile is in R, and R is identical for all of them

#### Scenario: Closed under parents
- **WHEN** R is computed
- **THEN** every ancestor of every member is a member, in ascending tile-id order
