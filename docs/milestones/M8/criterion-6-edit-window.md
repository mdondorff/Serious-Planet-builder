# M8 criterion 6: the edit-latency window of 2026-10-26 (recorded result)

Machine: Rasierklinge (RTX 3080 Ti Laptop), release build, store `global16384`, 2560x1440, readiness gate ok for all three runs (the first attempt of the first two was refused by the gate for competing load and rerun; nothing overridden). The chunk stage is the viewer's (`anetworld::viewer_chunk_stage`: 4 chunk workers, horizon jobs), no disk cache. Tables: `edit-latency-chunks-hold.md` (viewer stage with the stale hold), `edit-latency-chunks-nohold.md` (`--no-stale-hold`), `edit-latency-nochunks.md` (`--no-chunks`).

| Reading | With chunks (hold / no hold) | Without chunks |
|---|---|---|
| Eroded result drawn again within 10 s (19 cases) | 2 ok, 17 over / 3 ok, 16 over | n/a |
| Drawn again, typical | 10.1 to 13.1 s (1 m: 11.4 to 13.1; 200 m: 10.3 to 12.5; 2 km: 10.1 to 11.9) | n/a |
| Centre chunk landed / last chunk landed | 6.0 to 8.3 s / 10.0 to 12.7 s | n/a |
| Chunks re-solved per edit | 5 to 7 (4 for a 40 km hill) on a pool of 4: two rounds of about 5.5 s | n/a |
| M6 column (tiles the edit names refreshed and written), gated cases | 345 to 1003 ms (the recorded 20 km cases go down to 240 ms) | 69 to 326 ms |
| Preview | 2 to 14.5 ms (budget 16) | up to 10.4 ms |
| First chunk frame after an edit (the versions) | median 0.5 to 1 ms; spikes 5 to 49 ms in the first edit of a view | n/a |

## Verdict
- Criterion 6 is not met on the viewer's pools: the eroded result of every touched chunk the view wants is on screen after 10 to 13 s, because 5 to 7 chunks re-solve on 4 workers in two rounds.
- The M6 column is over its budget in 7 gated cases with chunks (the 200 m 50 m, 150 m hills and 150 m brush, the 200 m 1 km brush at 1003 ms against 1000, the 2 km 50 m, 150 m hills and 150 m brush); without chunks every case is ok.
- The stale hold (STRM-056) changes neither: the slowdown is not chunk jobs competing with the edit's tile refresh; the tiles of a world with a chunk stage cost more to rebuild and write (fewer tiles named, about 340 ms floor against 70 ms). The cause is to be profiled.
- Earlier readings of the same day on other pools (the scene's 18 workers, an overlapping prepush) are not recorded.

## Next (planner, 2026-10-26)
1. Profile one edit's tile refresh with and without the chunk layer and name the cost.
2. `--chunk-workers` and `--base-threads` in the scene, then 4x4, 6x2 and 8x2 in a quiet window (one round for 5 to 7 chunks needs 5 to 7 workers).
3. The first-chunk-frame spike: precompute the read-set discs and versions of the wanted chunks before the first edit.

## Window 2 of 2026-10-26 (main at #775 and #774; pools and base threads varied)
Machine and build as above, readiness gate ok for the four runs (6x2 and 8x2 were refused at option parsing the first time, then run). The scene tells the plan cache where the items changed after every swap (#775). Tables: `edit-latency-w5-4x4.md`, `-6x2.md`, `-8x2.md`, `-4x4-nohorizon.md` (chunk workers x base threads; the last is 4x4 with `--no-horizon-jobs`).

| Run | Eroded result drawn within 10 s (19 cases) | Drawn again, median / range (s) | M6 column over (13 gated) |
|---|---|---|---|
| 4 chunk workers x 4 base threads | 3 ok | 10.4 / 5.3 to 12.4 | 6 |
| 6 x 2 | 17 ok | 8.8 / 5.8 to 14.0 | 7 |
| 8 x 2 | 16 ok | 9.0 / 6.3 to 11.3 | 7 |
| 4 x 4, no horizon jobs | 2 ok | 11.4 / 5.4 to 12.9 | 8 |

- 5 to 7 chunks re-solve per edit; a pool of 6 does it in one round: criterion 6 is met in 17 of 19 cases (over: the 1 m 500 m hill at 14.0 s, the 200 m 1 km brush at 10.5 s). 8 workers add nothing over 6.
- The M6 column is over in 6 to 8 cases in every run (200 m and 2 km, small hills and brushes, 330 to 510 ms against 300; the 200 m 1 km brush at 1003 to 1025 ms against 1000). The plan after an edit is still delivered after 420 to 660 ms (about 100 to 175 ms without chunks) although the scene now announces the item change: the plan worker competes with the chunk jobs, or the cache does not survive the swap; not separated yet.
- Horizon jobs: no map was baked in any run (0 baked after every edit): the scene's frame loop does not call `horizon_frame`, so the horizon-off line compares nothing. To be added to the scene.

## Window 8 of 2026-10-27 (main at #782; 6 x 2, horizon jobs on and off, 300 m flight with prefetch)
Readiness gate ok for all three runs. The scene calls `horizon_frame` and `chunk_frame` each step. Tables: `edit-latency-w8-6x2-horizon.md`, `edit-latency-w8-6x2-nohorizon.md`, `envelope-w8-prefetch.json`.

| Run | Eroded result drawn within 10 s (19 cases) | M6 column over (13 gated) |
|---|---|---|
| 6 x 2, horizon jobs on | 2 ok | 8 |
| 6 x 2, `--no-horizon-jobs` | 13 ok | 7 |

- Horizon jobs cost 1 to 3 s per edit: the horizon job is in flight for the whole 10 to 17 s and shares the 6 workers with the 5 to 7 chunk jobs; almost no map finishes in the window (0 baked in 18 of 21 cases).
- The M6 column is over in 7 to 8 cases with or without horizon jobs: the plan is delivered after 390 to 1030 ms (700 to 1000 ms at 1 m, 200 m), not separated from the chunk jobs.
- 300 m flight, prefetch on (`prefetch_asked` about 840 per trial): highest speed 115 m/s, as without prefetch (floor 200 m/s). Limiter: chunk wait. Two chunks land per trial, each after a median 5 to 7 s (one solve); the stand-in share past 115 m/s is nodes whose chunk is not ready; the workers are idle at the end of most trials, so the pool and the plan time do not limit.

## Windows 9 and 10 of 2026-10-27 (main at #786 and #787; job classes, run-in envelope, bake threads)
Gate ok for every run. Files: `edit-latency-w9-6x2-horizon-jobclasses.md`, `edit-latency-w10-ht4.md`, `edit-latency-w10-ht1.md`, `envelope-w9-run-in.json`, `edit-resolve-w9-phases.log`.

| Run (6 x 2, horizon jobs on) | Eroded result drawn within 10 s (19) | Median drawn | M6 column over (13 gated) | Busy cores mean / peak (of 20) |
|---|---|---|---|---|
| Window 9, job classes (#786) | 2 | 11 s | 7 | n/a |
| Window 10, 4 bake threads | 3 | 10.5 s | 7 | 12.0 / 20 |
| Window 10, 1 bake thread (`--horizon-threads 1`) | 9 | 9.3 s | 8 | 10.7 / 20 |
| Window 8, horizon jobs off | 13 | n/a | 7 | n/a |

- The job classes alone change nothing (2 of 19 against 2): no queued chunk job waits for a bake in the scene.
- One bake thread recovers 6 of the 10 cases the horizon jobs cost; the rest is the bake's field evaluation or `horizon_frame`.
- Re-solve phases (`generate --edit-resolve`, 6 workers x 2 base threads, per chunk job): network 2.2 to 3.1 s, solve 1.9 to 2.5 s, drivers 0.1 to 0.2 s; the last chunk lands 4.7 to 5.2 s (2 chunks) and 6.2 to 6.5 s (4 chunks). In the scene the same re-solves land at 8 to 9 s with horizon jobs off.
- Envelope (`envelope-w9-run-in.json`, highest speed in m/s at 30, 100, 300, 1000 m): cold 125, 90, 115, 100; steady after a 12 s run-in 95, 110, 155, 78 (300 m: 145 with horizon off). Limiter of the steady 300 m flight: chunk wait (3 chunks land at 160 m/s after a median 6.3 s; stand-in 4.6 % of the nodes).

## Window 11 of 2026-10-27 (main at #792; track-set prefetch, AVX2 march, 60 s steady window; bake threads and stale cancel of #789 on main)
Gate ok. Files: `edit-latency-w11-6x2.md`, `envelope-w11-steady-300.json`. The full envelope scene was stopped after the 30 m and 100 m cold curves at the planner's request (it needs 1.5 to 2 h); the steady curve was run for 300 m alone (`--only-steady --altitudes-m 300`, #792 and the next PR); the other altitudes and the 12 s steady quick mode go to a later window.

| Run (6 x 2, horizon jobs on) | Eroded result drawn within 10 s (19) | M6 column over (13 gated) |
|---|---|---|
| Window 11, default bake threads, main with #789 | 9 | 8 |
| Window 10, 4 bake threads | 3 | 7 |
| Window 10, 1 bake thread | 9 | 8 |

- Last chunk landed 5.8 to 10.4 s, drawn again 6.7 to 11.4 s (1 m: 10.0 to 14.5 s; the 1 m cases are the ones over).
- Envelope, cold (the first visit after a stop, 12 s with one chunk edge), partial: 30 m 150 m/s, 100 m 65 m/s; 300 m at 80 m/s had 2.44 % chunk stand-in (the same start-up limit as window 9).
- Envelope, steady, 300 m, 12 s run-in then a 60 s window over several chunk edges, 11 trials: highest speed 460 m/s (bound 1 % over every 1 s window). Stand-in share 0 % up to 400 m/s, 0.92 % at 440, 0.98 % at 460, 1.01 % at 480, 96 % at 640; the coarse share is 0 up to 480. The floor of criterion 7 (200 m/s, owner: steady flight) is cleared 2.3 times.
- Limiter above 460 m/s: the chunk pool. At 480 m/s 28 chunks landed in 72 s, `chunk_jobs_at_end` 4 (the 6 workers busy to the end), median wait 0 s, maximum 5.3 s; about 4 chunks asked per frame. The track needs chunks faster than the pool solves them.
- Window 9 envelope for comparison (12 s steady window, one edge): steady 95, 110, 155, 78 m/s at 30, 100, 300, 1000 m.
