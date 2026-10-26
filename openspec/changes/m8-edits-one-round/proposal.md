## Why
Criterion 6 (owner, 2026-10-26): the eroded result of an edit is on the screen within 10 s for every touched chunk the view wants. Readings (2026-10-26, RTX laptop, store global16384 read-only, chunks in memory, 3 chunk workers of which 2 for stale chunks, site 45 N 100 W, in-view selection, each case run once):
| Camera | Wanted chunks | Hill reach | Chunk under the edit lands | Last chunk lands |
|---|---|---|---|---|
| 300 m | 4 | 2 km | 4.8 s | 8.8 s |
| 300 m | 4 | 5 km | 7.2 s | 14.6 s |
| 1 km | 2 | 2 km, 5 km | (not wanted) | 4.3 s, 5.1 s |
| low (3 x 3 level-15 tiles) | 2 | 150 m to 2 km, 5 km | 4.4 to 4.7 s, 7.0 s | the same |
A cold landing of the 2 chunks under the low camera took 5.5 s (4 chunks at 300 m: 8.7 s). Landing to drawn adds about 2 s (queue at most 1.8 s, load at most 0.3 s, #726 and #728). With 4 wanted chunks and 2 stale workers the re-solve runs in two rounds, so the last chunk lands late.

## What Changes
Aim: the wanted touched chunks of a 300 m view (4 chunks) in one round, at most about 8 s for the job and 2 s to draw. The levers, each to be measured before it is built:
1. **(a) Edits take every chunk worker.** While an edit is pending the stale chunks use all the workers (`stale_workers = workers`), and prefetch waits (it already starts only on workers nothing else needs); ADR 0083's class "visible streaming and edits". A chunk that is missing (a new one the camera needs) still goes first.
2. **(b) The chunk pool is sized for the view.** The pool is at least the chunks a 300 m view wants (4): `chunk_workers = max(3, 4)` on a machine with the threads for it (SPIKE-44: about 15 of 20 threads idle at 1 m). Each chunk job reads its base heights on up to 4 threads, so the workers and the threads per job are set together: the reading of `--workers 4 --stale-workers 4` against `3 / 2` (and 4 jobs with 3 threads each) decides the numbers.
3. **(c) The re-solve cost.** A re-solve reads the base heights outside the edit's reach from the previous version (a per-chunk cache by footprint version; the heights inside the reach plus the routing band are made again), and the solve's threaded part (1.65 s of the 7.0 s of the 2026-10-26 reading before the gather change) is threaded if it is cheap. The phases of one re-solve (base heights, routing and beds, coarse input, solve, drivers) are measured at 2 km and 5 km first; a lever is built only if its phase is large.
4. **(d) The 5 km hill is the hard case**: its re-solve takes 7.0 s against 4.4 to 4.7 s for the smaller ones, on the same two chunks. The cause is found first (more cells inside the reach to recompute, a longer corridor, more steps of the solve), then the fix that brings it under about 8 s for the job.
5. **Measurement.** `generate --edit-resolve` gets `--stale-workers N` and a per-phase line; the same window runs the edit-latency scene with the chunk stage (TEST-056), whose chunk table gives the on-screen figure.

## Questions
- The owner's reading of 10 s is on the screen for every wanted touched chunk: with 4 chunks at 300 m this is the criterion the table must show `ok` for; a 5 km hill over 9 or more wanted chunks (a 100 m view over a large edit) is beyond what one round can do, and is stated as the bound (chunks in view, nearest first), not hidden.
- Whether a fourth chunk worker is acceptable next to the streamer workers on the machines the viewer targets (the budget of ADR 0083 point 2).

## Impact
- Spec: STRM-056 (workers on stale chunks), GEN-082 (the version cache), TEST-056; the numbers come from the M8-8 window.
- Code: `streaming/chunks.rs` (`ChunkConfig`, the stale slots), `app/anetworld.rs` (the cache of base heights by version), `app/editresolve.rs` (options and phases).
