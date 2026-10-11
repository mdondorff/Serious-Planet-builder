## ADDED Requirements

### Requirement: STRM-008 Tile store
Tiles SHALL be stored in append-only packs with a rebuildable index (ADR 0019, clarified by proposed ADR 0024): each record carries its content key, kind, tile, versions, node set id, payload length and payload CRC-32C and is never updated in place; the index is a sorted array ordered by (kind, face, level, Hilbert index of the tile), written atomically and always rebuildable from the data by one scan, and a missing, damaged or stale index SHALL be rebuilt, never trusted. A torn tail SHALL be detected by length and CRC, truncated by the writer and ignored by readers; a payload that fails its CRC SHALL be reported as a cache miss to regenerate, naming the file, offset and key. A lookup SHALL binary-search the run of entries with the tile's sort key and match the content key, several keys per tile SHALL coexist, and when a key is held twice the later record in a pack and the newer pack win. A pack SHALL roll over at a size limit. One writer per pack SHALL hold an advisory OS lock that a crash releases, a second writer SHALL be refused naming the holder, readers SHALL need no lock, and a pack of another store (hash version or body) SHALL be refused naming the file. The Hilbert curve SHALL be the standard one (consecutive indices are neighbouring cells, the four children of a cell take four consecutive indices) and the CRC the standard CRC-32C.

Verify: A
Status: active
Source: CON-05, CON-22, ADR 0003, ADR 0019, ADR 0024

#### Scenario: Round trip and ordering
- **WHEN** records are appended, the pack finished and the store opened
- **THEN** every record is found by its kind, tile and key, an unknown key is a miss, and index entries are in Hilbert order within a kind, face and level

#### Scenario: Crash and damage
- **WHEN** a writer crashed mid-record, or a payload byte flipped
- **THEN** readers ignore the torn tail and the writer truncates it, and a flipped byte is a CRC failure on that record only

#### Scenario: Index
- **WHEN** the index is missing, damaged or older than the data
- **THEN** it is rebuilt by a scan and the rebuilt index equals the one a writer writes

#### Scenario: Writers
- **WHEN** a second writer opens a locked pack, or a writer crashed and left its lock file
- **THEN** the second is refused naming the holder, and a stale lock file blocks nothing
