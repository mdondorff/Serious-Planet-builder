## MODIFIED Requirements

### Requirement: TEST-012 Terrain performance workload
The performance harness SHALL provide a terrain frame workload at 1440p over scripted views from orbit to 1 m that generates and uploads meshes once, then reports per run the GPU time (timestamp queries), CPU planning time, CPU record time, wall time and the deterministic counts (nodes, draw calls, triangles, resident vertex bytes), checks the terrain budget (median GPU time at most 8 ms, p99 frame time at most 20 ms) and the CPU planning budget (median time to plan a frame at most 8 ms per view in steady state: with the per-tile selection cache warm and the camera stepping a few centimetres per plan; the cold first plan is reported beside it), measures the machine before any measured run and refuses, listing every failed condition with its measured value, unless it is ready (ADR 0020): AC power connected, a performance power scheme, the reference adapter, no `cargo`, `rustc`, `cargo-nextest`, `planet` or `clippy-driver` process outside the harness's own process tree, mean CPU load below 15 % and mean GPU utilisation below 10 % over 10 s, GPU temperature below 70 degrees C; the owner's `--machine-ready` overrides a failed gate and the override and the failed conditions are recorded in the log (`owner_override`); after the run the session is valid only if no run reported GPU throttling in the clock samples taken at its start and end (a missing sample counts) and the run-to-run noise of every view is at most 10 % (one retry of that view's runs), otherwise the session says so before it prints any verdict (`SESSION INVALID for budgets`, and in the log) and its verdicts do not count; before a view's runs the harness SHALL bring the GPU to its loaded state (ADR 0034): it renders the view in chunks of 60 frames and reads the P-state after each until it reads P0, up to a cap of 30 s per view (inclusive), prints and logs (`warmups`) what it did and never refuses for it; the clock state of a view (the P-state and the median graphics clock of the samples before and after the runs of its final batch) SHALL be logged per run (`gpu_valid`, `gpu_clock_state`), and a view whose samples are not all P0 (a state other than P0, mixed states or a missing sample) is NOT VALID for GPU budgets: its GPU time and frame wall time get no verdict (the console prints `NOT VALID for GPU budgets (<state>)`), while its CPU times and counts and the session's validity are unaffected; the workload SHALL run the default world (GEN-018: the eroded field over an in-memory bake of `--global-res` cells per face edge) unless `--world` names another, and the log SHALL record which field and bake it measured (`protocol.field`); it offers a labelled smoke mode that exercises the path on any adapter without being a measurement. A valid session SHALL be compared with the last valid log of the same scene, frame size and measured field (a baseline that measured another field, or recorded none, is not compared: the report says so and the session becomes the baseline), and every median (frame, GPU, CPU, plan) that rose by more than 15 % of the old median, and by at least 0.1 ms, SHALL be reported with both figures, as well as every count that changed (report only: the comparison fails nothing); the GPU time and frame wall time of a view SHALL be compared only with those of a baseline view that is valid for GPU budgets at equal clock state (the same P-state and graphics clocks within 5 % of the larger), otherwise the report says `not comparable: clock state <old> vs <new>` or that the view was not compared and why, while CPU times and counts are compared regardless; the baseline of the GPU figures is `terrain.last-gpu-valid.json` beside the log (the last valid session with at least one view valid for GPU budgets; the previous log stands in before it exists), compared per view; an invalid session SHALL be neither compared nor kept as the baseline (the baseline is `terrain.last-valid.json` beside the log; before it exists, the previous log stands in if it was valid). Every gate and validity rule is a pure function over measured facts. A frame loop that pipelines its work (STRM-016, ADR 0032 point 5) SHALL report, for every frame it draws, the time of the plan (the worker's time for the plan delivered since the last frame), the scheduler, the upload, the record, the GPU and the wall time as separate columns named `plan_ms`, `scheduler_ms`, `upload_ms`, `record_ms`, `gpu_ms` and `wall_ms` (a submitted frame that was not waited for has no GPU time and reports none), and a frame over budget SHALL be attributed to the largest of scheduler, upload, record and GPU; the plan runs off the frame path and is not one of them.

Verify: C
Status: active
Source: CON-20, CON-21, M4 item 7d, ADR 0034, ADR 0032 point 5

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
- **WHEN** a run reports throttling (software power cap, hardware slowdown, thermal or power brake) or a view's run-to-run noise stays above 10 % after one retry
- **THEN** the session prints `SESSION INVALID for budgets` with the reasons before any verdict and the log has `valid_for_budgets` false

#### Scenario: Warm-up to the loaded state
- **WHEN** a view is warmed up and the P-state reads P2, P1, then P0 (or P2 until the 30 s cap)
- **THEN** the warm-up stops at the first P0 reading, a P0 reading at exactly the cap counts, a cap that is hit leaves the run going with the last reading printed and logged, and a sampler that returns nothing never counts as P0

#### Scenario: View outside P0
- **WHEN** the samples of a view's runs read P2 (or P0 and P2 mixed, or one is missing)
- **THEN** the log has `gpu_valid` false for its runs with the state as reason, the console prints `NOT VALID for GPU budgets (P2 1150 MHz)` instead of the GPU and frame verdicts, and the session stays valid for budgets

#### Scenario: Equal clock state
- **WHEN** a P0 view at 1395 MHz is compared with a P0 view at 1350 MHz, with one at 1200 MHz, and with a P2 view at 1150 MHz
- **THEN** the first pair is compared, the others are reported as `not comparable: clock state P0 1395 MHz vs P0 1200 MHz` (and the P2 view as not valid for GPU budgets), and no regression is reported for them

#### Scenario: CPU figures at any clock state
- **WHEN** the session and the baseline both measured at P2 and a CPU plan median rose by 50 %
- **THEN** the rise and any changed count are reported, and no GPU figure is

#### Scenario: Time columns
- **WHEN** a pipelined loop draws frames while a plan is computed on a worker
- **THEN** every frame reports plan, scheduler, upload, record, GPU and wall time as separate columns in that order, the plan column is the worker's time and a slow frame is attributed to the largest of scheduler, upload, record and GPU
