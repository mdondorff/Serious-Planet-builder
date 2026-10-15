## MODIFIED Requirements

### Requirement: TEST-012 Terrain performance workload
The performance harness SHALL provide a terrain frame workload at 1440p over scripted views from orbit to 1 m that generates and uploads meshes once, then reports per run the GPU time (timestamp queries), CPU planning time, CPU record time, wall time and the deterministic counts (nodes, draw calls, triangles, resident vertex bytes), checks the terrain budget (median GPU time at most 8 ms, p99 frame time at most 20 ms) and the CPU planning budget (median time to plan a frame at most 8 ms per view in steady state: with the per-tile selection cache warm and the camera stepping a few centimetres per plan; the cold first plan is reported beside it), measures the machine before any measured run and refuses, listing every failed condition with its measured value, unless it is ready (ADR 0020): AC power connected, a performance power scheme, the reference adapter, no `cargo`, `rustc`, `cargo-nextest`, `planet` or `clippy-driver` process outside the harness's own process tree, mean CPU load below 15 % and mean GPU utilisation below 10 % over 10 s, GPU temperature below 70 degrees C; the owner's `--machine-ready` overrides a failed gate and the override and the failed conditions are recorded in the log (`owner_override`); after the run the session is valid only if no run reported thermal or hardware throttling (hardware slowdown, software or hardware thermal, power brake; an unparseable reason counts) in the clock samples taken at its start and end (a missing sample counts); the software power cap (throttle reason 0x4), the laptop GPU's normal governor at full load, is recorded per run (`power_cap`, and the raw reasons in the samples) and printed (`[power cap 0x4 recorded, not invalidating]`) but invalidates neither a view nor the session (ADR 0036 point 1) and the run-to-run noise of every view is at most 10 % (one retry of that view's runs), otherwise the session says so before it prints any verdict (`SESSION INVALID for budgets`, and in the log) and its verdicts do not count; before a view's runs the harness SHALL bring the GPU to its loaded state (ADR 0034, ADR 0036): it renders the view in chunks of 60 frames and reads the graphics clock after each until it reads at least the clock floor of 1,400 MHz (a named constant citing ADR 0036, whatever the P-state), up to a cap of 30 s per view (inclusive), prints and logs (`warmups`) what it did and never refuses for it; the clock state of a view (the P-state or P-states and the median graphics clock of the samples before and after the runs of its final batch) SHALL be logged per run (`gpu_valid`, `gpu_clock_state`), and a view is valid for GPU budgets only if every one of those samples exists and reads a graphics clock of at least 1,400 MHz, whatever the P-state (P2 at 1,710 MHz is valid, P0 at 1,200 MHz is not, one sample below the floor invalidates the view, a missing sample is unknown and never valid); otherwise it is NOT VALID for GPU budgets: its GPU time gets no verdict (the console prints `NOT VALID for GPU budgets (<lowest sample and the floor>)` and shows the GPU median without one), while its frame time (median and p99, judged against the frame budget as measured because that is what a user sees, ADR 0034 as accepted with the owner's change of 2026-10-15), its CPU times and counts and the session's validity are unaffected, and every view's console line prints its clock state beside the frame figures (`[clock state <state>]`); the workload SHALL run the default world (GEN-018: the eroded field over an in-memory bake of `--global-res` cells per face edge) unless `--world` names another, and the log SHALL record which field and bake it measured (`protocol.field`); it offers a labelled smoke mode that exercises the path on any adapter without being a measurement. A valid session SHALL be compared with the last valid log of the same scene, frame size and measured field (a baseline that measured another field, or recorded none, is not compared: the report says so and the session becomes the baseline), and every median (frame, GPU, CPU, plan) that rose by more than 15 % of the old median, and by at least 0.1 ms, SHALL be reported with both figures, as well as every count that changed (report only: the comparison fails nothing); the GPU time of a view SHALL be compared only with that of a baseline view that is valid for GPU budgets at equal clock state (both views valid for GPU budgets and graphics clocks within 5 % of the larger; the P-state is printed, not compared), otherwise the report says `not comparable: clock state <old> vs <new>` or that the view was not compared and why, while the frame medians, CPU times and counts are compared regardless of the clock state, a frame-median regression line carries `[clock state <old> vs <new>]` and a view whose states are unequal (or unknown: a log from before the samples) gets a line `frame medians of <view> compared across unequal clock states ...: clock state <old> vs <new>`; the baseline of the GPU figures is `terrain.last-gpu-valid.json` beside the log (the last valid session with at least one view valid for GPU budgets; the previous log stands in before it exists), compared per view; an invalid session SHALL be neither compared nor kept as the baseline (the baseline is `terrain.last-valid.json` beside the log; before it exists, the previous log stands in if it was valid). Every gate and validity rule is a pure function over measured facts. A frame loop that pipelines its work (STRM-016, ADR 0032 point 5) SHALL report, for every frame it draws, the time of the plan (the worker's time for the plan delivered since the last frame), the scheduler, the upload, the record, the GPU and the wall time as separate columns named `plan_ms`, `scheduler_ms`, `upload_ms`, `record_ms`, `gpu_ms` and `wall_ms` (a submitted frame that was not waited for has no GPU time and reports none), and a frame over budget SHALL be attributed to the largest of scheduler, upload, record and GPU; the plan runs off the frame path and is not one of them. The same loop SHALL report, after the scheduler counts, the tile bytes written that frame (`upload_bytes`), the finished tiles deferred by the per-frame upload budget (`deferred`) and the node-record bytes (`node_bytes`), so that the upload cap (STRM-006, a byte count asserted at tier A) and the upload time (a perf window) are read from the same log line. A streaming case (TEST-019) SHALL also log the CPU's effective clock before and after the case (`cpu_clock_before_mhz`, `cpu_clock_after_mhz`: the base frequency times `% Processor Performance`) and print it with the active power plan's name and GUID, as facts of the host only: they invalidate no view or session, a change in them (and in `max_resident_slots`) is not reported as a changed count, and a host that cannot read them logs nothing and prints `not measured`; a log without these counts stays readable and comparable. The validity of a session is decided in one place, a pure function over the measured facts (`planet_perf::validity`), used by the harness, the log writer, the console and `cargo xtask accept M4`, and it gives two answers: **frame validity** (frame time, CPU, plan and record times, counts, RAM, slots and lag) and **GPU validity** (`gpu_ms`, the GPU budget, GPU comparisons). A smoke run, another adapter, a power scheme that is not a performance scheme (or cannot be read), thermal or hardware throttling, run-to-run noise above the limit, a bake that is not the production source and the owner's override of the readiness gate void both; a run that starts with the GPU in P5 or lower (an idle clock, which a CPU-bound streaming scene never raises) and a view's clock sample below the floor void the GPU verdicts only (the session's, or the view's own), never a frame verdict (ADR 0034 as accepted); the software power cap voids neither. The console prints the answers once, before any verdict, in agreeing words: `SESSION VALID for budgets: frame verdicts: yes; GPU verdicts: yes ...`, or `SESSION VALID for frame verdicts: yes; GPU verdicts: NOT VALID (<cause>)`, or `SESSION INVALID for budgets: frame verdicts: NO; GPU verdicts: NO ...` with the causes; the log keeps `valid_for_budgets` (true only when both hold) and adds `frame_valid`, `gpu_session_valid` and `gpu_only_causes`, and a log from before them is read through the same function from `valid_for_budgets` and its warnings (frame-valid when its only warning is the idle start); the regression report treats a frame-valid session as valid for the frame, CPU and count figures and baselines.

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

#### Scenario: Idle GPU start
- **WHEN** a streaming session passes the gate, shows no thermal throttling, and a run starts with the GPU in P8 at 210 MHz
- **THEN** the console prints one line `SESSION VALID for frame verdicts: yes; GPU verdicts: NOT VALID (...)`, the frame verdicts are given as measured, the log has `frame_valid` true, `gpu_session_valid` false and `valid_for_budgets` false, and the same run with a smoke flag, another adapter, a balanced power scheme, thermal throttling or an owner override prints `SESSION INVALID for budgets: frame verdicts: NO; GPU verdicts: NO`

### Requirement: TEST-021 Milestone M4 acceptance
`cargo xtask accept M4` SHALL run the automated steps of `accept M3` except the discrete-GPU tier C run (format, lints and layering, spec-lint, tiers A and B, tier C on the software adapter, the three run-mode smokes, the `test fast` timing step), and SHALL judge M4's hardware exit criteria only from the logs of a valid streaming perf window (`perf/logs/<scene>.json` and `<scene>.frames.txt`, or `--logs DIR`), never from a measurement of its own. The exit criteria are those of the architecture report's M4 row as amended by the owner on 2026-10-15 (ADR 0032): on the scenes `dive`, `teleport-1m`, `teleport-2km`, `look-1m` and `look-2km`, no blank tile (the draw's own count and the uncovered nodes of the fresh selection), p99 frame at most 20 ms (and the median and maximum budgets of TEST-018), VRAM within the terrain share and the tier (the allocator's slots and bytes, the driver's whole-GPU figure beside them) (the allocator's slots and bytes gate; the driver figure is a conservative whole-GPU upper bound that fails when read over the tier cap and is reported "not measured", not refused, when absent), RAM at most 8 GiB as the scene's own peak working set, and the detail lag gated for the dive descent and the still look and tracked, not gated, for the teleports and the dive's ground segment; `dive-fast` is run and reported and is not part of the verdict. The verdicts SHALL be the harness's own (`planet_perf::streaming::case_verdicts`) recomputed from the per-frame log, so the command and the scene cannot disagree. The command SHALL REFUSE, listing every reason, print the commands for the architect's window and the acceptance, write no report and exit non-zero, when: a gated scene has no log or no per-frame log; a session is invalid for frame verdicts (the readiness gate of ADR 0020, throttling per ADR 0036, a smoke run, another adapter or power scheme, an owner override, noise; thermal or hardware throttling invalidates; the software power cap, a GPU clock below the floor and a run that started with the GPU in an idle P-state do not, because those void only GPU verdicts and no M4 criterion is a GPU time; `planet_perf::validity` decides, and a log from before `frame_valid` is re-read from `valid_for_budgets` and its warnings); `protocol.source` is not a stored bake of 16384 cells per face edge (an in-memory bake is refused); `protocol.field` is not the world's eroded field over that bake; the log is old-format (no `protocol.source`, counts missing, other `frames.txt` columns); the RAM peak is missing or not the scene's own (`scene_alone` not 1); the frames are not the full script (2400 for the dive, 600 otherwise) or the log's frame statistics differ from those of `frames.txt`; or the five scenes come from different sources, fields, adapters or power schemes, or the log's upload and allocation maxima differ from those of `frames.txt`. Frame-time and count verdicts are valid at any clock state, which is printed per scene (GPU-valid or not; no GPU verdict is given, TEST-019); an earlier `.last-valid.json` is never used. A refusal is not a failure of M4. With valid logs the command SHALL write `docs/milestones/M4-report.md` (as `target/review/M4-report.generated.md` if that file already exists, never overwriting it, and never creating it when an automated step failed) holding the verdict, the criteria and tracked tables with the evidence of each scene, the clock and host facts, `dive-fast` and the automated steps, and the slots for the findings no log carries (the cold start, the tracked teleport lag and dive ground segment, the upload cap of ADR 0033 point 9, the RAM split of point 8, the prefetch look-ahead of ADR 0031 question 2, the compact against the vertex GPU ratio, `gpu_ms` in the pipelined scenes), each marked TO FILL for the architect, and SHALL exit non-zero if a gated criterion fails or an automated step fails. The thresholds are named constants citing ADR 0032, ADR 0033 and CON-21.

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

#### Scenario: A window whose sessions started at an idle GPU clock
- **WHEN** every session passed the gate and started with the GPU in P8, and the frame verdicts of the five scenes are within budget
- **THEN** the verdict is PASS, each scene's facts say `session valid for frame verdicts: yes; GPU verdicts: NOT VALID (...)`, and a window with a smoke, wrong-adapter, power-scheme or throttled session is still refused with the reason
