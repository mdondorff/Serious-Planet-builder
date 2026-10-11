# 0019 Tile storage: an append-only pack with a rebuildable index, Hilbert-ordered
Status: proposed
Date: 2026-10-11
Source: background report §4 (Formats), ADR 0003 ("Hilbert-ordered storage keys are a separate decision with the tile storage ADR"), ADR 0006, ADR 0018, M3 queue item 5; constitution CON-05, CON-14, CON-22

## Context
Tiles are disposable caches of the WorldDefinition (CON-05), keyed by 128-bit content keys (ADR 0018). The disk tier sits between the RAM LRU and the generators and must serve a tile in well under a millisecond, never block a frame (CON-22), survive a crash without corrupting later reads, and let the editor and the headless `generate` mode share one store. Spatial locality matters: a camera needs a tile and its neighbours and parents together.

## Options (with trade-offs)
1. **Custom pack: append-only data file plus an index that can be rebuilt by scanning the data.** Pure Rust, no unsafe or C, compaction output deterministic and diffable by hash; durability is not required because every entry is regenerable.
2. SQLite (`rusqlite`, bundled C). Transactions, queries, tooling; but a C dependency, write amplification, and a general database for a key-to-blob problem.
3. LMDB (`heed`). Fast reads through mmap; C dependency, mmap and unsafe at the boundary, map-size management.
4. One file per tile. Simple; millions of small files, slow on Windows, awkward to ship or sync.

## Decision
Option 1.
- **Data file** (`*.tpack`): header (magic, format version, hash version, body id) then records `key (16 bytes) | kind (u8) | payload length (u32 LE) | payload CRC-32C (u32 LE) | payload`. All integers in the header, records and index are little endian; index entries are fixed-size, 8-byte aligned and have no padding bytes that carry data (reserved bytes are zero). Append only; a torn tail is detected by the length and CRC and truncated at open; nothing is ever updated in place.
- **Index** (`*.tidx`): a sorted array of `(sort key, key, offset, length)` entries, written atomically (temp file and rename) and always rebuildable from the data file by one scan; a missing or stale index is rebuilt, never trusted (header records the data file length and tail CRC it was built from). **Lookup**: a tile is requested by `TileId` and content key; derive the sort key `(kind, face, level, Hilbert)` from the `TileId`, binary-search the run of entries with that sort key (several definitions or versions can exist per tile), and match the 16-byte content key within the run. If two packs (rollover) hold the same content key, the newest record wins: packs are ordered by their creation sequence number in the file name, and within a pack the later offset wins, so the choice is deterministic. Lookups binary-search the memory-mapped-free in-RAM copy loaded at open (16 bytes of key plus offsets per tile: 6 million tiles are about 200 MB, so large stores page the index in blocks; the block size is a tuning detail).
- **Hilbert-ordered sort key** (ADR 0003): entries sort by `(kind, face, level, Hilbert index of (x, y) at that level)`, `Hilbert` the standard 2D curve over the level's grid. A tile's four children, its siblings and its neighbours are then near each other in the index and, because the data file is written in generation order, a streaming consumer asks for runs. The 64-bit `TileId` (Morton path) stays the in-memory identity; the Hilbert key is derived from it by a pure function with known answers.
- **Compression**: heights and fields use LZ4 (`lz4_flex`, pure Rust, MIT) after a lossless byte shuffle of the f32 planes (and a delta against the parent tile later if it pays); textures are stored as the GPU formats directly (BC7/BC5, later). Zstd is a later option behind the format version if sizes matter more than decode speed.
- **Validation**: the payload CRC is checked on read; a mismatch drops the entry and regenerates (a cache miss, logged with the key and file offset). `cargo xtask cache-verify` regenerates a sample from the definition and compares bytes (ADR 0006).
- **Concurrency**: one writer process per pack, many readers; the generate CLI and the editor use different packs or take the lock in turn. The lock is an OS advisory lock (`LockFileEx` on Windows, `flock` on Linux) on a lock file, so a crashed writer releases it with its process and never blocks the cache; the lock file also records pid and process start time for the message shown when a lock is held. In-process, writes go through one writer thread. A cache does not need deterministic bytes (content keys and CRCs give correctness), and which tiles share a batch depends on streaming timing, so the bytes of a pack written by streaming are not specified; only compaction output is (next item).
- **Space reclamation** (the format has no in-place assumptions, so this can be a later change): after a generator, mesh or hash-version bump the old records are dead. `cargo xtask cache-compact` (and opening a pack whose dead fraction exceeds a threshold, with the lock held) rewrites the *live* entries, those reachable from the current definition and versions, in Hilbert order (real on-disk locality) into a new pack with a new sequence number and a rebuilt index, then atomically replaces the old pack (temp file and rename). The result is a pure function of the live set, independent of thread count and of timing, so compaction output is byte-deterministic (CON-14) and hashable. An optional size cap on the cache directory evicts least-recently-used packs, and entries older than the cap, only through the same rewrite.
- **Size limits**: a pack file rolls over at 4 GiB (a 32-bit-safe offset width is not assumed: offsets are u64); packs live under the user's cache directory, never in git (CLAUDE.md's 1 MB rule applies to the repository only).

## Consequences
- Easier: cheap to implement and test (tier A, no GPU), compaction output hashable in the manifest, cloud later (the same record is the wire format: key, kind, length, CRC, payload).
- Harder: no ad-hoc queries (nothing needs them), the index needs a rebuild path and tests for torn tails and stale indexes, and block-paged index lookups are our code to get right.
- Tests (A): round trip; torn tail truncated; flipped payload byte rejected by CRC; index rebuilt equals index written; Hilbert known answers and locality (neighbours within a bounded index distance on a sample); concurrent readers with one writer; a stale lock left by a killed writer does not block a new writer; the same key in two packs resolves to the newest; compaction drops dead records and its bytes are independent of thread count and of the order tiles were first written.

## Follow-ups
- Decide Zstd versus LZ4 after real field data exists (hydrology output compresses very differently from noise).
- Memory-mapped reads are a possible later optimisation behind the same API; they need an `unsafe` review ADR first.

## Licences of adopted code or techniques
`lz4_flex` (MIT); CRC-32C implemented in-tree or via the `crc32c` crate (Apache-2.0 or MIT); Hilbert curve algorithm is public (Hilbert 1891; the iterative form is standard, no code adopted). Record the crate licences in `deny.toml` when added.
