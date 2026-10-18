## MODIFIED Requirements

### Requirement: TEST-008 Repro bundles
Every visual bug SHALL be reproducible from a bundle containing camera pose or path, the definition hash, generator version, settings (the LOD parameters among them, with the finest level `max_level` and the render floor `min_level`) and adapter information, replayed by `cargo xtask repro <bundle>` (tile scenes and terrain scenes) in every run mode that renders (`test-render` and `editor --offscreen`); a terrain bundle of a run that named a world (`--world`) SHALL also record that world (the canonical definition text, the height field id and the bake size in cells per face edge; format version 2) and replay SHALL rebuild the field from the bundle alone, refusing a recorded world that does not hash to the recorded definition hash, a field id the recorded world no longer resolves to, and a radius that differs from the request's; a bundle without a world (every bundle of a seed-only run, and every version 1 bundle) is the reference field of its seed and still replays; replay SHALL refuse a version mismatch, SHALL refuse a definition hash this build cannot rebuild (without a recorded world the single planet of the recorded seed and radius is the only definition it can) and a missing (zero) hash, each with the recorded and rebuilt hashes, and SHALL report any difference between the rebuilt and the recorded plan in numbers.

Verify: A
Status: planned
Source: CON-19, ADR 0006, M4 item 7d, LOD-011, ADR 0062

#### Scenario: Replay
- **WHEN** a bundle is replayed
- **THEN** the same FramePlan is produced and the run reports any difference in numbers

#### Scenario: Foreign definition
- **WHEN** a bundle records a definition hash this build cannot rebuild, or none
- **THEN** replay refuses it with the hashes and what to do, before replanning

#### Scenario: Terrain replay
- **WHEN** a terrain bundle recorded with a seed is replayed with another seed, another generator or mesh version, or a tampered plan
- **THEN** the replay refuses a version mismatch, rejects a corrupt bundle by its plan hash, and reports the differing plan lines (for example mesh keys) in numbers; an unchanged bundle replays to the recorded pixels

#### Scenario: World bundle
- **WHEN** a terrain run with `--world` writes a bundle and it is replayed with the world file gone
- **THEN** the replay rebuilds the field from the bundle, plans the recorded plan and draws the recorded pixels; an edited definition, a changed field id or another bake size is refused or reported in numbers

#### Scenario: Version 1 bundle
- **WHEN** a bundle of format version 1 is replayed
- **THEN** it reads as the seed-only reference field and replays as before

#### Scenario: Floor in the bundle
- **WHEN** a terrain bundle is recorded with floor 3 and replayed, and a bundle without `min_level` is replayed
- **THEN** the replayed plan equals the recorded one, and the bundle without `min_level` replays as floor 0 (the plan it recorded)

### Requirement: TEST-018 Streaming test cases
The streaming cases of ADR 0032 SHALL be camera scripts defined once in a GPU-free crate (`planet_frame::scenes`) and shared by the tier-A fake-clock tests and the perf scenes: one camera per frame of 1/60 s of script time, a pure function of the scene, the height field and a seed, with time entering only as the frame index times the step (CON-04). The dive (`dive`, `dive-fast`, and `dive-cold`, which is the dive script itself run over a reader that ignores the tile pack's pre-filled coarse tiles: TEST-019, ADR 0045 point 3) SHALL lower the altitude above the terrain by a constant factor per unit time from 20,000 km to 1 m in 30 s (equal time per decade, 7.3 decades) and then travel north on the ground at 1 m for 10 s at 1.5 m/s (`dive`) or 30 m/s (`dive-fast`), 40 s in all, looking along the direction of travel (pitch a function of time only). The teleport (`teleport-1m`, `teleport-2km`) SHALL hold a view at 2 km until it is settled (the residency set resident, nothing queued, in flight or waiting, for one second) and jump to the antipodal point at 1 m or 2 km altitude for 10 s. Mouse look (`look-1m`, `look-2km`) SHALL hold a position at 1 m or 2 km, settled, for 10 s of seeded yaw and pitch steps with a yaw reversal at every segment and a final 180 degree yaw turn within 0.2 s, from a self-contained integer generator (SplitMix64) identical on every platform. The detail lag of a frame SHALL be judged against the plan selected afresh for that frame's own camera with every tile resident (the plan the synchronous path makes for the camera; the stale-plan rule of the drawing is no part of the definition): the area-weighted mean, over the nodes of that fresh selection, of the node's level (the target) minus the drawn level, weighted by the node's projected pixels (the footprint of its tile edge at its distance from the camera, capped at the screen), where the drawn level of a node is the level of the deepest tile that is resident and drawn in that frame and is the node itself or one of its ancestors; a drawn tile finer than the target is not a negative lag (a node covered only by drawn tiles below it has lag 0); a node with no drawn tile at, above or below it is uncovered, counted as its level plus one levels and reported with its tile id; an uncovered node is a blank tile (ADR 0032's hard measure is a tile, root or ancestor, for every visible node), so zero uncovered nodes in every frame is one of the hard measures below. Every frame of a case has a lag: there is no frame "without a plan" and a gated lag rule is never "not measured" for want of one; a frame whose camera pose or drawn set was not recorded is missing data and fails the case loudly (TEST-020). The hard measures of every case SHALL be: no blank tile in any frame (the draw's own count of nodes with nothing resident, and the uncovered nodes of the frame's fresh selection; the uncovered nodes of `dive-fast` and `dive-cold` are report-only, like their other criteria), frame median at most 16.6 ms, p99 at most 20 ms and no frame over 40 ms, upload at most 1 MiB per frame and upload time at most 2 ms (judged on the p99 of the frames, the maximum printed; the start-up load of the roots, 3.4 MiB at floor 3, happens before the first measured frame and is not one of them, STRM-001), allocated terrain bytes within the tier's share in every frame, terrain-pool VRAM within the tier cap, process peak working set within 8 GiB; the detail-lag budgets SHALL be gated for the dive descent (mean at most 1 level, no frame over 3) and for a still look (lag 0, no request, cancellation or upload) and tracked, not gated, for the teleport (under 1 level within 2 s at 1 m and 1 s at 2 km, draw set complete within 9 s and 4 s) and for the dive's ground segment at walking speed (zero within 2 s), and reported without a budget for the ground segment of `dive-fast` and for the whole of `dive-cold` (the cold start, which the pre-fill takes out of the gates: its lag, descent and ground segment, is reported, never gated). `dive-cold` is never gated: every measure of it that is over its budget is printed and reported as `over (not gated)`, but a frame that draws nothing at all (a blank node: the roots, the tiles at the render floor (LOD-011), are loaded before frame 0, so it cannot happen) still fails the run. `--prefill-levels N` is an option of the binary (a streaming scene then requires levels F..=N pre-filled, F the render floor); `cargo xtask perf` runs the default 4. A budget changes only as a declared change to ADR 0032. The tier-A dive case SHALL model the generation machine, not only the clock: the tile spawner is a generation-rate model (N workers, a duration per job on the fake clock, frames of 1/60 s; `TimedSpawner`) under the default configuration of the pipelined loop (the in-flight limit and the queue ahead derived from the pool size, STRM-020), never a spawner that finishes a fixed number of jobs a frame or a limit above the pool, so that a cap or a refill that waits for the frame boundary shows as lost throughput; the dive case SHALL assert that the tiles generated per second over the frames in which requests wait reach at least 80% of N divided by the job duration (N = 8 and 20, 18 ms a job, the 1,217 tiles/s of ADR 0032 on 20 threads) and that the previous refill, with no queue ahead, does not.

Verify: A
Status: planned
Source: ADR 0032 points 1-4 (accepted, owner 2026-10-15), ADR 0031 point 3, CON-04, CON-20, STRM-002, TEST-020, STRM-020, perf window 2 (2026-10-16), LOD-011, ADR 0062

#### Scenario: Dive profile
- **WHEN** the dive script is built
- **THEN** its altitude falls monotonically by the same factor every frame over the first 1,800 frames (4.1 s per decade, 1 m at 30 s), then holds 1 m for 600 frames while the position moves north at the scene's speed, and `dive`, `dive-fast` and `dive-cold` share the descent exactly (`dive-cold` is the `dive` script in every camera)

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
- **WHEN** the recorded frames of a case have a blank node or an uncovered node (outside `dive-fast` and `dive-cold`, where it is reported), a p99 above 20 ms, a frame over 40 ms, an upload above 1 MiB or 2 ms, allocated bytes above the share, or a gated lag over its budget
- **THEN** the verdict for that measure is OVER and names the frame, the value and the budget; a tracked budget over is reported `over (tracked, not gated)`; a reading that is not available (memory) is `not measured`, never a pass; a frame without a lag is missing data and is OVER for every lag rule

#### Scenario: Mouse look at the settled state
- **WHEN** the managed loop on fakes is settled and the camera only turns, at 0.2, 0.7, 3, 10 and 15 degrees a frame (the last a half turn in 0.2 s), with and without pitch changes
- **THEN** every frame has lag 0 and no uncovered node

#### Scenario: Dive on the generation-rate model
- **WHEN** the dive wants (20,000 km to 1 m, then a hold) run through the streamer on 8 and on 20 model workers of 18 ms a job with the default queue ahead, and again with none
- **THEN** every frame with a request waiting has all workers busy, the generation rate is at least 80% of N / 18 ms, and without the queue ahead it is under 70% of it

### Requirement: TEST-019 Streaming perf scenes
The performance harness SHALL provide the scenes `dive`, `dive-fast`, `teleport-1m`, `teleport-2km`, `look-1m`, `look-2km` and `dive-cold` (`test-render --perf --scene NAME`, or `streaming` for all seven in one process, which is a convenience and not the way to measure RAM; `cargo xtask perf stream NAME`, and `cargo xtask perf stream all` (also `perf stream` without a scene), which runs the seven one after another, each in its own child process of the same binary, so that every scene starts cold and its RAM figure is its own), each driving the pipelined managed loop (STRM-016) on the real renderer along the camera script of TEST-018 at 60 frames per second of script time, paced by the real clock, which only paces and measures. A scene SHALL take `--store DIR`, a stored bake (the production source is 16384 cells per face edge; `--global-res` names another resolution), read through a fresh reader per scene, and SHALL refuse to run without it unless `--allow-memory-bake` is given, in which case the run reads the in-memory bake of `--global-res` cells, is labelled `NOT the production source` and its session is not valid for streaming budgets; the log SHALL record `protocol.source` (`store DIR, N cells` or `in-memory N cells ...`) beside `protocol.field`, followed by what the tile pack held at the start of the scene (STRM-021, ADR 0045 point 3): `store DIR, N cells; prefilled coarse pack (levels F-L present at start)` when the harness checked, at the start of the scene, that every tile of levels F to L (`--prefill-levels L`, default 4, the owner's decision of 2026-10-16; F is the render floor, LOD-011, default 3; an L below F is refused before anything runs, naming both, because no tile would be checked) is in the pack for the run's cells, finest level and tile form and the scene reads them, `...; no pre-filled coarse pack (P of T tiles of levels F-L present)` when some are missing, and `...; cold: the pack's coarse tiles are ignored, every tile is generated on demand` for `dive-cold`, whose reader (the same bake, the same field) never reads a tile from the pack; the scenes `dive` and `dive-fast` SHALL refuse to start, before the readiness gate and naming the count and the command that pre-fills the store (`generate --prefill-dir`), unless the pack holds all those tiles (a smoke run only records the marker), and `dive-cold` is a scene of its own so that cold numbers stay cold; the counts carry `prefill_tiles_used`, the tiles the scene took from the pack instead of generating. Each scene SHALL record per frame the plan, scheduler, upload, record, GPU (not measured here, and then absent) and wall time as separate columns, the detail lag against the fresh plan of the frame's camera (TEST-020) and the count of uncovered nodes, the blank-node count, the uploaded bytes, the resident, in-flight and queued counts, the dispatched, uploaded and cancelled counts and the degraded tiles, and the allocated terrain bytes, and write them to `<scene>.frames.txt` and the column statistics to the scene's log; print the verdicts of TEST-018 in the style of the terrain scene's lines, the frame verdicts holding at any clock state and no GPU verdict being given; log the clock and P-state before, during and after the case with the lowest sample in the log's `after` slot; report the RAM as the peak working set of the process that ran the scene, labelled `scene's own peak working set (child process)` when one scene ran in the process (its own world open, bake or store cache included), and judged against 8 GiB (CON-21), the driver-reported VRAM labelled as the whole GPU (an upper bound for the scene) beside the allocator's `slots allocated of share slots` (the terrain pool's slots, its ceiling since blocks are allocated on demand, against the slots the tier's terrain share holds after the pool's fixed items, in whole blocks, with the most slots resident; OVER if the pool exceeds the share's slots and counted with the gated measures), each or `not measured`, and write the same figures to the log's counts (`process_peak_bytes`, `driver_vram_bytes`, `pool_slots`, `share_slots`, `max_resident_slots`, `scene_alone`: 1 when the process ran this scene alone, so that the RAM figure is known to be the scene's own: TEST-021); print one `SCENE-MEMORY` line per scene for the orchestration; record the CPU's effective clock at the start and at the end of each case (`cpu_clock_before_mhz`, `cpu_clock_after_mhz` in the counts, or `not measured` on the console; the reading is the clock at that idle moment, not during the frames) and the active power plan with its GUID (`power_plan` on the console, `power_scheme` in the log), recorded and never judged and never compared between logs; fail the run, after writing the logs, naming the scene and the frame if any frame drew a blank tile (nothing resident to show; the draw counts such nodes, it has no ids for them, so the tile ids are given only where known) or, in every scene but `dive-fast` and `dive-cold`, left a node of its fresh selection uncovered (`BLANK TILE` lines name the scene, the frame and the tile ids); and compare a valid session only with the last valid log of the same scene (each scene has its own log and baseline), source and field, never across a source change. `perf stream all` SHALL pass `--store`, `--global-res`, `--out`, `--machine-ready`, `--smoke` and `--allow-memory-bake` to every child (`--out DIR` becomes `DIR/<scene>.json`, so each scene keeps its own log and baseline), run the readiness gate in each child before that child's measured case (each scene is gated on the machine as it then is; the gate is not shared between children), not stop at a failing child, print after the last child a summary that lists all seven scenes with each child's exit status, its own RAM against 8 GiB, its whole-GPU VRAM and its slots against the share, and exit non-zero if any child failed. A labelled smoke mode (`--perf-smoke --scene NAME`, `cargo xtask perf stream NAME --smoke`) SHALL run the same path on any adapter for a few frames and never be a measurement.

Verify: A, C
Status: planned
Source: ADR 0032 points 4 and 8, ADR 0034, ADR 0036, TEST-012, TEST-018, STRM-016, TEST-020, LOD-011, ADR 0062

#### Scenario: Scenes
- **WHEN** the scene registry is listed
- **THEN** it holds exactly `dive`, `dive-fast`, `teleport-1m`, `teleport-2km`, `look-1m`, `look-2km`, `dive-cold`, in the scripts, the verdict rules and `cargo xtask perf stream`

#### Scenario: No store
- **WHEN** a streaming scene is started without `--store` and without `--allow-memory-bake` outside smoke mode
- **THEN** it refuses, naming the store, how to make one and the memory-bake option with its label

#### Scenario: Source in the log
- **WHEN** a scene ran over a store
- **THEN** the log has `protocol.source` with the store path and the cells, and a log whose source differs (an in-memory bake, another path, none recorded) is not compared and the report says so

#### Scenario: Pre-filled start and the cold dive
- **WHEN** a store pre-filled to levels F-L is read by the scenes
- **THEN** `dive`, `dive-fast` and the others log `protocol.source` with `prefilled coarse pack (levels F-L present at start)` and count the tiles they took from the pack, `dive-cold` logs `cold: the pack's coarse tiles are ignored` and takes none, and a store with fewer tiles makes `dive` and `dive-fast` refuse before the gate naming `P of T tiles` and `generate --prefill-dir` while `dive-cold` and the other scenes go on

#### Scenario: Smoke
- **WHEN** the seven scenes run in smoke mode on the software adapter over a stored 256-cell bake pre-filled to level 3 (floor 3)
- **THEN** each completes, prints the verdict lines and `GPU time: not measured`, writes, when `--out` is given, its log (marked not valid for budgets) and its frame columns, and no blank tile is reported

#### Scenario: Blank tile
- **WHEN** a frame draws a node with no tile of its own or of an ancestor, or a node of the frame's fresh selection has nothing drawn at, above or below it (not in `dive-fast` and `dive-cold`)
- **THEN** the scene's logs are written, `BLANK TILE` lines name the scene, the frame and the tiles, and the run fails

#### Scenario: One child per scene
- **WHEN** `perf stream all` runs on a fake process runner
- **THEN** it starts seven children in the order of the registry, each with one scene and the session's options passed through, takes each scene's RAM from its own child, and lists all seven scenes in the combined summary

#### Scenario: A failing child
- **WHEN** one child exits non-zero (a blank tile, an uncovered node)
- **THEN** the remaining scenes still run and the combined exit is non-zero, naming the failed scenes

#### Scenario: RAM and slots in the verdict
- **WHEN** a scene has run alone in its process
- **THEN** the RAM line reads `scene's own peak working set (child process)` against the 8 GiB budget, the terrain slots line reads `N slots allocated of M share slots`, and the driver VRAM line says whole GPU, upper bound, beside the same slot counts

### Requirement: TEST-021 Milestone M4 acceptance
`cargo xtask accept M4` SHALL run the automated steps of `accept M3` except the discrete-GPU tier C run (format, lints and layering, spec-lint, tiers A and B, tier C on the software adapter, the three run-mode smokes, the `test fast` timing step), and SHALL judge M4's hardware exit criteria only from the logs of a valid streaming perf window (`perf/logs/<scene>.json` and `<scene>.frames.txt`, or `--logs DIR`), never from a measurement of its own. The exit criteria are those of the architecture report's M4 row as amended by the owner on 2026-10-15 (ADR 0032): on the scenes `dive`, `teleport-1m`, `teleport-2km`, `look-1m` and `look-2km`, no blank tile (the draw's own count and the uncovered nodes of the fresh selection), p99 frame at most 20 ms (and the median and maximum budgets of TEST-018), VRAM within the terrain share and the tier (the allocator's slots and bytes, the driver's whole-GPU figure beside them) (the allocator's slots and bytes gate; the driver figure is a conservative whole-GPU upper bound that fails when read over the tier cap and is reported "not measured", not refused, when absent), RAM at most 8 GiB as the scene's own peak working set, and the detail lag gated for the dive descent and the still look and tracked, not gated, for the teleports and the dive's ground segment; `dive-fast` is run and reported and is not part of the verdict. The gated dive starts with levels F-4 pre-filled (F the render floor, LOD-011, default 3) in the tile pack (owner, 2026-10-16, ADR 0045 point 3): it SHALL be judged only from a log whose `protocol.source` says `prefilled coarse pack (levels F-N present at start)` with N at least 4, `dive-fast` is shown only from such a log (otherwise as unusable, never a refusal), and the cold start is the scene `dive-cold`, which is tracked and reported and never gated, whose log must say `cold` (a log that does not is shown as unusable) and whose numbers are listed in the report beside, never in, the verdict. The other four gated scenes are judged from logs that say pre-filled or that carry no marker (logs from before the pre-fill, which read as they always did and are reported as `not stated`: they started from an empty tile cache, which is cold). The verdicts SHALL be the harness's own (`planet_perf::streaming::case_verdicts`) recomputed from the per-frame log, so the command and the scene cannot disagree. The command SHALL REFUSE, listing every reason, print the commands for the architect's window and the acceptance, write no report and exit non-zero, when: a gated scene has no log or no per-frame log; a session is invalid for frame verdicts (the readiness gate of ADR 0020, throttling per ADR 0036, a smoke run, another adapter or power scheme, an owner override, noise; thermal or hardware throttling invalidates; the software power cap, a GPU clock below the floor and a run that started with the GPU in an idle P-state do not, because those void only GPU verdicts and no M4 criterion is a GPU time; `planet_perf::validity` decides, and a log from before `frame_valid` is re-read from `valid_for_budgets` and its warnings); `protocol.source` is not a stored bake of 16384 cells per face edge (an in-memory bake is refused; the place is `protocol.source` before its `; ` marker); the gated `dive` log does not say pre-filled to level 4 or deeper (a cold run, a pack found incomplete, a shallower pre-fill, or a log from before the pre-fill, which has no marker and so counts as cold), the refusal naming the source, what it says and the commands (`generate --prefill-dir <store> --global-res 16384`, then `cargo xtask perf stream all --store <store>`); `protocol.field` is not the world's eroded field over that bake; the log is old-format (no `protocol.source`, counts missing, other `frames.txt` columns); the RAM peak is missing or not the scene's own (`scene_alone` not 1); the frames are not the full script (2400 for the dive, 600 otherwise) or the log's frame statistics differ from those of `frames.txt`; or the five scenes come from different places (the bake read, not the pack marker), fields, adapters or power schemes, or the log's upload and allocation maxima differ from those of `frames.txt`. Frame-time and count verdicts are valid at any clock state, which is printed per scene (GPU-valid or not; no GPU verdict is given, TEST-019); an earlier `.last-valid.json` is never used. A refusal is not a failure of M4. With valid logs the command SHALL write `docs/milestones/M4-report.md` (as `target/review/M4-report.generated.md` if that file already exists, never overwriting it, and never creating it when an automated step failed) holding the verdict, the criteria and tracked tables with the evidence of each scene, the clock and host facts, `dive-fast` and the automated steps, and `dive-cold` (as the cold start, with its own rows marked not gated), the tile pack's state at the start of the gated dive, and the slots for the findings no log carries (the cold start, the tracked teleport lag and dive ground segment, the upload cap of ADR 0033 point 9, the RAM split of point 8, the prefetch look-ahead of ADR 0031 question 2, the compact against the vertex GPU ratio, `gpu_ms` in the pipelined scenes), each marked TO FILL for the architect, and SHALL exit non-zero if a gated criterion fails or an automated step fails. The thresholds are named constants citing ADR 0032, ADR 0033 and CON-21.

Verify: A
Status: planned
Source: ADR 0032 (accepted, owner 2026-10-15), ADR 0033 points 1 and 8, ADR 0020, ADR 0034, ADR 0036, TEST-018, TEST-019, TEST-020, CON-21; opus-planner decisions of 2026-10-15 (M4 acceptance built before the perf window), LOD-011, ADR 0062

#### Scenario: A valid window
- **WHEN** the five gated scenes' logs come from one valid session over the 16384-cell store with the eroded field and every gated measure within its budget
- **THEN** the verdict is PASS, the report is rendered from the logs with the findings slots marked TO FILL, and `dive-fast` is shown as report-only

#### Scenario: A cold dive is refused for the gate
- **WHEN** the `dive` log's source says cold, says some tiles were missing, says a pre-fill shallower than level 4, or has no marker (a log from before the pre-fill), and the other four scenes are valid
- **THEN** the command refuses naming `dive`, the source, what it says and the commands to pre-fill the store and re-run, writes no report, and the same logs with a pre-filled `dive` (level 4 or deeper) are judged

#### Scenario: dive-cold and dive-fast
- **WHEN** `dive-cold` is present with a source that says cold and terrible frames, `dive-fast` is present with a cold source, and every gated scene passes
- **THEN** the verdict is PASS, the report has a `dive-cold` section whose rows read `over (not gated)`, `dive-fast` is shown as unusable naming its cold source, a `dive-cold` log that does not say cold is shown as unusable, and an absent `dive-cold` is `not run`, never a refusal

#### Scenario: Logs from before the pre-fill
- **WHEN** the teleport and look logs carry no marker but the `dive` log says pre-filled and the places are equal
- **THEN** they are judged, and their facts say that the tile pack state is not stated

#### Scenario: Refusals
- **WHEN** a scene is missing, a session is gate-failed or throttled, the source is an in-memory bake, a log is old-format, the RAM is not the scene's own or is missing, or the frames are partial
- **THEN** the command refuses with every reason and the window and acceptance commands, writes nothing and exits non-zero

#### Scenario: Failures
- **WHEN** a frame has a blank tile or an uncovered node, p99 exceeds 20 ms, RAM exceeds 8 GiB, the slots or allocated bytes exceed the share, or the dive descent or a still look is over its lag budget
- **THEN** the verdict is FAIL naming the scene and measure, while an over-budget teleport recovery or dive ground segment is reported as tracked and `dive-fast` never changes the verdict

#### Scenario: A completed report is kept
- **WHEN** `docs/milestones/M4-report.md` already exists
- **THEN** the generated report goes to `target/review/M4-report.generated.md`

#### Scenario: A window whose sessions started at an idle GPU clock
- **WHEN** every session passed the gate and started with the GPU in P8, and the frame verdicts of the five scenes are within budget
- **THEN** the verdict is PASS, each scene's facts say `session valid for frame verdicts: yes; GPU verdicts: NOT VALID (...)`, and a window with a smoke, wrong-adapter, power-scheme or throttled session is still refused with the reason

### Requirement: TEST-024 Store check in the smoke run
A smoke run over a stored bake (`test-render --perf-smoke --scene S --store DIR`, `cargo xtask perf stream S --smoke --store DIR`) SHALL first check the store read-only and print the result before any scene: the store, `GLOBAL_VERSION` and `MESH_VERSION`, the ocean channel read at a fixed grid of 288 directions (12 latitudes by 24 longitudes) with the counts of sea and land, and the number of tiles of levels F to L (`--prefill-levels L`, default 4; F is the render floor, LOD-011, default 3) the tile pack holds in the compact form and in the vertex form, each present tile read and decoded through the pack's checks. The run SHALL fail before any scene, naming the problem and the command that fixes it, when the store has no ocean channel, when the channel reads all land or all sea on the grid, or when the compact form lacks tiles of levels F to L (the vertex form is shown for information: the production run draws the compact form). The check SHALL write nothing to the store, and the smoke run is labelled as no measurement.

Verify: A, C
Status: planned
Source: opus-planner request of 2026-10-17 (M5 item 8, before window 6), TEST-019, STRM-021, GEN-022, ADR 0045 point 3, LOD-011, ADR 0062

#### Scenario: A complete store
- **WHEN** the smoke run reads a stored bake pre-filled to level 3 with `--prefill-levels 3`
- **THEN** it prints `store check (smoke, read-only, NOT a measurement)`, `ocean channel: present, 288 directions read, N sea and M land`, `compact form 384 of 384 present` and `store check ok`, runs its scenes, and the listing of the store directory (names, sizes, modification times) is unchanged afterwards

#### Scenario: An incomplete pack
- **WHEN** the same store is smoked with `--prefill-levels 4`
- **THEN** the run fails with `store check FAILED`, the count `of 1,920 compact tiles of levels 3-4`, `MESH_VERSION` and the `generate --prefill-dir` command, before any scene runs, and the store is unchanged

#### Scenario: A store without a usable ocean
- **WHEN** the channel is missing, or reads all land or all sea
- **THEN** the problems name `no ocean channel` and `generate --global-dir`, or `reads land on all 288` or `reads sea on all 288`
