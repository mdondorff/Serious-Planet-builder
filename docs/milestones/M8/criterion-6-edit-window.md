# M8 criterion 6: the edit-latency window of 2026-10-26 (recorded result)

Machine: Rasierklinge (RTX 3080 Ti Laptop), release build, store `global16384`, 2560x1440, readiness gate ok for all three runs (the first attempt of the first two was refused by the gate for competing load and rerun; nothing overridden). The chunk stage is the viewer's (`anetworld::viewer_chunk_stage`: 4 chunk workers, horizon jobs), no disk cache. Tables: `edit-latency-chunks-hold.md` (viewer stage with the stale hold), `edit-latency-chunks-nohold.md` (`--no-stale-hold`), `edit-latency-nochunks.md` (`--no-chunks`).

| Reading | With chunks (hold / no hold) | Without chunks |
|---|---|---|
| Eroded result drawn again within 10 s (19 cases) | 2 ok, 17 over / 3 ok, 16 over | n/a |
| Drawn again, typical | 10.1 to 13.1 s (1 m: 11.4 to 13.1; 200 m: 10.3 to 12.5; 2 km: 10.1 to 11.9) | n/a |
| Centre chunk landed / last chunk landed | 6.0 to 8.3 s / 10.0 to 12.7 s | n/a |
| Chunks re-solved per edit | 5 to 7 (4 for a 40 km hill) on a pool of 4: two rounds of about 5.5 s | n/a |
| M6 column (tiles the edit names refreshed and written) | 345 to 1003 ms | 70 to 326 ms |
| Preview | 2 to 14.5 ms (budget 16) | 2 to 10 ms |
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
