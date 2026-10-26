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
