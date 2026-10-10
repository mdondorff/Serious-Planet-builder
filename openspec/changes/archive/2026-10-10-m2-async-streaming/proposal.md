## Why

M2 needs a minimal asynchronous tile path so a camera can dive from orbit to 1 m without ever waiting for generation (CON-22), with time and spawning injected so it is deterministic in tests (CON-04). Hardening (disk cache, upload and VRAM budgets, velocity prefetch) stays in M4.

## What Changes

- `streaming`: `Clock`/`FakeClock`/`SystemClock`, `Spawner` with `ThreadPool` and `ManualSpawner`, `TileSource`, and `Streamer` (parent-first requests, priorities, cancellation, LRU capacity, failure retry on the injected clock, nearest-resident-ancestor resolution, blocking load of the six roots at start-up only).
- Tests: unit tests of each behaviour with fakes, a thread-pool test that proves `begin_frame` does not block on a stuck job, and a dive test (orbit to 1 m, 6 jobs per frame) that asserts something is always drawable, parent-first dispatch, cancellations and settling.
- `app`: `test-render --scene terrain --stream` generates meshes through the streaming path; its image equals the synchronous one.
- STRM-001, 002, 005 active; new STRM-007 (capacity and retry). STRM-003, 004, 006 stay planned (cache keys, cache-verify, upload/VRAM budgets belong to M3/M4).

## Capabilities

### Modified Capabilities
- `streaming-cache`: STRM-001, 002, 005 (priority wording aligned), new STRM-007.

## Impact

`crates/streaming`, `crates/frame` (dive test), `crates/app`.
