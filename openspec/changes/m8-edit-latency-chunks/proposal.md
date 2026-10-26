## Why
Criterion 6 of M8 (owner, 2026-10-26): an edit over a chunk shows within the M6 budgets and its eroded result is on the screen within 10 s for every touched chunk the view wants. The edit-latency scene (TEST-019, TEST-041 to TEST-044) builds its world without a chunk stage, so the window of 2026-10-26 measured the M6 budgets on the chain without chunks and none of the M8 default (`Under`, chunk versions, stale marking).

## What Changes
1. With `--store` and without `--no-chunks` the scene gives the world the A-net chunk stage (`chunkstage::enable_viewer_stage`: the pool, the `ChunkConfig` and the horizon jobs of the window, no disk cache), with the hold of the stale chunks while the edit's tiles are built, drives the chunk frames in its loop as the viewer does (`chunk_frame`, the landings into the managed loop) and settles the chunks before every case.
2. After each run's exact frame it keeps the frames going until the chunks are solved and drawn again and records, from `set_definition`: the longest `chunk_frame` call of the first frames (what the versions of the wanted chunks cost), the chunks that landed again, when the chunk under the edit centre and the last one landed, and when the tiles that read them were drawn (the eroded result on the screen).
3. A table of that chunk side after the stages table, with the 10 s verdict on the last column; `--no-chunks` keeps the scene as it was for the comparison line.

## Impact
- Spec: TEST-056 (tier A for the table; the scene itself runs in the M8-8 window).
- Code: `app/src/editperf.rs`, `lib.rs` (`--no-chunks`, `--no-stale-hold`, `--no-horizon-jobs`, `--chunk-workers N`, `--base-threads N`; the chunk table gives the horizon maps baked after the edit), `xtask/src/testcmd.rs` (the option is passed on).
- Not in this change: the one-round re-solve (separate proposal), brush cases over chunks (they run with the same code).
