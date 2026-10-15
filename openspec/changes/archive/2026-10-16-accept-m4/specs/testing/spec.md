## MODIFIED Requirements

### Requirement: TEST-019 Streaming perf scenes
The performance harness SHALL provide the scenes `dive`, `dive-fast`, `teleport-1m`, `teleport-2km`, `look-1m` and `look-2km` (`test-render --perf --scene NAME`, or `streaming` for all six in one process, which is a convenience and not the way to measure RAM; `cargo xtask perf stream NAME`, and `cargo xtask perf stream all` (also `perf stream` without a scene), which runs the six one after another, each in its own child process of the same binary, so that every scene starts cold and its RAM figure is its own), each driving the pipelined managed loop (STRM-016) on the real renderer along the camera script of TEST-018 at 60 frames per second of script time, paced by the real clock, which only paces and measures. A scene SHALL take `--store DIR`, a stored bake (the production source is 16384 cells per face edge; `--global-res` names another resolution), read through a fresh reader per scene, and SHALL refuse to run without it unless `--allow-memory-bake` is given, in which case the run reads the in-memory bake of `--global-res` cells, is labelled `NOT the production source` and its session is not valid for streaming budgets; the log SHALL record `protocol.source` (`store DIR, N cells` or `in-memory N cells ...`) beside `protocol.field`. Each scene SHALL record per frame the plan, scheduler, upload, record, GPU (not measured here, and then absent) and wall time as separate columns, the detail lag against the fresh plan of the frame's camera (TEST-020) and the count of uncovered nodes, the blank-node count, the uploaded bytes, the resident, in-flight and queued counts, the dispatched, uploaded and cancelled counts and the degraded tiles, and the allocated terrain bytes, and write them to `<scene>.frames.txt` and the column statistics to the scene's log; print the verdicts of TEST-018 in the style of the terrain scene's lines, the frame verdicts holding at any clock state and no GPU verdict being given; log the clock and P-state before, during and after the case with the lowest sample in the log's `after` slot; report the RAM as the peak working set of the process that ran the scene, labelled `scene's own peak working set (child process)` when one scene ran in the process (its own world open, bake or store cache included), and judged against 8 GiB (CON-21), the driver-reported VRAM labelled as the whole GPU (an upper bound for the scene) beside the allocator's `slots allocated of share slots` (the terrain pool's slots, its ceiling since blocks are allocated on demand, against the slots the tier's terrain share holds after the pool's fixed items, in whole blocks, with the most slots resident; OVER if the pool exceeds the share's slots and counted with the gated measures), each or `not measured`, and write the same figures to the log's counts (`process_peak_bytes`, `driver_vram_bytes`, `pool_slots`, `share_slots`, `max_resident_slots`, `scene_alone`: 1 when the process ran this scene alone, so that the RAM figure is known to be the scene's own: TEST-021); print one `SCENE-MEMORY` line per scene for the orchestration; record the CPU's effective clock at the start and at the end of each case (`cpu_clock_before_mhz`, `cpu_clock_after_mhz` in the counts, or `not measured` on the console; the reading is the clock at that idle moment, not during the frames) and the active power plan with its GUID (`power_plan` on the console, `power_scheme` in the log), recorded and never judged and never compared between logs; fail the run, after writing the logs, naming the scene and the frame if any frame drew a blank tile (nothing resident to show; the draw counts such nodes, it has no ids for them, so the tile ids are given only where known) or, in every scene but `dive-fast`, left a node of its fresh selection uncovered (`BLANK TILE` lines name the scene, the frame and the tile ids); and compare a valid session only with the last valid log of the same scene (each scene has its own log and baseline), source and field, never across a source change. `perf stream all` SHALL pass `--store`, `--global-res`, `--out`, `--machine-ready`, `--smoke` and `--allow-memory-bake` to every child (`--out DIR` becomes `DIR/<scene>.json`, so each scene keeps its own log and baseline), run the readiness gate in each child before that child's measured case (each scene is gated on the machine as it then is; the gate is not shared between children), not stop at a failing child, print after the last child a summary that lists all six scenes with each child's exit status, its own RAM against 8 GiB, its whole-GPU VRAM and its slots against the share, and exit non-zero if any child failed. A labelled smoke mode (`--perf-smoke --scene NAME`, `cargo xtask perf stream NAME --smoke`) SHALL run the same path on any adapter for a few frames and never be a measurement.

Verify: A, C
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

#### Scenario: One child per scene
- **WHEN** `perf stream all` runs on a fake process runner
- **THEN** it starts six children in the order of the registry, each with one scene and the session's options passed through, takes each scene's RAM from its own child, and lists all six scenes in the combined summary

#### Scenario: A failing child
- **WHEN** one child exits non-zero (a blank tile, an uncovered node)
- **THEN** the remaining scenes still run and the combined exit is non-zero, naming the failed scenes

#### Scenario: RAM and slots in the verdict
- **WHEN** a scene has run alone in its process
- **THEN** the RAM line reads `scene's own peak working set (child process)` against the 8 GiB budget, the terrain slots line reads `N slots allocated of M share slots`, and the driver VRAM line says whole GPU, upper bound, beside the same slot counts


## ADDED Requirements

### Requirement: TEST-021 Milestone M4 acceptance
`cargo xtask accept M4` SHALL run the automated steps of `accept M3` except the discrete-GPU tier C run (format, lints and layering, spec-lint, tiers A and B, tier C on the software adapter, the three run-mode smokes, the `test fast` timing step), and SHALL judge M4's hardware exit criteria only from the logs of a valid streaming perf window (`perf/logs/<scene>.json` and `<scene>.frames.txt`, or `--logs DIR`), never from a measurement of its own. The exit criteria are those of the architecture report's M4 row as amended by the owner on 2026-10-15 (ADR 0032): on the scenes `dive`, `teleport-1m`, `teleport-2km`, `look-1m` and `look-2km`, no blank tile (the draw's own count and the uncovered nodes of the fresh selection), p99 frame at most 20 ms (and the median and maximum budgets of TEST-018), VRAM within the terrain share and the tier (the allocator's slots and bytes, the driver's whole-GPU figure beside them) (the allocator's slots and bytes gate; the driver figure is a conservative whole-GPU upper bound that fails when read over the tier cap and is reported "not measured", not refused, when absent), RAM at most 8 GiB as the scene's own peak working set, and the detail lag gated for the dive descent and the still look and tracked, not gated, for the teleports and the dive's ground segment; `dive-fast` is run and reported and is not part of the verdict. The verdicts SHALL be the harness's own (`planet_perf::streaming::case_verdicts`) recomputed from the per-frame log, so the command and the scene cannot disagree. The command SHALL REFUSE, listing every reason, print the commands for the architect's window and the acceptance, write no report and exit non-zero, when: a gated scene has no log or no per-frame log; a session is invalid for budgets (the readiness gate of ADR 0020, throttling per ADR 0036, a smoke run, another adapter or power scheme, an owner override; thermal or hardware throttling invalidates, the software power cap and a GPU clock below the floor do not); `protocol.source` is not a stored bake of 16384 cells per face edge (an in-memory bake is refused); `protocol.field` is not the world's eroded field over that bake; the log is old-format (no `protocol.source`, counts missing, other `frames.txt` columns); the RAM peak is missing or not the scene's own (`scene_alone` not 1); the frames are not the full script (2400 for the dive, 600 otherwise) or the log's frame statistics differ from those of `frames.txt`; or the five scenes come from different sources, fields, adapters or power schemes, or the log's upload and allocation maxima differ from those of `frames.txt`. Frame-time and count verdicts are valid at any clock state, which is printed per scene (GPU-valid or not; no GPU verdict is given, TEST-019); an earlier `.last-valid.json` is never used. A refusal is not a failure of M4. With valid logs the command SHALL write `docs/milestones/M4-report.md` (as `target/review/M4-report.generated.md` if that file already exists, never overwriting it, and never creating it when an automated step failed) holding the verdict, the criteria and tracked tables with the evidence of each scene, the clock and host facts, `dive-fast` and the automated steps, and the slots for the findings no log carries (the cold start, the tracked teleport lag and dive ground segment, the upload cap of ADR 0033 point 9, the RAM split of point 8, the prefetch look-ahead of ADR 0031 question 2, the compact against the vertex GPU ratio, `gpu_ms` in the pipelined scenes), each marked TO FILL for the architect, and SHALL exit non-zero if a gated criterion fails or an automated step fails. The thresholds are named constants citing ADR 0032, ADR 0033 and CON-21.

Verify: A
Status: active
Source: ADR 0032 (accepted, owner 2026-10-15), ADR 0033 points 1 and 8, ADR 0020, ADR 0034, ADR 0036, TEST-018, TEST-019, TEST-020, CON-21; opus-planner decisions of 2026-10-15 (M4 acceptance built before the perf window)

#### Scenario: A valid window
- **WHEN** the five gated scenes' logs come from one valid session over the 16384-cell store with the eroded field and every gated measure within its budget
- **THEN** the verdict is PASS, the report is rendered from the logs with the findings slots marked TO FILL, and `dive-fast` is shown as report-only

#### Scenario: Refusals
- **WHEN** a scene is missing, a session is gate-failed or throttled, the source is an in-memory bake, a log is old-format, the RAM is not the scene's own or is missing, or the frames are partial
- **THEN** the command refuses with every reason and the window and acceptance commands, writes nothing and exits non-zero

#### Scenario: Failures
- **WHEN** a frame has a blank tile or an uncovered node, p99 exceeds 20 ms, RAM exceeds 8 GiB, the slots or allocated bytes exceed the share, or the dive descent or a still look is over its lag budget
- **THEN** the verdict is FAIL naming the scene and measure, while an over-budget teleport recovery or dive ground segment is reported as tracked and `dive-fast` never changes the verdict

#### Scenario: A completed report is kept
- **WHEN** `docs/milestones/M4-report.md` already exists
- **THEN** the generated report goes to `target/review/M4-report.generated.md`
