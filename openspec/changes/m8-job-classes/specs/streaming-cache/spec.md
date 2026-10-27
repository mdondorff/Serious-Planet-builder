## ADDED Requirements

### Requirement: STRM-060 Background jobs run in classes and a bake never holds a chunk worker
The shared job pool SHALL run a waiting job of a better class before any waiting job of a worse class (`JobClass`: visible streaming and edits, prefetch, idle; ADR 0083 point 2), in submission order within a class, and SHALL NOT preempt a running job. Idle jobs SHALL run on at most `idle_cap` workers of the pool. The viewer's chunk pool SHALL have `VIEW_CHUNK_WORKERS` threads for chunk jobs plus `idle_cap` (1) for the idle class, and the chunk horizon jobs (STRM-049) SHALL be idle jobs, so that no chunk job waits for a horizon bake.

Verify: A
Status: active
Source: opus-planner (2026-10-27, window 8), ADR 0083 point 2, STRM-049, STRM-056

#### Scenario: A queued chunk job against waiting bakes
- **WHEN** a one-worker pool is busy and idle, prefetch and visible jobs are submitted in that order
- **THEN** the visible jobs start first, then the prefetch job, then the idle jobs; with an idle cap of 1 on three workers, a second idle job does not start while the first runs and two visible jobs run beside it
