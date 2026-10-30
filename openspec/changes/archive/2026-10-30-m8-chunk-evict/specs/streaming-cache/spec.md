## MODIFIED Requirements

### Requirement: STRM-041 Chunk residency, eviction and disk cache
A chunk not wanted for `chunk_evict_s` (default 30) on the injected clock SHALL be evicted from RAM, and its bytes SHALL count toward the RAM caps as a pool of their own, 512 MiB by default, the least recently wanted leaving first beyond it. A landed chunk SHALL be stored in the pack store under a key that covers the body, the process hash, the chunk grid, the pyramid top level, the radius, the key of the field below the chunk stage and the chunk, and verified on read (CON-05); a verification failure SHALL be a miss and the chunk recomputed, with the failure named in the log.

Verify: A
Status: active
Source: ADR 0077, D7 (runtime generation, disk cache), CON-05, STRM-033, opus-planner (2026-10-27: the default was 5; owner test, 1150 evictions in an hour)

#### Scenario: Evict and reload
- **WHEN** a chunk is evicted and wanted again, with its disk entry intact, then with a corrupted entry
- **THEN** the first reload reads the disk entry without computing, the second recomputes and logs the failure, and both give the identical result

#### Scenario: Not wanted for 32 s
- **WHEN** one of two ready chunks is not wanted for 32 s of the injected clock and the other is
- **THEN** the first is evicted and the second stays, and the pool's bytes never pass the cap
