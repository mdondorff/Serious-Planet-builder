## MODIFIED Requirements

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
