# streaming-cache Specification

## Purpose
TBD - created by archiving change m0-foundations. Update Purpose after archive.

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
A persistent tile cache key SHALL be the first 16 bytes of BLAKE3 (`derive_key` mode, context `planet-builder cache key v1`) over a canonical encoding of everything the artefact depends on (ADR 0018, clarified by proposed ADR 0023): the format tag, the kind, the hash version and generator version, the implementation id, the face mapping id, the body id and the 128-bit body parameter hash (id, seed, radius, reference surface), the tile id, the key of its source artefact with a presence byte (a mesh from its heights), its output parameters (resolution, grid size, skirt fraction, mesh version) in ascending tag order, and the read set as entries (a node entry with the 128-bit content hash of the node read, and a query entry with the parameters and the 128-bit result hash of each spatial query, an empty result included; a result hash covers the matching items' ids, content hashes and direction bits in id order), nodes sorted by path then queries by kind and parameters, duplicates collapsed. Every variable-length part SHALL be length-prefixed and every list preceded by its count, every hash that feeds a key SHALL have its own `derive_key` context, a change of any input SHALL change the key, equal keys SHALL imply equal tile bytes (checked by STRM-004, `cache-verify`, not yet built), and the encoding SHALL have a version tag and pinned known answers that are the same on every platform.

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
`cargo xtask cache-verify` SHALL regenerate a sample of cached tiles from scratch and compare them with the cached bytes, reporting each mismatching tile ID.

Verify: A
Status: planned
Source: CON-05, report §16

#### Scenario: Missing input in a key
- **WHEN** a cached tile was produced with an input that is not part of its key
- **THEN** cache-verify reports the tile ID and the differing bytes' offset

### Requirement: STRM-008 Tile store
Tiles SHALL be stored in append-only packs with a rebuildable index (ADR 0019, clarified by proposed ADR 0024): each record carries its content key, kind, tile, versions, node set id, payload length and a CRC-32C of the payload and of its own header and is never updated in place; the index is a sorted array ordered by (kind, face, level, Hilbert index of the tile), written atomically and always rebuildable from the data by one scan, and a missing, damaged or stale index, or one that names records outside the data, SHALL be rebuilt, never trusted. A torn tail (a record cut short, or a zero-filled remainder) SHALL be truncated by the writer and ignored by readers; a record whose payload fails its CRC SHALL be skipped (a miss to regenerate, naming the file, offset and key when it is read) without cutting the records after it; a header that fails its CRC in the middle of a pack SHALL stop readers there and leave the pack untouched while the writer continues in a new one; and a read error SHALL be an error, never a torn tail. A cache SHALL degrade to misses: a pack still being created, shorter than its header, made under an older hash version or not a pack SHALL be skipped by readers and reported, never fail the store, and a pack of another body SHALL be refused naming the file. A lookup SHALL binary-search the run of entries with the tile's sort key and match the content key, several keys per tile SHALL coexist, and when a key is held twice the later record in a pack and the newer pack win. A pack SHALL roll over at a size limit and sequence numbers SHALL never wrap. One writer per directory SHALL hold an advisory OS lock that a crash releases, across rollovers, a second writer SHALL be refused naming the holder, and readers SHALL need no lock. The Hilbert curve SHALL be the standard one (consecutive indices are neighbouring cells, the four children of a cell take four consecutive indices) and the CRC the standard CRC-32C.

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

#### Scenario: Index
- **WHEN** the index is missing, damaged, older than the data or names records outside it
- **THEN** it is rebuilt by a scan and the rebuilt index equals the one a writer writes

#### Scenario: Writers
- **WHEN** a second writer opens a locked directory, even while the first rolls packs over
- **THEN** the second is refused naming the holder

#### Scenario: Stores
- **WHEN** a pack is of an older hash version, still being created, or not a pack
- **THEN** readers skip it and report it, and a pack of another body is refused naming the file

### Requirement: STRM-005 Cancellation and prioritisation
Queued and in-flight requests SHALL be cancelled when nothing needs them any more, and requests SHALL be ordered by screen-space error times visibility with parents before children.

Verify: A
Status: active
Source: report §4

#### Scenario: Cancel
- **WHEN** a node leaves view before its job starts
- **THEN** the job is cancelled and never runs

### Requirement: STRM-006 Upload and memory budgets
Upload bytes per frame, resident tile count and allocated VRAM SHALL stay within configured caps, and the VRAM cap SHALL be configurable (8 GB mid-range tier, 14 GB high tier).

Verify: A
Status: planned
Source: CON-20, CON-21

#### Scenario: Cap
- **WHEN** residency demand exceeds the cap
- **THEN** the least valuable tiles are evicted and the allocated total stays at or below the cap

### Requirement: STRM-007 Resident capacity
The streamer SHALL evict the least recently used tiles beyond a configured capacity, never evicting level-0 tiles or tiles used in the current frame (so the resident count exceeds the capacity only by tiles in use and by results arriving between frames), and SHALL retry failed tiles only after a configured delay measured on the injected clock.

Verify: A
Status: active
Source: CON-04, CON-20

#### Scenario: Interest moves away
- **WHEN** the wanted set moves to other tiles and the capacity is exceeded
- **THEN** the oldest unused tiles are evicted, roots and tiles in use stay, and a failed tile is not retried before the delay
