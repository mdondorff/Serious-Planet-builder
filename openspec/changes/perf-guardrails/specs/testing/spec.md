## MODIFIED Requirements

### Requirement: TEST-011 Count-based budgets
Draw calls, triangles, resident tiles and upload bytes SHALL be derived from the plan and asserted against budgets in tier A tests on any machine, and tier C tests SHALL check that the renderer drew and uploaded exactly those counts (allocated VRAM joins them when the renderer allocates budgeted pools); the counts of each standard view (the scripted cameras at the production parameters) SHALL also be pinned per view, so that a change that raises the nodes, triangles, resident tiles or upload bytes of any view fails with the view, the count and the pin, and a pin is raised only by a change that says why; time-based budgets SHALL be measured only on the reference machine.

Verify: A
Status: active
Source: CON-20

#### Scenario: Budget exceeded
- **WHEN** a frame plan exceeds its draw-call budget
- **THEN** the test fails and prints the count and the budget

#### Scenario: Pinned view
- **WHEN** the plan of a standard view has more nodes than its pin
- **THEN** the test fails naming the view, the count and the pin

#### Scenario: Renderer matches the plan
- **WHEN** a terrain plan is rendered
- **THEN** the draw calls, triangles and uploaded vertex bytes the renderer reports equal the counts derived from the plan

### Requirement: TEST-012 Terrain performance workload
The performance harness SHALL provide a terrain frame workload at 1440p over scripted views from orbit to 1 m that generates and uploads meshes once, then reports per run the GPU time (timestamp queries), CPU planning time, CPU record time, wall time and the deterministic counts (nodes, draw calls, triangles, resident vertex bytes), checks the terrain budget (median GPU time at most 8 ms, p99 frame time at most 20 ms) and the CPU planning budget (median time to plan a frame at most 8 ms per view in steady state: with the per-tile selection cache warm and the camera stepping a few centimetres per plan; the cold first plan is reported beside it), measures the machine before any measured run and refuses, listing every failed condition with its measured value, unless it is ready (ADR 0020): AC power connected, a performance power scheme, the reference adapter, no `cargo`, `rustc`, `cargo-nextest`, `planet` or `clippy-driver` process outside the harness's own process tree, mean CPU load below 15 % and mean GPU utilisation below 10 % over 10 s, GPU temperature below 70 degrees C; the owner's `--machine-ready` overrides a failed gate and the override and the failed conditions are recorded in the log (`owner_override`); after the run the session is valid only if no run reported GPU throttling in the clock samples taken at its start and end (a missing sample counts) and the run-to-run noise of every view is at most 10 % (one retry of that view's runs), otherwise the session says so before it prints any verdict (`SESSION INVALID for budgets`, and in the log) and its verdicts do not count; it offers a labelled smoke mode that exercises the path on any adapter without being a measurement. A valid session SHALL be compared with the last valid log of the same scene and frame size, and every median (frame, GPU, CPU, plan) that rose by more than 15 % of the old median, and by at least 0.1 ms, SHALL be reported with both figures, as well as every count that changed (report only: the comparison fails nothing); an invalid session SHALL be neither compared nor kept as the baseline. Every gate and validity rule is a pure function over measured facts.

Verify: C
Status: active
Source: CON-20, CON-21

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

#### Scenario: Regression report
- **WHEN** a valid session's ground-1m GPU median is 4.2 ms and the last valid log's is 3.0 ms
- **THEN** the console reports the path, metric, both medians and the rise (+40 %) against the 15 % limit, and a rise of 14 %, or of less than 0.1 ms, is not reported

#### Scenario: No baseline
- **WHEN** the session is invalid, the earlier log is invalid or unreadable, or it was measured at another frame size
- **THEN** nothing is compared and the console says why; only a valid session replaces the baseline

#### Scenario: Session validity
- **WHEN** a run reports throttling (software power cap, hardware slowdown, thermal or power brake) or a view's run-to-run noise stays above 10 % after one retry
- **THEN** the session prints `SESSION INVALID for budgets` with the reasons before any verdict and the log has `valid_for_budgets` false
