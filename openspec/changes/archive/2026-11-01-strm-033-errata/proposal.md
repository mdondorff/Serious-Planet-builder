## Why
Two errata found by SPIKE-89 (inbox #54, #55):
- STRM-033 says the runtime reader's block cache holds at most 16 blocks; the code's budget is `SOURCE_CACHE_BYTES`, 512 MiB, 31 blocks of 1024 cells, since ADR 0033 point 6. 512 MiB is 0.5 GiB, not 0.5 GB.
- turn-cover (#522, 2026-10-21) added "STRM-033 Cover tile states". STRM-033 was already the RAM share, so the delta was archived without being synced: the rule `crates/frame/tests/cover.rs` tests lives only in the archive.

## What Changes
- STRM-069 Cover tile states: the archived text, verbatim, under a free ID (opus-planner, 2026-11-01).
- STRM-033: the reader's cache as the code has it; 0.5 GiB. The 3 GiB share is unchanged (an owner decision, with SPIKE-89).
- `cover.rs` cites STRM-069 and STRM-016 instead of STRM-033.
- spec-lint already refuses an ADDED requirement whose ID a main spec holds (`speclint.rs`, added after #522), so this collision cannot recur; two open changes adding one ID fail its duplicate-ID check.

## Capabilities
### Modified Capabilities
- `streaming-cache`: STRM-069 added, STRM-033 modified
