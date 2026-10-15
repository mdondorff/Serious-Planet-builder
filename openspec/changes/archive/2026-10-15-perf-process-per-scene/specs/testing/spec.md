## MODIFIED Requirements

### Requirement: TEST-012 Terrain performance workload
The performance harness SHALL provide a terrain frame workload at 1440p over scripted views from orbit to 1 m that generates and uploads meshes once, then reports per run the GPU time (timestamp queries), CPU planning time, CPU record time, wall time and the deterministic counts (nodes, draw calls, triangles, resident vertex bytes), checks the terrain budget (median GPU time at most 8 ms, p99 frame time at most 20 ms) and the CPU planning budget (median time to plan a frame at most 8 ms per view in steady state: with the per-tile selection cache warm and the camera stepping a few centimetres per plan; the cold first plan is reported beside it), measures the machine before any measured run and refuses, listing every failed condition with its measured value, unless it is ready (ADR 0020): AC power connected, a performance power scheme, the reference adapter, no `cargo`, `rustc`, `cargo-nextest`, `planet` or `clippy-driver` process outside the harness's own process tree, mean CPU load below 15 % and mean GPU utilisation below 10 % over 10 s, GPU temperature below 70 degrees C; the owner's `--machine-ready` overrides a failed gate and the override and the failed conditions are recorded in the log (`owner_override`); after the run the session is valid only if no run reported thermal or hardware throttling (hardware slowdown, software or hardware thermal, power brake; an unparseable reason counts) in the clock samples taken at its start and end (a missing sample counts); the software power cap (throttle reason 0x4), the laptop GPU's normal governor at full load, is recorded per run (`power_cap`, and the raw reasons in the samples) and printed (`[power cap 0x4 recorded, not invalidating]`) but invalidates neither a view nor the session (ADR 0036 point 1) and the run-to-run noise of every view is at most 10 % (one retry of that view's runs), otherwise the session says so before it prints any verdict (`SESSION INVALID for budgets`, and in the log) and its verdicts do not count; before a view's runs the harness SHALL bring the GPU to its loaded state (ADR 0034, ADR 0036): it renders the view in chunks of 60 frames and reads the graphics clock after each until it reads at least the clock floor of 1,400 MHz (a named constant citing ADR 0036, whatever the P-state), up to a cap of 30 s per view (inclusive), prints and logs (`warmups`) what it did and never refuses for it; the clock state of a view (the P-state or P-states and the median graphics clock of the samples before and after the runs of its final batch) SHALL be logged per run (`gpu_valid`, `gpu_clock_state`), and a view is valid for GPU budgets only if every one of those samples exists and reads a graphics clock of at least 1,400 MHz, whatever the P-state (P2 at 1,710 MHz is valid, P0 at 1,200 MHz is not, one sample below the floor invalidates the view, a missing sample is unknown and never valid); otherwise it is NOT VALID for GPU budgets: its GPU time gets no verdict (the console prints `NOT VALID for GPU budgets (<lowest sample and the floor>)` and shows the GPU median without one), while its frame time (median and p99, judged against the frame budget as measured because that is what a user sees, ADR 0034 as accepted with the owner's change of 2026-10-15), its CPU times and counts and the session's validity are unaffected, and every view's console line prints its clock state beside the frame figures (`[clock state <state>]`); the workload SHALL run the default world (GEN-018: the eroded field over an in-memory bake of `--global-res` cells per face edge) unless `--world` names another, and the log SHALL record which field and bake it measured (`protocol.field`); it offers a labelled smoke mode that exercises the path on any adapter without being a measurement. A valid session SHALL be compared with the last valid log of the same scene, frame size and measured field (a baseline that measured another field, or recorded none, is not compared: the report says so and the session becomes the baseline), and every median (frame, GPU, CPU, plan) that rose by more than 15 % of the old median, and by at least 0.1 ms, SHALL be reported with both figures, as well as every count that changed (report only: the comparison fails nothing); the GPU time of a view SHALL be compared only with that of a baseline view that is valid for GPU budgets at equal clock state (both views valid for GPU budgets and graphics clocks within 5 % of the larger; the P-state is printed, not compared), otherwise the report says `not comparable: clock state <old> vs <new>` or that the view was not compared and why, while the frame medians, CPU times and counts are compared regardless of the clock state, a frame-median regression line carries `[clock state <old> vs <new>]` and a view whose states are unequal (or unknown: a log from before the samples) gets a line `frame medians of <view> compared across unequal clock states ...: clock state <old> vs <new>`; the baseline of the GPU figures is `terrain.last-gpu-valid.json` beside the log (the last valid session with at least one view valid for GPU budgets; the previous log stands in before it exists), compared per view; an invalid session SHALL be neither compared nor kept as the baseline (the baseline is `terrain.last-valid.json` beside the log; before it exists, the previous log stands in if it was valid). Every gate and validity rule is a pure function over measured facts. A frame loop that pipelines its work (STRM-016, ADR 0032 point 5) SHALL report, for every frame it draws, the time of the plan (the worker's time for the plan delivered since the last frame), the scheduler, the upload, the record, the GPU and the wall time as separate columns named `plan_ms`, `scheduler_ms`, `upload_ms`, `record_ms`, `gpu_ms` and `wall_ms` (a submitted frame that was not waited for has no GPU time and reports none), and a frame over budget SHALL be attributed to the largest of scheduler, upload, record and GPU; the plan runs off the frame path and is not one of them. The same loop SHALL report, after the scheduler counts, the tile bytes written that frame (`upload_bytes`), the finished tiles deferred by the per-frame upload budget (`deferred`) and the node-record bytes (`node_bytes`), so that the upload cap (STRM-006, a byte count asserted at tier A) and the upload time (a perf window) are read from the same log line. A streaming case (TEST-019) SHALL also log the CPU's effective clock before and after the case (`cpu_clock_before_mhz`, `cpu_clock_after_mhz`: the base frequency times `% Processor Performance`) and print it with the active power plan's name and GUID, as facts of the host only: they invalidate no view or session, a change in them (and in `max_resident_slots`) is not reported as a changed count, and a host that cannot read them logs nothing and prints `not measured`; a log without these counts stays readable and comparable.

Verify: C
Status: active
Source: CON-20, CON-21, M4 item 7d, ADR 0034, ADR 0036 points 1 and 2, ADR 0032 point 5, ADR 0033 point 9

#### Scenario: Refusal
- **WHEN** the workload is started on a software adapter, with or without the machine-ready flag
- **THEN** it refuses with the adapter and the corrective action (the adapter is never overridable: CON-21 requires the discrete GPU confirmed by the harness)

#### Scenario: Smoke
- **WHEN** the smoke mode runs on the software adapter
- **THEN** it prints that it is not a measurement, completes every view and writes no log unless asked

#### Scenario: Plan time verdict
- **WHEN** the terrain workload has planned a view
- **THEN** it reports the median warm CPU plan time with the 8 ms budget and a verdict (ok or OVER), and the cold plan time, for that view

#### Scenario: Cache equivalence
- **WHEN** plans are made with a cache that has seen earlier frames and another field
- **THEN** every plan equals the plan made without a cache, bit for bit

#### Scenario: Readiness gate refusal
- **WHEN** the gate sees battery power, a non-performance power scheme, a build process outside the harness's process tree, CPU load or GPU utilisation at or above its limit, or a GPU temperature at or above 70 degrees C
- **THEN** it refuses and reports every failed condition with its measured value; a value that could not be read is a refusal, never a pass

#### Scenario: Harness process tree
- **WHEN** the process table holds `cargo` ancestors of the harness and a child of the harness
- **THEN** they are not counted; a `cargo` or `rustc` process elsewhere (including a sibling of an ancestor) is

#### Scenario: Owner override
- **WHEN** the gate fails and the owner passed `--machine-ready`
- **THEN** the run continues, the failed conditions are printed, the log records them under `owner_override`, and the session is not valid for budgets

#### Scenario: Field change
- **WHEN** a valid session measured the eroded field and the baseline measured the reference field (or recorded none)
- **THEN** no median or count is compared, the report names both fields, and the session replaces the baseline

#### Scenario: Regression report
- **WHEN** a valid session's ground-1m GPU median is 4.2 ms and the last valid log's is 3.0 ms
- **THEN** the console reports the path, metric, both medians and the rise (+40 %) against the 15 % limit, and a rise of 14 %, or of less than 0.1 ms, is not reported

#### Scenario: No baseline
- **WHEN** the session is invalid, the earlier log is invalid or unreadable, or it was measured at another frame size
- **THEN** nothing is compared and the console says why; only a valid session replaces the baseline

#### Scenario: Session validity
- **WHEN** a run reports hardware slowdown, thermal throttling or a power brake (or an unparseable reason), or a view's run-to-run noise stays above 10 % after one retry
- **THEN** the session prints `SESSION INVALID for budgets` with the reasons before any verdict and the log has `valid_for_budgets` false

#### Scenario: Power cap
- **WHEN** a run reports throttle reason 0x4 (software power cap) and nothing else but idle, applications-clock or sync-boost bits
- **THEN** the log has `power_cap` true for the runs of that view and the raw reasons in its samples, the console prints `[power cap 0x4 recorded, not invalidating]` on the view's line, and neither the view nor the session is invalid; 0x4 together with a thermal, hardware-slowdown or power-brake bit still invalidates the session, and the cap is still recorded

#### Scenario: Warm-up to the clock floor
- **WHEN** a view is warmed up and the graphics clock reads 1,150 MHz, then 1,300 MHz, then 1,710 MHz (in P2 or P0), or stays at 1,150 MHz until the 30 s cap
- **THEN** the warm-up stops at the first reading of at least 1,400 MHz whatever the P-state, a reading at the floor at exactly the cap counts, a cap that is hit leaves the run going with the last reading printed and logged, and a sampler that returns nothing never counts as reaching the floor

#### Scenario: View below the clock floor
- **WHEN** the samples of a view's runs read 1,150 MHz in P2, or 1,200 MHz in P0, or 1,470 MHz with one sample at 1,150 MHz, or one is missing
- **THEN** the log has `gpu_valid` false for its runs with the lowest sample and the floor as reason (`no clock sample` when one is missing), the console prints `NOT VALID for GPU budgets (P2 1150 MHz, below the 1400 MHz floor)` with the GPU median and no GPU verdict, keeps the frame median, the p99 and its verdict with `[clock state ...]` beside them, and the session stays valid for budgets

#### Scenario: View at the clock floor in any P-state
- **WHEN** the samples of a view read P2 at 1,710 MHz, or P0 at 1,400 MHz, or P0 and P2 mixed with every clock at or above 1,400 MHz
- **THEN** the view is valid for GPU budgets (`gpu_valid` true, `gpu_clock_state` naming the P-state or P-states and the median clock), and its GPU median gets a verdict

#### Scenario: Equal clock state
- **WHEN** a view at P0 1470 MHz is compared with a view at P0 1450 MHz, at P2 1500 MHz, at P2 1710 MHz, and with a view at P2 1150 MHz
- **THEN** the first two pairs' GPU medians are compared (the P-state is not compared), the third is reported as `not comparable: clock state P0 1470 MHz vs P2 1710 MHz` (14 % apart), the last as not valid for GPU budgets, and no GPU regression is reported for the last two

#### Scenario: Frame medians at any clock state
- **WHEN** a session at P0 1470 MHz (frame median 4.0 ms) is compared with a session at P2 1150 MHz (5.2 ms) of the same view, with a baseline log from before the clock samples, with a session at P2 1710 MHz against another at P2 1710 MHz, and with two sessions that both read 1,150 MHz
- **THEN** the frame medians are compared in all cases, a rise is reported with `[clock state <old> vs <new>]`, unequal states (a view below the floor is never equal, being not valid for GPU budgets) add the line `frame medians of <view> compared across unequal clock states`, equal valid states add none, and the GPU medians are not compared across unequal states

#### Scenario: CPU figures at any clock state
- **WHEN** the session and the baseline both measured at P2 and a CPU plan median rose by 50 %
- **THEN** the rise and any changed count are reported, and no GPU figure is

#### Scenario: Time columns
- **WHEN** a pipelined loop draws frames while a plan is computed on a worker
- **THEN** every frame reports plan, scheduler, upload, record, GPU and wall time as separate columns in that order, the plan column is the worker's time and a slow frame is attributed to the largest of scheduler, upload, record and GPU

#### Scenario: Upload columns
- **WHEN** a managed loop writes tiles under the upload budget
- **THEN** every frame reports `upload_bytes`, `deferred` and `node_bytes` after the scheduler counts, and `upload_bytes` never exceeds the cap except by a single tile

#### Scenario: CPU clock and power plan
- **WHEN** a streaming case ran on a host that can read them
- **THEN** the log counts hold the effective clock before and after, the console prints it with the power plan's name and GUID, an unreadable value prints `not measured`, and no clock or plan value changes any verdict or any comparison with an older log

### Requirement: TEST-019 Streaming perf scenes
The performance harness SHALL provide the scenes `dive`, `dive-fast`, `teleport-1m`, `teleport-2km`, `look-1m` and `look-2km` (`test-render --perf --scene NAME`, or `streaming` for all six in one process, which is a convenience and not the way to measure RAM; `cargo xtask perf stream NAME`, and `cargo xtask perf stream all` (also `perf stream` without a scene), which runs the six one after another, each in its own child process of the same binary, so that every scene starts cold and its RAM figure is its own), each driving the pipelined managed loop (STRM-016) on the real renderer along the camera script of TEST-018 at 60 frames per second of script time, paced by the real clock, which only paces and measures. A scene SHALL take `--store DIR`, a stored bake (the production source is 16384 cells per face edge; `--global-res` names another resolution), read through a fresh reader per scene, and SHALL refuse to run without it unless `--allow-memory-bake` is given, in which case the run reads the in-memory bake of `--global-res` cells, is labelled `NOT the production source` and its session is not valid for streaming budgets; the log SHALL record `protocol.source` (`store DIR, N cells` or `in-memory N cells ...`) beside `protocol.field`. Each scene SHALL record per frame the plan, scheduler, upload, record, GPU (not measured here, and then absent) and wall time as separate columns, the detail lag against the fresh plan of the frame's camera (TEST-020) and the count of uncovered nodes, the blank-node count, the uploaded bytes, the resident, in-flight and queued counts, the dispatched, uploaded and cancelled counts and the degraded tiles, and the allocated terrain bytes, and write them to `<scene>.frames.txt` and the column statistics to the scene's log; print the verdicts of TEST-018 in the style of the terrain scene's lines, the frame verdicts holding at any clock state and no GPU verdict being given; log the clock and P-state before, during and after the case with the lowest sample in the log's `after` slot; report the RAM as the peak working set of the process that ran the scene, labelled `scene's own peak working set (child process)` when one scene ran in the process (its own world open, bake or store cache included), and judged against 8 GiB (CON-21), the driver-reported VRAM labelled as the whole GPU (an upper bound for the scene) beside the allocator's `slots allocated of share slots` (the terrain pool's slots, its ceiling since blocks are allocated on demand, against the slots the tier's terrain share holds after the pool's fixed items, in whole blocks, with the most slots resident; OVER if the pool exceeds the share's slots and counted with the gated measures), each or `not measured`, and write the same figures to the log's counts (`process_peak_bytes`, `driver_vram_bytes`, `pool_slots`, `share_slots`, `max_resident_slots`); print one `SCENE-MEMORY` line per scene for the orchestration; record the CPU's effective clock at the start and at the end of each case (`cpu_clock_before_mhz`, `cpu_clock_after_mhz` in the counts, or `not measured` on the console; the reading is the clock at that idle moment, not during the frames) and the active power plan with its GUID (`power_plan` on the console, `power_scheme` in the log), recorded and never judged and never compared between logs; fail the run, after writing the logs, naming the scene and the frame if any frame drew a blank tile (nothing resident to show; the draw counts such nodes, it has no ids for them, so the tile ids are given only where known) or, in every scene but `dive-fast`, left a node of its fresh selection uncovered (`BLANK TILE` lines name the scene, the frame and the tile ids); and compare a valid session only with the last valid log of the same scene (each scene has its own log and baseline), source and field, never across a source change. `perf stream all` SHALL pass `--store`, `--global-res`, `--out`, `--machine-ready`, `--smoke` and `--allow-memory-bake` to every child (`--out DIR` becomes `DIR/<scene>.json`, so each scene keeps its own log and baseline), run the readiness gate in each child before that child's measured case (each scene is gated on the machine as it then is; the gate is not shared between children), not stop at a failing child, print after the last child a summary that lists all six scenes with each child's exit status, its own RAM against 8 GiB, its whole-GPU VRAM and its slots against the share, and exit non-zero if any child failed. A labelled smoke mode (`--perf-smoke --scene NAME`, `cargo xtask perf stream NAME --smoke`) SHALL run the same path on any adapter for a few frames and never be a measurement.

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
