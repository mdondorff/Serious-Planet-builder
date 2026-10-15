## MODIFIED Requirements

### Requirement: TEST-018 Streaming test cases
The streaming cases of ADR 0032 SHALL be camera scripts defined once in a GPU-free crate (`planet_frame::scenes`) and shared by the tier-A fake-clock tests and the perf scenes: one camera per frame of 1/60 s of script time, a pure function of the scene, the height field and a seed, with time entering only as the frame index times the step (CON-04). The dive (`dive`, `dive-fast`) SHALL lower the altitude above the terrain by a constant factor per unit time from 20,000 km to 1 m in 30 s (equal time per decade, 7.3 decades) and then travel north on the ground at 1 m for 10 s at 1.5 m/s (`dive`) or 30 m/s (`dive-fast`), 40 s in all, looking along the direction of travel (pitch a function of time only). The teleport (`teleport-1m`, `teleport-2km`) SHALL hold a view at 2 km until it is settled (the residency set resident, nothing queued, in flight or waiting, for one second) and jump to the antipodal point at 1 m or 2 km altitude for 10 s. Mouse look (`look-1m`, `look-2km`) SHALL hold a position at 1 m or 2 km, settled, for 10 s of seeded yaw and pitch steps with a yaw reversal at every segment and a final 180 degree yaw turn within 0.2 s, from a self-contained integer generator (SplitMix64) identical on every platform. The detail lag of a frame SHALL be judged against the plan selected afresh for that frame's own camera with every tile resident (the plan the synchronous path makes for the camera; the stale-plan rule of the drawing is no part of the definition): the area-weighted mean, over the nodes of that fresh selection, of the node's level (the target) minus the drawn level, weighted by the node's projected pixels (the footprint of its tile edge at its distance from the camera, capped at the screen), where the drawn level of a node is the level of the deepest tile that is resident and drawn in that frame and is the node itself or one of its ancestors; a drawn tile finer than the target is not a negative lag (a node covered only by drawn tiles below it has lag 0); a node with no drawn tile at, above or below it is uncovered, counted as its level plus one levels and reported with its tile id; an uncovered node is a blank tile (ADR 0032's hard measure is a tile, root or ancestor, for every visible node), so zero uncovered nodes in every frame is one of the hard measures below. Every frame of a case has a lag: there is no frame "without a plan" and a gated lag rule is never "not measured" for want of one; a frame whose camera pose or drawn set was not recorded is missing data and fails the case loudly (TEST-020). The hard measures of every case SHALL be: no blank tile in any frame (the draw's own count of nodes with nothing resident, and the uncovered nodes of the frame's fresh selection; the uncovered nodes of `dive-fast` are report-only, like its other criteria), frame median at most 16.6 ms, p99 at most 20 ms and no frame over 40 ms, upload at most 1 MiB per frame and upload time at most 2 ms (judged on the p99 of the frames, the maximum printed), allocated terrain bytes within the tier's share in every frame, terrain-pool VRAM within the tier cap, process peak working set within 8 GiB; the detail-lag budgets SHALL be gated for the dive descent (mean at most 1 level, no frame over 3) and for a still look (lag 0, no request, cancellation or upload) and tracked, not gated, for the teleport (under 1 level within 2 s at 1 m and 1 s at 2 km, draw set complete within 9 s and 4 s) and for the dive's ground segment at walking speed (zero within 2 s), and reported without a budget for the ground segment of `dive-fast`. A budget changes only as a declared change to ADR 0032.

Verify: A
Status: active
Source: ADR 0032 points 1-4 (accepted, owner 2026-10-15), ADR 0031 point 3, CON-04, CON-20, STRM-002, TEST-020

#### Scenario: Dive profile
- **WHEN** the dive script is built
- **THEN** its altitude falls monotonically by the same factor every frame over the first 1,800 frames (4.1 s per decade, 1 m at 30 s), then holds 1 m for 600 frames while the position moves north at the scene's speed, and `dive` and `dive-fast` share the descent exactly

#### Scenario: Teleport script
- **WHEN** a teleport script is built
- **THEN** it has a prelude camera at 2 km, 600 identical frames at the antipodal point at the scene's altitude, and is the same on every build

#### Scenario: Mouse look script
- **WHEN** the look angles are built for a seed
- **THEN** they are 600 frames, identical for the same seed and different for another, with the known first output of the generator, at least eight yaw reversals, no step faster than 180 degrees per second before the turn, pitch within its limits, and the last 12 frames turn the yaw by exactly 180 degrees

#### Scenario: Detail lag
- **WHEN** a node of level 2 is drawn by its level 0 ancestor, another by itself, a third has no drawn tile at, above or below it, and a fourth has only finer tiles drawn below it
- **THEN** the lag is the area-weighted mean of 2, 0, the third's level plus one and 0, the third is reported uncovered with its tile id, a drawn tile finer than the target is not a negative lag, and with a level 1 and a level 0 ancestor both drawn the deepest one counts

#### Scenario: Verdicts
- **WHEN** the recorded frames of a case have a blank node or an uncovered node (outside `dive-fast`, where it is reported), a p99 above 20 ms, a frame over 40 ms, an upload above 1 MiB or 2 ms, allocated bytes above the share, or a gated lag over its budget
- **THEN** the verdict for that measure is OVER and names the frame, the value and the budget; a tracked budget over is reported `over (tracked, not gated)`; a reading that is not available (memory) is `not measured`, never a pass; a frame without a lag is missing data and is OVER for every lag rule

#### Scenario: Mouse look at the settled state
- **WHEN** the managed loop on fakes is settled and the camera only turns, at 0.2, 0.7, 3, 10 and 15 degrees a frame (the last a half turn in 0.2 s), with and without pitch changes
- **THEN** every frame has lag 0 and no uncovered node

### Requirement: TEST-019 Streaming perf scenes
The performance harness SHALL provide the scenes `dive`, `dive-fast`, `teleport-1m`, `teleport-2km`, `look-1m` and `look-2km` (`test-render --perf --scene NAME`, or `streaming` for all six in one session, `cargo xtask perf stream`), each driving the pipelined managed loop (STRM-016) on the real renderer along the camera script of TEST-018 at 60 frames per second of script time, paced by the real clock, which only paces and measures. A scene SHALL take `--store DIR`, a stored bake (the production source is 16384 cells per face edge; `--global-res` names another resolution), read through a fresh reader per scene, and SHALL refuse to run without it unless `--allow-memory-bake` is given, in which case the run reads the in-memory bake of `--global-res` cells, is labelled `NOT the production source` and its session is not valid for streaming budgets; the log SHALL record `protocol.source` (`store DIR, N cells` or `in-memory N cells ...`) beside `protocol.field`. Each scene SHALL record per frame the plan, scheduler, upload, record, GPU (not measured here, and then absent) and wall time as separate columns, the detail lag against the fresh plan of the frame's camera (TEST-020) and the count of uncovered nodes, the blank-node count, the uploaded bytes, the resident, in-flight and queued counts, the dispatched, uploaded and cancelled counts and the degraded tiles, and the allocated terrain bytes, and write them to `<scene>.frames.txt` and the column statistics to the scene's log; print the verdicts of TEST-018 in the style of the terrain scene's lines, the frame verdicts holding at any clock state and no GPU verdict being given; log the clock and P-state before, during and after the case with the lowest sample in the log's `after` slot; report process peak working set and driver-reported VRAM, or `not measured`; fail the run, after writing the logs, naming the scene and the frame if any frame drew a blank tile (nothing resident to show; the draw counts such nodes, it has no ids for them, so the tile ids are given only where known) or, in every scene but `dive-fast`, left a node of its fresh selection uncovered (`BLANK TILE` lines name the scene, the frame and the tile ids); and compare a valid session only with the last valid log of the same scene (each scene has its own log and baseline), source and field, never across a source change. A labelled smoke mode (`--perf-smoke --scene NAME`, `cargo xtask perf stream NAME --smoke`) SHALL run the same path on any adapter for a few frames and never be a measurement.

Verify: C
Status: active
Source: ADR 0032 points 4 and 8, ADR 0034, ADR 0036, TEST-012, TEST-018, STRM-016, TEST-020

#### Scenario: Scenes
- **WHEN** the scene registry is listed
- **THEN** it holds exactly `dive`, `dive-fast`, `teleport-1m`, `teleport-2km`, `look-1m`, `look-2km`, in the scripts, the verdict rules and `cargo xtask perf stream`

#### Scenario: No store
- **WHEN** a streaming scene is started without `--store` and without `--allow-memory-bake` outside smoke mode
- **THEN** it refuses, naming the store, how to make one and the memory-bake option with its label

#### Scenario: Source in the log
- **WHEN** a scene ran over a store
- **THEN** the log has `protocol.source` with the store path and the cells, and a log whose source differs (an in-memory bake, another path, none recorded) is not compared and the report says so

#### Scenario: Smoke
- **WHEN** the six scenes run in smoke mode on the software adapter over a stored 256-cell bake
- **THEN** each completes, prints the verdict lines and `GPU time: not measured`, writes, when `--out` is given, its log (marked not valid for budgets) and its frame columns, and no blank tile is reported

#### Scenario: Blank tile
- **WHEN** a frame draws a node with no tile of its own or of an ancestor, or a node of the frame's fresh selection has nothing drawn at, above or below it (not in `dive-fast`)
- **THEN** the scene's logs are written, `BLANK TILE` lines name the scene, the frame and the tiles, and the run fails

### Requirement: TEST-020 Detail lag against a fresh plan of each frame's camera
The detail lag of TEST-018 SHALL be computed by one pure, GPU-free function of the fresh selection and the set of tiles drawn in the frame (`planet_frame::scenes::detail_lag`; the selection is `plan_terrain` of the frame's own request over the case's height field, so it is the plan the synchronous path makes and it is deterministic). At tier A the managed loop on fakes (fake clock, fake I/O, the reference field) SHALL compute it inline for every frame; a camera that holds still at the settled state, and one that only turns there at any rate (a 180 degree turn in 0.2 s included), has lag 0 and nothing uncovered in every frame, and a dive or a teleport script has no uncovered node in any frame. In a perf scene the lag SHALL NOT be computed on the timed path: the scene records for every frame the camera pose (the f64 `Camera` of the script) and the ids of the tiles of the plan actually drawn (resident and drawn that frame; a few kilobytes a frame), and after the case has ended and its loop been torn down (jobs cancelled, in-flight ones finished) a post-pass (`lag_post_pass`) makes the fresh selection of each recorded pose and judges it against that frame's drawn set, in contiguous chunks on scoped threads each with its own field and selection cache. A selection is a pure function of its request, so the result SHALL be identical for every thread count (CON-14) and equal, frame for frame, to the inline computation. A frame without a recorded pose SHALL fail the post-pass naming the frame (missing data); every lag verdict (gated, tracked or report-only) of a frame range containing a frame without a lag SHALL be OVER with the first such frame, never "not measured" and never a pass. The post-pass adds no frame time; its duration is printed per case, and the per-frame log carries the lag and the count of uncovered nodes (`lag`, `uncovered`), the verdicts a line `uncovered nodes` beside the lag: OVER, with the first frame and the tile ids, in every scene but `dive-fast` (an uncovered node is a blank tile: TEST-018), where it is reported.

Verify: A, C
Status: active
Source: ADR 0032 points 4 and 6, TEST-018, TEST-019, CON-03, CON-14, STRM-016

#### Scenario: Inline lag on fakes
- **WHEN** the managed loop on fakes draws a still, settled view for 30 frames
- **THEN** every frame's lag against the fresh plan of its camera is 0 with nothing uncovered, and during a dive or a teleport the lag is positive while tiles are missing and 0 once the draw set is resident

#### Scenario: Post-pass equals inline
- **WHEN** the cameras and drawn sets of a script run on fakes are replayed through the post-pass on 1, 2, 3 and 8 threads
- **THEN** every frame's lag equals the inline value exactly, for a dive, a teleport and a rotation, whatever the thread count

#### Scenario: Missing pose
- **WHEN** a frame has no recorded camera pose
- **THEN** the post-pass fails naming that frame, and a lag rule whose frames include one without a lag is OVER, not "not measured"
