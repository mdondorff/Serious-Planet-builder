## Why
B6 PR 1 of `remove-chunk-stage` (owner, 2026-10-29; ADR 0086): nothing starts the chunk stage any more.

## What changes
- Removed: the flags `--chunks`, `--no-chunks`, `--no-stale-hold`, `--chunk-workers`; `xtask gate --chunks`; `PLANET_SMOKE_CHUNKS` and `PLANET_SMOKE_LENS`. The window, the editor, the edit-latency scene and the envelope scene start the horizon jobs from the base terrain and no chunk stage.
- Removed tests that ran the stage through those hooks (two `gpu_window` cases); the default-path gate test keeps TEST-061 to TEST-064.
- Reworded: STRM-046, TEST-054, TEST-064. The new ADR 0086 records the removal and the status lines of ADR 0077, 0081, 0082 and 0084.

## Moved to later PRs
`--edit-resolve`, `--stale-workers`, `--brush` and `--base-threads` belong to `editresolve.rs` (PR 4). The log field `chunk_stage` of the envelope scene stays (always false) because `accept M8` reads it; `xtask/src/gate.rs` KNOWN ceilings and the `capture-stills` stage (TEST-048, TEST-049, TEST-052) go with their code. The archives of `m8-edit-latency-chunks` and `m8-envelope-late-chunks` wait for the PRs that delete the tests citing TEST-056 and TEST-059.

## Default path
No golden, no pixel and no hash changes: the chunk stage was off by default.
