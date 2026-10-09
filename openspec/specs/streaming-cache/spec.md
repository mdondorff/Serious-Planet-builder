# streaming-cache Specification

## Purpose
TBD - created by archiving change m0-foundations. Update Purpose after archive.

## Requirements

### Requirement: STRM-001 Injected time, I/O and spawning
Streaming and scheduling code SHALL receive time, I/O and task spawning through injected interfaces, and tests SHALL be able to run it with a fake clock, fake I/O with injectable latency and failures, and a deterministic single-threaded spawner.

Verify: A
Status: planned
Source: CON-04

#### Scenario: Fake clock
- **WHEN** a scheduler test advances a fake clock by 5 s
- **THEN** exactly the jobs due within 5 s run, in a reproducible order

### Requirement: STRM-002 Never block a frame
For every visible node some level of detail SHALL be available: missing tiles SHALL be requested parent-first, and a frame SHALL never wait for generation or I/O.

Verify: A
Status: planned
Source: CON-22

#### Scenario: Fast dive
- **WHEN** a scripted orbit-to-ground dive runs with fake I/O of 200 ms latency
- **THEN** every frame has a resident tile for every visible node

### Requirement: STRM-003 Content-addressed keys
A tile cache key SHALL be the hash of the read-set hash, generator version, implementation ID and tile ID, and equal keys SHALL imply equal tile bytes.

Verify: A
Status: planned
Source: CON-05, ADR 0006

#### Scenario: Version bump
- **WHEN** the generator version changes
- **THEN** every key changes and no stale tile is served

### Requirement: STRM-004 Cache verification
`cargo xtask cache-verify` SHALL regenerate a sample of cached tiles from scratch and compare them with the cached bytes, reporting each mismatching tile ID.

Verify: A
Status: planned
Source: CON-05, report §16

#### Scenario: Missing input in a key
- **WHEN** a cached tile was produced with an input that is not part of its key
- **THEN** cache-verify reports the tile ID and the differing bytes' offset

### Requirement: STRM-005 Cancellation and prioritisation
Generation jobs SHALL be cancellable, and requests SHALL be ordered by screen-space error times visibility with parents before children.

Verify: A
Status: planned
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
