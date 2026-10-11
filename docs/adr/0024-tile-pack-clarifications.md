# 0024 Clarifications to the tile pack format (ADR 0019)
Status: proposed
Date: 2026-10-11
Source: implementing ADR 0019 (STRM-008, `planet_cache::pack`); ADR 0019 (accepted, never edited: CON-23), ADR 0018, ADR 0023; constitution CON-05, CON-14, CON-22

## Context
ADR 0019 is accepted and fixes the pack as an append-only data file with a rebuildable Hilbert-ordered index. Implementing it showed places where the text is incomplete or the platform forced a choice. None of these changes a decision of ADR 0019; they are what a second reader (a remote server, a verifier, a compactor) needs. Nothing is stored in this format yet outside tests.

## Clarifications (what the code does; the owner may amend)
1. **The record carries its tile id.** ADR 0019's record has no tile, yet the index must be rebuildable from the data file alone and its sort key `(kind, face, level, Hilbert)` needs the tile. The record is therefore `key (16) | kind (u8) | tile id (u64 LE) | generator version (u32) | hash version (u32) | node set id (16) | payload length (u32) | payload CRC-32C (u32) | payload`: a 57-byte header.
2. **Pack header** (24 bytes): magic `PLNTPACK`, format version (u32), hash version (u32), body id (u32), 4 reserved zero bytes. A pack of another hash version or body is refused naming the file.
3. **Index file** (`.tidx`): a 40-byte header (magic `PLNTTIDX`, format version, 4 reserved bytes, data length (u64), tail CRC (u32), 4 reserved bytes, entry count (u64)), then entries of 40 bytes: sort key (11) | key (16) | record offset (u64) | payload length (u32) | one reserved zero byte. ADR 0019 estimates 48 bytes per entry; 11 + 16 + 8 + 4 is 39, so 40 is the 8-byte-aligned size and 6 million tiles are about 240 MB.
4. **Staleness.** An index is trusted only if its recorded data length equals the file length and its recorded tail CRC equals the CRC-32C of the last 4096 bytes of the data. Anything else (missing, wrong magic or size, entries not strictly sorted, other lengths) is rebuilt by one scan. Readers rebuild in memory and never write; the writer writes the index atomically (temp file and rename) when it finishes a pack, at rollover and on request.
5. **Corruption.** A scan stops at the first record that is cut short or fails its CRC and treats the rest as a torn tail: the writer truncates there, readers ignore it. Damage in the middle of a pack larger than the 4096-byte tail window is not seen by the index check and surfaces as a CRC failure on read of that record only (`PackError::Corrupt`, naming the file, offset and key), which the caller treats as a miss and regenerates; its neighbours stay readable until the next scan, which then cuts the pack at the damage.
6. **Duplicates.** Within a pack the later offset wins (earlier entries of an equal `(sort key, key)` are dropped from the index); across packs the pack with the higher sequence number wins.
7. **Size limit.** A record that would take a pack past the limit (4 GiB by default) first rolls the pack over, unless the pack holds no record yet, so a pack is at most the limit unless a single record is larger.
8. **Lock.** The advisory lock is `std::fs::File::try_lock` on `pack-<seq>.lock` (`LockFileEx` / `flock`). The Windows lock is mandatory, so no other process can read the locked file; the holder's pid and start time are written to `pack-<seq>.holder` for the message a refused writer shows. A stale lock or holder file left by a crashed writer blocks nothing: the lock is the OS's, not the file's.
9. **Not built yet:** compression (heights and fields: byte shuffle plus LZ4), compaction and the size cap, `cargo xtask cache-verify`, the streamer using the store. The payload is opaque bytes to the pack.

## Decision
None needed to proceed: these are the implementation's reading of an accepted ADR. If the owner prefers a different reading, the pack format version bumps; nothing outside tests is stored.

## Consequences
- A second implementation can read and write the same files from ADR 0019 and this ADR.
- Point 5 means one damaged byte in a large pack costs one tile now and the tail of the pack at the next scan; compaction (a later change) rewrites the live records and removes the damage.

## Licences of adopted code or techniques
None (CRC-32C implemented in-tree; std locking).
