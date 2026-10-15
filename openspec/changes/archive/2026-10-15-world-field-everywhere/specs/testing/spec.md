## MODIFIED Requirements

### Requirement: TEST-008 Repro bundles
Every visual bug SHALL be reproducible from a bundle containing camera pose or path, the definition hash, generator version, settings and adapter information, replayed by `cargo xtask repro <bundle>` (tile scenes and terrain scenes) in every run mode that renders (`test-render` and `editor --offscreen`); a terrain bundle of a run that named a world (`--world`) SHALL also record that world (the canonical definition text, the height field id and the bake size in cells per face edge; format version 2) and replay SHALL rebuild the field from the bundle alone, refusing a recorded world that does not hash to the recorded definition hash, a field id the recorded world no longer resolves to, and a radius that differs from the request's; a bundle without a world (every bundle of a seed-only run, and every version 1 bundle) is the reference field of its seed and still replays; replay SHALL refuse a version mismatch, SHALL refuse a definition hash this build cannot rebuild (without a recorded world the single planet of the recorded seed and radius is the only definition it can) and a missing (zero) hash, each with the recorded and rebuilt hashes, and SHALL report any difference between the rebuilt and the recorded plan in numbers.

Verify: A
Status: active
Source: CON-19, ADR 0006, M4 item 7d

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

### Requirement: TEST-011 Count-based budgets
Draw calls, triangles, resident tiles and upload bytes SHALL be derived from the plan and asserted against budgets in tier A tests on any machine, and tier C tests SHALL check that the renderer drew and uploaded exactly those counts (allocated VRAM joins them when the renderer allocates budgeted pools); the counts of each standard view (the scripted cameras at the production parameters) SHALL also be pinned per view, over the default world (the eroded field over a named bake) and over the reference field the goldens keep, so that a change that raises the nodes, triangles, resident tiles or upload bytes of any view fails with the view, the count and the pin, and a pin is raised only by a change that says why; time-based budgets SHALL be measured only on the reference machine.

Verify: A
Status: active
Source: CON-20, M4 item 7d

#### Scenario: Budget exceeded
- **WHEN** a frame plan exceeds its draw-call budget
- **THEN** the test fails and prints the count and the budget

#### Scenario: Pinned view
- **WHEN** the plan of a standard view has more nodes than its pin
- **THEN** the test fails naming the view, the count and the pin

#### Scenario: Renderer matches the plan
- **WHEN** a terrain plan is rendered
- **THEN** the draw calls, triangles and uploaded vertex bytes the renderer reports equal the counts derived from the plan

#### Scenario: Default world pins
- **WHEN** the ten standard views are planned over the default world (eroded field, 256-cell bake)
- **THEN** each view's nodes, triangles, resident tiles and upload bytes stay within its pin, and the test names the bake the pins were made over

### Requirement: TEST-012 Terrain performance workload
The performance harness SHALL provide a terrain frame workload at 1440p over scripted views from orbit to 1 m that generates and uploads meshes once, then reports per run the GPU time (timestamp queries), CPU planning time, CPU record time, wall time and the deterministic counts (nodes, draw calls, triangles, resident vertex bytes), checks the terrain budget (median GPU time at most 8 ms, p99 frame time at most 20 ms) and the CPU planning budget (median time to plan a frame at most 8 ms per view in steady state: with the per-tile selection cache warm and the camera stepping a few centimetres per plan; the cold first plan is reported beside it), measures the machine before any measured run and refuses, listing every failed condition with its measured value, unless it is ready (ADR 0020): AC power connected, a performance power scheme, the reference adapter, no `cargo`, `rustc`, `cargo-nextest`, `planet` or `clippy-driver` process outside the harness's own process tree, mean CPU load below 15 % and mean GPU utilisation below 10 % over 10 s, GPU temperature below 70 degrees C; the owner's `--machine-ready` overrides a failed gate and the override and the failed conditions are recorded in the log (`owner_override`); after the run the session is valid only if no run reported GPU throttling in the clock samples taken at its start and end (a missing sample counts) and the run-to-run noise of every view is at most 10 % (one retry of that view's runs), otherwise the session says so before it prints any verdict (`SESSION INVALID for budgets`, and in the log) and its verdicts do not count; the workload SHALL run the default world (GEN-018: the eroded field over an in-memory bake of `--global-res` cells per face edge) unless `--world` names another, and the log SHALL record which field and bake it measured (`protocol.field`); it offers a labelled smoke mode that exercises the path on any adapter without being a measurement. A valid session SHALL be compared with the last valid log of the same scene, frame size and measured field (a baseline that measured another field, or recorded none, is not compared: the report says so and the session becomes the baseline), and every median (frame, GPU, CPU, plan) that rose by more than 15 % of the old median, and by at least 0.1 ms, SHALL be reported with both figures, as well as every count that changed (report only: the comparison fails nothing); an invalid session SHALL be neither compared nor kept as the baseline (the baseline is `terrain.last-valid.json` beside the log; before it exists, the previous log stands in if it was valid). Every gate and validity rule is a pure function over measured facts.

Verify: C
Status: active
Source: CON-20, CON-21, M4 item 7d

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
