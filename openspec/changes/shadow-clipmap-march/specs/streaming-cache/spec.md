## MODIFIED Requirements

### Requirement: STRM-060 Background jobs run in classes and a bake never holds a chunk worker
The shared job pool SHALL run a waiting job of a better class before any waiting job of a worse class (`JobClass`: visible streaming and edits, prefetch, idle; ADR 0083 point 2), in submission order within a class, and SHALL NOT preempt a running job. Idle jobs SHALL run on at most `idle_cap` workers of the pool. The text is reworded against the spec at the time of PR 7, after B6 has removed the chunk pool.

Verify: A
Status: active
Source: opus-planner (2026-10-27, window 8), ADR 0083 point 2, ADR 0085

#### Scenario: A queued chunk job against waiting bakes
- **WHEN** a one-worker pool is busy and idle, prefetch and visible jobs are submitted in that order
- **THEN** the visible jobs start first, then the prefetch job, then the idle jobs; with an idle cap of 1 on three workers, a second idle job does not start while the first runs and two visible jobs run beside it

## REMOVED Requirements

### Requirement: STRM-045 The coarse horizon is stored as a channel
**Reason**: ADR 0085 points 4 and 7; the coarse horizon is no longer made or read.
**Migration**: PR 8. The code that makes and reads the channel goes; the channel stored in `C:/DEV/SeriousPlanetData/global16384` stays on disk untouched, and the store opens without a rebuild: a tier-A check in PR 8 on the definition hash and the store's channel list (opus-planner, 2026-10-30; the rule of B6 decision 3).

### Requirement: STRM-048 The chunk horizon follows its chunk
**Reason**: ADR 0085 point 7.
**Migration**: PR 7 of `shadow-clipmap-march`.

### Requirement: STRM-049 The world bakes the chunk horizon
**Reason**: ADR 0085 point 7.
**Migration**: PR 7.

### Requirement: STRM-050 The chunk horizon on the pack disk cache
**Reason**: ADR 0085 point 7; the pack record kind stays readable as an unknown kind (packs are not rewritten).
**Migration**: PR 7.

### Requirement: STRM-057 A chunk's horizon is baked again only when an edit touches it
**Reason**: ADR 0085 point 7; an edit's shadow follows in the frame its tile lands (REND-047).
**Migration**: PR 7.

### Requirement: STRM-059 Chunk horizons share height tiles
**Reason**: ADR 0085 point 7.
**Migration**: PR 7.

### Requirement: STRM-064 The horizon maps are baked without a chunk stage
**Reason**: ADR 0085 point 7.
**Migration**: PR 7.

### Requirement: STRM-061 A horizon bake stops when an edit makes it stale
**Reason**: ADR 0085 point 7 (synced from `m8-horizon-share` at its archive, 2026-11-01).
**Migration**: PR 7.
