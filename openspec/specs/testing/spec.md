# testing Specification

## Purpose
How the project proves itself: spec traceability and the A/B/C/D verification tiers (CON-16, CON-17, ADR 0012), per-adapter goldens, image metrics and the golden review policy (CON-18, ADRs 0009, 0017), actionable failures and repro bundles (CON-19), count-based budgets and the performance protocol on the reference machine (CON-20, CON-21, ADR 0020), and the flight and stress scenes used for the real-GPU review.

## Requirements

### Requirement: TEST-001 Spec traceability
Every requirement in the specs SHALL have a unique trace ID, a verification tier (A, B, C or D) and a status (active, planned or deprecated); every `// spec: ID` citation in the code SHALL name an existing requirement or constitution rule; every active tier A, B or C requirement SHALL be cited by at least one test of its own tier, where a citation counts only when the `// spec:` comment sits directly above a `#[test]` function (only comments and attributes, including wrapped multi-line ones, may sit between; only the `#[test]` attribute counts) and integration test files `tests/gpu_*.rs` are tier C, `tests/oracle_*.rs` tier B and all others (including unit tests under `src/`) tier A, matching the test runner, and a `// spec:` comment directly above a performance-harness scene entry point (a `fn perf_*` item under `crates/app/src/`, with the same comments-and-attributes rule between) counts as tier C evidence only (CON-17: the scene measures what no test may; there is no further tier); every tier D requirement SHALL name its review artefact; and every main spec SHALL have a Purpose section that is not empty and does not contain the placeholder "TBD" (in any case).

Verify: A
Status: active
Source: CON-16, ADR 0009, ADR 0012

#### Scenario: Placeholder Purpose
- **WHEN** a spec's Purpose is missing, empty or contains "TBD"
- **THEN** `cargo xtask spec-lint` fails and names the spec

#### Scenario: Uncited active requirement
- **WHEN** an active tier A requirement has no citing test
- **THEN** `cargo xtask spec-lint` fails and names the requirement and the comment to add

#### Scenario: Unknown citation
- **WHEN** a test cites an ID that is not in the specs or the constitution
- **THEN** `cargo xtask spec-lint` fails and names the file and the ID

#### Scenario: Citation not on a test
- **WHEN** a `// spec:` comment sits above a helper function, a constant or a module instead of a `#[test]` function
- **THEN** it does not count towards coverage and the requirement is reported as not cited

#### Scenario: Wrong tier
- **WHEN** an active tier C requirement is cited only by tests outside `gpu_*` files
- **THEN** `cargo xtask spec-lint` fails and says which tiers cite it and which tier is required

#### Scenario: Perf scene citation
- **WHEN** a `// spec:` comment sits directly above a `fn perf_*` item under `crates/app/src/`
- **THEN** it counts as tier C evidence of the requirement (and of no other tier); above any other function, in another crate or under `tests/` it covers nothing

### Requirement: TEST-002 Per-adapter goldens and candidates
Golden images SHALL be stored per adapter family under `tests/goldens/<adapter>/` (the key drops driver and compiler versions, e.g. `vulkan-llvmpipe`), with the full driver string recorded in `driver.txt` beside them and printed on every mismatch; a test SHALL never create or overwrite a golden; when a golden is missing the test SHALL write a candidate to `target/review/candidates/<adapter>/` and report it as pending (failing in strict mode, and always when the adapter already has blessed goldens) unless its name is listed in `tests/goldens/pending.txt`; an adapter that runs where `PLANET_REQUIRE_ADAPTER_GOLDENS=1` (CI) and has no golden directory SHALL fail; debug views SHALL compare exactly. Candidates are promoted only by the owner's `/bless` or under the golden review policy (TEST-016, ADR 0017).

Verify: A
Status: active
Source: CON-18, ADR 0009, ADR 0017

#### Scenario: Missing golden
- **WHEN** a golden test runs without a golden for the adapter
- **THEN** a candidate PNG is written, no file appears under `tests/goldens/`, and the test output says `PENDING BLESS`

#### Scenario: Strict mode
- **WHEN** `PLANET_GOLDEN_STRICT=1` and a golden is missing
- **THEN** the test fails

#### Scenario: Blessed adapter
- **WHEN** a golden is missing for an adapter that already has at least one blessed golden
- **THEN** the test fails even without strict mode

#### Scenario: Waiting for blessing
- **WHEN** a golden is missing but its name is listed in `tests/goldens/pending.txt`
- **THEN** the test reports it as pending and passes, and the line is removed when the owner blesses the golden

#### Scenario: Adapter family key
- **WHEN** two llvmpipe adapters differ only in the LLVM version or bit width
- **THEN** they share one golden directory key, and a mismatch message shows both the current and the recorded driver string

#### Scenario: No golden directory in CI
- **WHEN** `PLANET_REQUIRE_ADAPTER_GOLDENS=1` and the running adapter's key has no golden directory
- **THEN** the golden test fails and says the key or driver family may have changed

### Requirement: TEST-003 Image metrics
The test kit SHALL provide metrics that hold without a golden, including exact pixel counts, so that a hole (background colour inside the rendered object) or a stray colour is detectable.

Verify: A
Status: active
Source: CON-18, report §16

#### Scenario: Hole counting
- **WHEN** an image contains two pixels of the hole colour
- **THEN** the count of that colour is exactly two

### Requirement: TEST-004 Headless render on the software adapter
A flat-colour triangle SHALL render headless on the software adapter to a 256 by 256 image in which the corner is the clear colour, the centre is the fill colour, every pixel is one of the two, and the filled area is within 1 % of the analytic area.

Verify: C
Status: active
Source: CON-18, ADR 0009

#### Scenario: Hello triangle
- **WHEN** the hello-triangle test runs on WARP or lavapipe
- **THEN** all metrics hold and the image is compared with the adapter's golden (or written as a candidate)

### Requirement: TEST-005 Reference-adapter check
The performance harness SHALL refuse to measure on any adapter that is not a discrete GPU named as the reference GPU, and the refusal message SHALL say what to change.

Verify: A
Status: active
Source: CON-21, report §8

#### Scenario: Integrated or software adapter
- **WHEN** the harness is started and the adapter is integrated, software or another discrete GPU
- **THEN** it exits with an error naming the adapter and the corrective action, and no frame is measured

### Requirement: TEST-006 Performance protocol
The performance harness SHALL discard warm-up samples, report median and 99th percentile (nearest rank) per run, reject non-finite samples, log GPU clocks, power, temperature, throttle reasons and the active power scheme, record the owner's override of the readiness gate (`owner_override`, empty when the gate passed), and report run-to-run noise as the relative spread of run medians, and SHALL mark a session invalid for time budgets (with the reasons in the log and on the console) when the power scheme is not a performance scheme or the GPU was in a low performance state when a run started.

Verify: A
Status: active
Source: CON-21, report §8

#### Scenario: Statistics
- **WHEN** samples 1 to 100 follow two warm-up outliers
- **THEN** the median is 50.5, the p99 is 99 and the outliers are ignored

#### Scenario: Balanced power profile
- **WHEN** a session ran under the Balanced power scheme or started with the GPU idle
- **THEN** the log contains `valid_for_budgets` false and a warning naming the scheme or the performance state

#### Scenario: Log content
- **WHEN** a run report is serialised
- **THEN** it contains the confirmation flag, median, clocks before and after, and the protocol parameters

### Requirement: TEST-007 Actionable failures
Failure output SHALL contain numeric deltas and file paths, and image mismatches SHALL write the candidate and a diff image to `target/review/`.

Verify: A
Status: active
Source: CLAUDE.md, report §16

#### Scenario: One differing pixel
- **WHEN** one pixel differs by 5 in one channel
- **THEN** the report states 1 differing pixel, a maximum channel delta of 5, its coordinates, and the diff image path

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

### Requirement: TEST-009 FramePlan snapshots
A FramePlan and a TerrainPlan SHALL each be serialisable to a stable text form that can be snapshot-tested, diffed between versions and stored in a repro bundle; the terrain text form SHALL carry every field of the plan (size, view, cells, the f64 camera, view-projection, clear colour, height range and per node tile, level, origin, camera-relative origin, morph values, colour and mesh key) so that a change to any one of them changes the text and its hash.

Verify: A
Status: active
Source: CON-03, ADR 0009

#### Scenario: Round trip
- **WHEN** a FramePlan is serialised and read back
- **THEN** it is equal to the original

#### Scenario: Every terrain plan field
- **WHEN** any single field of a terrain plan is changed (down to one ulp of an f64 or f32 value)
- **THEN** its text form and plan hash change, and the diff names the difference

### Requirement: TEST-010 Debug view per visual feature
Every visual feature SHALL ship with a flat-colour debug view and at least one tier-C test; debug views SHALL be independent of lighting and compare exactly. The terrain `relief` view (REND-006) is the one exception to flat colour: it uses a fixed sun so that frames stay legible at every altitude, and it still compares exactly.

Verify: C
Status: active
Source: CON-18

#### Scenario: Face view
- **WHEN** the face debug view is rendered
- **THEN** the six faces appear in six distinct flat colours with no background pixels inside the planet disc (checked from the LOD change in M2; M1 checks the two-tile case below)

#### Scenario: Tile views
- **WHEN** the face, tile-id and height views of two generated tiles are rendered
- **THEN** no clear-colour pixel appears inside a planned rectangle, the face view is exactly the face colour, and the images match the adapter's goldens (or are written as candidates)

### Requirement: TEST-011 Count-based budgets
Draw calls, triangles, resident tiles and upload bytes SHALL be derived from the plan and asserted against budgets in tier A tests on any machine, and tier C tests SHALL check that the renderer drew exactly the plan's draw calls and triangles and uploaded exactly the bytes of the tiles in the format they were built in (the plan's upload bytes are the vertex-format reference count; a compact tile uploads its slot (header, climate block and texels, GEN-025), levels 0-2 and fallback tiles their vertices, and the product never uploads more than the reference) (allocated VRAM joins them when the renderer allocates budgeted pools); the counts of each standard view (the scripted cameras at the production parameters, including the production finest level, LOD-010) SHALL also be pinned per view, over the default world (the eroded field over a named bake) and over the reference field the goldens keep, so that a change that raises the nodes, triangles, resident tiles or upload bytes of any view fails with the view, the count and the pin, and a pin is raised only by a change that says why; the ground-1m counts at the reference finest level 26 SHALL stay pinned beside (SPIKE-07 stays reproducible); time-based budgets SHALL be measured only on the reference machine.

Verify: A
Status: active
Source: CON-20, M4 item 7d, LOD-010, GEN-025, ADR 0062

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
- **WHEN** the ten standard views are planned over the default world (eroded field, 256-cell bake) at the production finest level (22)
- **THEN** each view's nodes, triangles, resident tiles and upload bytes stay within its pin, the test names the bake the pins were made over, and ground-1m at level 26 equals its reference pin

#### Scenario: Compact upload bytes
- **WHEN** a compact tile is uploaded
- **THEN** the bytes counted are its slot (9,168 B at cells 32, GEN-025), and the pinned upload bytes of the standard views are re-pinned with that change and its reason

### Requirement: TEST-012 Terrain performance workload
The performance harness SHALL provide a terrain frame workload at 1440p over scripted views from orbit to 1 m that generates and uploads meshes once, then reports per run the GPU time (timestamp queries), CPU planning time, CPU record time, wall time and the deterministic counts (nodes, draw calls, triangles, resident vertex bytes), checks the terrain budget (median GPU time at most 8 ms, p99 frame time at most 20 ms) and the CPU planning budget (median time to plan a frame at most 8 ms per view in steady state: with the per-tile selection cache warm and the camera stepping a few centimetres per plan; the cold first plan is reported beside it), measures the machine before any measured run and refuses, listing every failed condition with its measured value, unless it is ready (ADR 0020): AC power connected, a performance power scheme, the reference adapter, no `cargo`, `rustc`, `cargo-nextest`, `planet` or `clippy-driver` process outside the harness's own process tree, mean CPU load below 15 % and mean GPU utilisation below 10 % over 10 s, GPU temperature below 70 degrees C; the owner's `--machine-ready` overrides a failed gate and the override and the failed conditions are recorded in the log (`owner_override`); after the run the session is valid only if no run reported thermal or hardware throttling (hardware slowdown, software or hardware thermal, power brake; an unparseable reason counts) in the clock samples taken at its start and end (a missing sample counts); the software power cap (throttle reason 0x4), the laptop GPU's normal governor at full load, is recorded per run (`power_cap`, and the raw reasons in the samples) and printed (`[power cap 0x4 recorded, not invalidating]`) but invalidates neither a view nor the session (ADR 0036 point 1) and the run-to-run noise of every view is at most 10 % (one retry of that view's runs), otherwise the session says so before it prints any verdict (`SESSION INVALID for budgets`, and in the log) and its verdicts do not count; before a view's runs the harness SHALL bring the GPU to its loaded state (ADR 0034, ADR 0036): it renders the view in chunks of 60 frames and reads the graphics clock after each until it reads at least the clock floor of 1,400 MHz (a named constant citing ADR 0036, whatever the P-state), up to a cap of 30 s per view (inclusive), prints and logs (`warmups`) what it did and never refuses for it; the clock state of a view (the P-state or P-states and the median graphics clock of the samples before and after the runs of its final batch) SHALL be logged per run (`gpu_valid`, `gpu_clock_state`), and a view is valid for GPU budgets only if every one of those samples exists and reads a graphics clock of at least 1,400 MHz, whatever the P-state (P2 at 1,710 MHz is valid, P0 at 1,200 MHz is not, one sample below the floor invalidates the view, a missing sample is unknown and never valid); otherwise it is NOT VALID for GPU budgets: its GPU time gets no verdict (the console prints `NOT VALID for GPU budgets (<lowest sample and the floor>)` and shows the GPU median without one), while its frame time (median and p99, judged against the frame budget as measured because that is what a user sees, ADR 0034 as accepted with the owner's change of 2026-10-15), its CPU times and counts and the session's validity are unaffected, and every view's console line prints its clock state beside the frame figures (`[clock state <state>]`); the workload SHALL run the default world (GEN-018: the eroded field over an in-memory bake of `--global-res` cells per face edge) unless `--world` names another, and the log SHALL record which field and bake it measured (`protocol.field`); it offers a labelled smoke mode that exercises the path on any adapter without being a measurement. A valid session SHALL be compared with the last valid log of the same scene, frame size and measured field (a baseline that measured another field, or recorded none, is not compared: the report says so and the session becomes the baseline), and every median (frame, GPU, CPU, plan) that rose by more than 15 % of the old median, and by at least 0.1 ms, SHALL be reported with both figures, as well as every count that changed (report only: the comparison fails nothing); the GPU time of a view SHALL be compared only with that of a baseline view that is valid for GPU budgets at equal clock state (both views valid for GPU budgets and graphics clocks within 5 % of the larger; the P-state is printed, not compared), otherwise the report says `not comparable: clock state <old> vs <new>` or that the view was not compared and why, while the frame medians, CPU times and counts are compared regardless of the clock state, a frame-median regression line carries `[clock state <old> vs <new>]` and a view whose states are unequal (or unknown: a log from before the samples) gets a line `frame medians of <view> compared across unequal clock states ...: clock state <old> vs <new>`; the baseline of the GPU figures is `terrain.last-gpu-valid.json` beside the log (the last valid session with at least one view valid for GPU budgets; the previous log stands in before it exists), compared per view; an invalid session SHALL be neither compared nor kept as the baseline (the baseline is `terrain.last-valid.json` beside the log; before it exists, the previous log stands in if it was valid). Every gate and validity rule is a pure function over measured facts. A frame loop that pipelines its work (STRM-016, ADR 0032 point 5) SHALL report, for every frame it draws, the time of the plan (the worker's time for the plan delivered since the last frame), the scheduler, the upload, the record, the GPU and the wall time as separate columns named `plan_ms`, `scheduler_ms`, `upload_ms`, `record_ms`, `gpu_ms` and `wall_ms` (a submitted frame that was not waited for has no GPU time and reports none), and a frame over budget SHALL be attributed to the largest of scheduler, upload, record and GPU; the plan runs off the frame path and is not one of them. The same loop SHALL report, after the scheduler counts, the tile bytes written that frame (`upload_bytes`), the finished tiles deferred by the per-frame upload budget (`deferred`), the node-record bytes (`node_bytes`) and the queue calls the frame's tile writes made (`upload_calls`, REND-018: each call is a staging allocation and a copy command, the cost the bytes do not show), so that the upload cap (STRM-006, a byte count asserted at tier A), the calls per frame (REND-018, a count asserted at tiers A and C) and the upload time (a perf window) are read from the same log line; the streaming cases' `frames.txt` carries `upload_calls` as the last of these columns, followed by those of TEST-031. A streaming case (TEST-019) SHALL also log the CPU's effective clock before and after the case (`cpu_clock_before_mhz`, `cpu_clock_after_mhz`: the base frequency times `% Processor Performance`) and print it with the active power plan's name and GUID, as facts of the host only: they invalidate no view or session, a change in them (and in `max_resident_slots`) is not reported as a changed count, and a host that cannot read them logs nothing and prints `not measured`; a log without these counts stays readable and comparable. A streaming session SHALL also record the tile generation pool it ran with (`protocol.gen`: `workers`, `ahead`, `priority` (`below-normal` or `normal`; a log without it ran at normal) and, when the size came from the default rule, `reserve`; STRM-020, ADR 0046), as facts only: a log without it reads, is accepted by `cargo xtask accept` and is compared as before, the comparison naming the pools beside the figures (`gen settings: baseline none recorded, this session <settings>`), and two logs that both record it with different values are not compared as equal: the line `regression check skipped: gen settings differ: <old> vs <new>` is printed and the session becomes the baseline, because the frame and upload medians depend on the threads that generate tiles beside the frame thread. The validity of a session is decided in one place, a pure function over the measured facts (`planet_perf::validity`), used by the harness, the log writer, the console and `cargo xtask accept M4`, and it gives two answers: **frame validity** (frame time, CPU, plan and record times, counts, RAM, slots and lag) and **GPU validity** (`gpu_ms`, the GPU budget, GPU comparisons). A smoke run, another adapter, a power scheme that is not a performance scheme (or cannot be read), thermal or hardware throttling, run-to-run noise above the limit, a bake that is not the production source and the owner's override of the readiness gate void both; a run that starts with the GPU in P5 or lower (an idle clock, which a CPU-bound streaming scene never raises) and a view's clock sample below the floor void the GPU verdicts only (the session's, or the view's own), never a frame verdict (ADR 0034 as accepted); the software power cap voids neither. The console prints the answers once, before any verdict, in agreeing words: `SESSION VALID for budgets: frame verdicts: yes; GPU verdicts: yes ...`, or `SESSION VALID for frame verdicts: yes; GPU verdicts: NOT VALID (<cause>)`, or `SESSION INVALID for budgets: frame verdicts: NO; GPU verdicts: NO ...` with the causes; the log keeps `valid_for_budgets` (true only when both hold) and adds `frame_valid`, `gpu_session_valid` and `gpu_only_causes`, and a log from before them is read through the same function from `valid_for_budgets` and its warnings (frame-valid when its only warning is the idle start); the regression report treats a frame-valid session as valid for the frame, CPU and count figures and baselines.

Verify: C
Status: active
Source: CON-20, CON-21, REND-018, M4 fix F10, M4 fix F5, M4 item 7d, ADR 0034, ADR 0036 points 1 and 2, ADR 0032 point 5, ADR 0033 point 9

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
- **WHEN** a run reports hardware slowdown, thermal throttling or a power brake (or an unparseable reason), or a view's run-to-run noise stays above the limit after one retry
- **THEN** the session prints `SESSION INVALID for budgets` with the reasons before any verdict and the log has `valid_for_budgets` false

#### Scenario: Noise floor (ADR 0059)
- **WHEN** the run-to-run noise `(max - min) / median` of a view's run medians is computed
- **THEN** it counts as above the limit only when it exceeds both 10 % of the median and an absolute floor: 0.5 ms for a waited frame total, 0.1 ms for a per-pass GPU figure (`gate::NOISE_FLOOR_FRAME_MS`, `NOISE_FLOOR_PASS_MS`); a spread of exactly 10 % or exactly the floor is not above; fewer than two runs or a non-positive median still counts as noisy

#### Scenario: Acceptance derives validity from the logged data
- **WHEN** `cargo xtask accept` reads a perf log
- **THEN** it judges again, with the harness's own functions (`gate::validity`, `gate::noise_needs_retry`, `clock::gpu_validity`), the throttling reasons from the logged throttle masks of every run, the noise reasons from the logged run medians of every view with two or more runs (the streaming scenes are not noise-checked), and each view's GPU clock validity from its logged clock samples against the current floor; the recorded flags and reasons stay in the log as a record
- **AND** it prints each place where the derived verdict differs from the recorded one (reasons recorded and not true, reasons true and not recorded, `frame_valid`, `gpu_session_valid`, a view's GPU clock verdict)
- **AND** a kind of reason whose inputs are not all in the log (a missing clock sample or throttle mask, a view whose runs are absent), a log without a `warnings` list, and a recorded invalid session without any reason keep their recorded verdict and the output says so
- **AND** causes that depend on the run alone (smoke, adapter, power scheme, source, readiness gate, owner override, idle start, load calibration) stay as recorded

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

#### Scenario: Generation settings
- **WHEN** a streaming log records `protocol.gen`, and is compared with a baseline that recorded other settings, the same settings, and none
- **THEN** other settings print `gen settings differ: <old> vs <new>`, compare nothing and make the session the baseline; the same settings compare as before; none recorded compares as before and prints the pools beside the figures; no log is refused for lacking the field

### Requirement: TEST-013 Flight sequence for the real-GPU review
`planet test-render --scene terrain --flight N --out-dir DIR` SHALL render a scripted, deterministic flight (a log-spaced dive from 20,000 km to 1 m above the terrain, then a glide at 1 m in steps of 5 cm) through the persistent terrain renderer with the production LOD defaults (cells 32, tau 6 px; overridable with `--cells`, `--tau`) at a chosen size on the adapter selected by `--adapter` or `PLANET_ADAPTER`, in the `relief` view unless `--view` names another, writing `frame_NNNN.png`, an index `flight.txt` (frame, altitude above terrain, node count, plan hash, finest level) and `contact_sheet.png` (up to 12 frames evenly spaced from the first to the last, each labelled with its frame number and altitude). Options a flight would ignore, and flight-only options outside a flight, SHALL be rejected before any GPU work.

Verify: C
Status: active
Source: CON-13, ADR 0009, ADR 0014

#### Scenario: Deterministic sequence
- **WHEN** the flight runs twice on one adapter
- **THEN** every frame and the index are identical, the index starts at 20,000 km and ends at 1 m, and consecutive glide cameras move between 2.5 and 10 cm

#### Scenario: Bad arguments
- **WHEN** fewer than 10 frames, no output directory, another scene, a conflicting option (`--perf`, `--perf-smoke`, `--repro`, `--out`, `--bundle`, `--stream`, `--tile`, `--script`, `--frames`, `--res`, `--manifest`, `--face-net`), a flight-only option without `--flight`, or a size whose LOD parameters fail validation are given
- **THEN** the run fails with an actionable message and creates no output directory

#### Scenario: Contact sheet and index
- **WHEN** a 30-frame flight is rendered at 160x120
- **THEN** `contact_sheet.png` holds 12 thumbnails in 4 columns, and every index row has five fields with the finest level growing from the first frame to the last

### Requirement: TEST-014 Real-GPU flight review
The owner SHALL review the TEST-013 flight rendered on the reference GPU once, in the default relief view, and confirm that it shows no jitter, vertex swimming, cracks or popping beyond the LOD transitions. The 1280x720 flight review is the owner's review of the relief view at milestone gates (the relief view has no goldens: a 128x96 image is too small to show a relief defect and would change with nearly every terrain change); its automatic check is a tier-B property test of the relief view at the orbit and ground-1m cameras (no clear-colour pixel below the horizon, grid and contour lines present, REND-006).

Verify: D
Status: planned
Review: `target/review/flight-relief-nvidia/` (240 frames, 1280x720, relief view, RTX 3080 Ti) is rendered and waits for the owner. The earlier `target/review/flight-nvidia/` (tile-id view) approved at the M2 gate covered the dive; its glide frames were solid colour and showed nothing.
Source: CON-13, CON-18, ADR 0014

#### Scenario: Glide at 1 m
- **WHEN** the owner steps through the glide frames at 1 m
- **THEN** the grid lines and contours of the relief view move smoothly with the camera and no vertex jumps between consecutive frames

#### Scenario: Relief flight
- **WHEN** the owner reviews the relief flight rendered on the reference GPU
- **THEN** the owner confirms or rejects it, and the verdict is recorded in `docs/status.md`

### Requirement: TEST-015 Steep-terrain stress
A test-only steep height field (`generators::stress::Steep`, behind the `stress` feature that only dev-dependencies enable: ridged, `|h| <= 8 km`, about -5.5 to +6 km in practice, 20 m slopes above 27 degrees; its own implementation id and version enter its height and mesh keys) SHALL pass the crack, morph, border and precision invariants of the reference terrain with the reference tolerances (GEN-004, REND-009, LOD-004, LOD-005, LOD-008): border positions and normals of neighbouring tiles across every cube edge, fully morphed borders on the coarser neighbour's border polyline, morph closing every level transition in the ten scripted views, deep-tile vertices within 1 mm, vertices within 0.25 px at 1 m on the leaf that selection picks under the camera, and no holes in the ten scripted views on the GPU (with the leaf level capped at 19: on WARP a full-depth view exceeds the test time limit; levels 20 to 26 are covered by the tier-A checks). The selection-dependent checks SHALL run with parameters for which a crack-free morph interval exists on this field (`viewport_h_px / tau_px` of about 17 times `cells` or more: cells 4 at 77 and cells 8 at 180 in the tests), and the plan SHALL reject production parameters (cells 32 at 240) on this field with a plan error (LOD-008). Plans with a field whose seed or height bound does not fit the request SHALL be rejected with a plan error. The reference field through the field-generic paths SHALL equal the default paths.

Verify: C
Status: active
Source: CON-11, CON-13, ADR 0003, ADR 0006, ADR 0014, ADR 0016, M2 report (SPIKE-02 caveat)

#### Scenario: Steep invariants
- **WHEN** the tier-A and tier-C checks run on the steep field
- **THEN** they pass with the reference tolerances, and the ten views are written to `target/review/steep/<adapter>/` for review

#### Scenario: Reference unchanged
- **WHEN** a mesh, height or mesh key, plan or scripted view is built through the field-generic path with the reference field
- **THEN** it equals the default path bit for bit (mesh and key compared directly; plans and views through the same snapshots and the determinism manifest of GEN-003)

#### Scenario: Stress field versioned
- **WHEN** the committed stress mesh is generated
- **THEN** its content hash equals the known answer recorded for its version

### Requirement: TEST-016 Golden review policy
Golden tests SHALL check every golden of a test before failing and write every candidate of the run. `cargo xtask golden-review [--proposal <file>] [--extra <dir>]... [--tests <junit.xml>]... [--apply]` SHALL compare every candidate with its golden on every adapter, write a golden | candidate | diff triptych and `report.md` to `target/review/golden-review/`, and classify each candidate under ADR 0017: auto-bless rounding noise (a debug or shaded view, at most 32 changed pixels, max delta 1) and declared changes (listed under `Expected golden changes:` in the change's proposal; judged per adapter, each adapter's candidate against its own golden: ADR 0061), and escalate everything else (a changed driver, lit, sky and water views, first goldens of any class, new adapters, a size change, undeclared changes, unclassified goldens) with a reason (TEST-022). `--apply` SHALL copy only the auto verdicts and never touch `driver.txt`. The test's own oracles, open review findings and earlier owner reverts are checked by the agent applying the `golden-review` skill.

Verify: A
Status: active
Source: CON-18, ADR 0009, ADR 0017, ADR 0037, ADR 0061

#### Scenario: Owner decisions reproduced
- **WHEN** the policy runs on the PR #43 candidates (WARP, llvmpipe, NVIDIA) with the change's declarations
- **THEN** every candidate the owner approved is auto-blessed, and without the declarations every real change escalates

#### Scenario: Every mismatch of a run
- **WHEN** a test checks three goldens and two mismatch
- **THEN** it fails once, naming both, and writes both candidates

### Requirement: TEST-017 Flight legibility
Every frame of a flight in its default view SHALL show structure: at least 10 % of its terrain pixels sit next to a clearly different pixel (a channel differs by at least 12). The check SHALL be shown to fail on the flat-colour `tile-id` flight, so that it cannot pass vacuously.

Verify: C
Status: active
Source: CON-13, CON-18, ADR 0014

#### Scenario: Measure
- **WHEN** a frame is measured
- **THEN** terrain pixels are those whose right and lower neighbours are not the clear colour either, and the check uses the production LOD parameters (cells 32) because the flat-colour views only turn solid when tiles are large

#### Scenario: Default view
- **WHEN** a 30-frame flight is rendered in the default view at 768x432 with cells 32
- **THEN** no frame, orbit or glide, falls below the structure threshold

#### Scenario: Negative control
- **WHEN** the same flight is rendered with `--view tile-id`
- **THEN** every frame of its 1 m glide falls below the threshold

### Requirement: TEST-018 Streaming test cases
The streaming cases of ADR 0032 SHALL be camera scripts defined once in a GPU-free crate (`planet_frame::scenes`) and shared by the tier-A fake-clock tests and the perf scenes: one camera per frame of 1/60 s of script time, a pure function of the scene, the height field and a seed, with time entering only as the frame index times the step (CON-04). The dive (`dive`, `dive-fast`, and `dive-cold`, which is the dive script itself run over a reader that ignores the tile pack's pre-filled coarse tiles: TEST-019, ADR 0045 point 3) SHALL lower the altitude above the terrain by a constant factor per unit time from 20,000 km to 1 m in 30 s (equal time per decade, 7.3 decades) and then travel north on the ground at 1 m for 10 s at 1.5 m/s (`dive`) or 30 m/s (`dive-fast`), 40 s in all, looking along the direction of travel (pitch a function of time only). The teleport (`teleport-1m`, `teleport-2km`) SHALL hold a view at 2 km until it is settled (the residency set resident, nothing queued, in flight or waiting, for one second) and jump to the antipodal point at 1 m or 2 km altitude for 10 s. Mouse look (`look-1m`, `look-2km`) SHALL hold a position at 1 m or 2 km, settled, for 10 s of seeded yaw and pitch steps with a yaw reversal at every segment and a final 180 degree yaw turn within 0.2 s, from a self-contained integer generator (SplitMix64) identical on every platform. The detail lag of a frame SHALL be judged against the plan selected afresh for that frame's own camera with every tile resident (the plan the synchronous path makes for the camera; the stale-plan rule of the drawing is no part of the definition): the area-weighted mean, over the nodes of that fresh selection, of the node's level (the target) minus the drawn level, weighted by the node's projected pixels (the footprint of its tile edge at its distance from the camera, capped at the screen), where the drawn level of a node is the level of the deepest tile that is resident and drawn in that frame and is the node itself or one of its ancestors; a drawn tile finer than the target is not a negative lag (a node covered only by drawn tiles below it has lag 0); a node with no drawn tile at, above or below it is uncovered, counted as its level plus one levels and reported with its tile id; an uncovered node is a blank tile (ADR 0032's hard measure is a tile, root or ancestor, for every visible node), so zero uncovered nodes in every frame is one of the hard measures below. Every frame of a case has a lag: there is no frame "without a plan" and a gated lag rule is never "not measured" for want of one; a frame whose camera pose or drawn set was not recorded is missing data and fails the case loudly (TEST-020). The hard measures of every case SHALL be: no blank tile in any frame (the draw's own count of nodes with nothing resident, and the uncovered nodes of the frame's fresh selection; the uncovered nodes of `dive-fast` and `dive-cold` are report-only, like their other criteria), frame median at most 16.6 ms, p99 at most 20 ms and no frame over 40 ms, upload at most 1 MiB per frame and upload time at most 2 ms (judged on the p99 of the frames, the maximum printed), allocated terrain bytes within the tier's share in every frame, terrain-pool VRAM within the tier cap, process peak working set within 8 GiB; the detail-lag budgets SHALL be gated for the dive descent (mean at most 1 level, no frame over 3) and for a still look (lag 0, no request, cancellation or upload) and tracked, not gated, for the teleport (under 1 level within 2 s at 1 m and 1 s at 2 km, draw set complete within 9 s and 4 s) and for the dive's ground segment at walking speed (zero within 2 s), and reported without a budget for the ground segment of `dive-fast` and for the whole of `dive-cold` (the cold start, which the pre-fill takes out of the gates: its lag, descent and ground segment, is reported, never gated). `dive-cold` is never gated: every measure of it that is over its budget is printed and reported as `over (not gated)`, but a frame that draws nothing at all (a blank node: the six roots are loaded before frame 0, so it cannot happen) still fails the run. `--prefill-levels N` is an option of the binary (a streaming scene then requires levels 0..=N pre-filled); `cargo xtask perf` runs the default 4. A budget changes only as a declared change to ADR 0032. The tier-A dive case SHALL model the generation machine, not only the clock: the tile spawner is a generation-rate model (N workers, a duration per job on the fake clock, frames of 1/60 s; `TimedSpawner`) under the default configuration of the pipelined loop (the in-flight limit and the queue ahead derived from the pool size, STRM-020), never a spawner that finishes a fixed number of jobs a frame or a limit above the pool, so that a cap or a refill that waits for the frame boundary shows as lost throughput; the dive case SHALL assert that the tiles generated per second over the frames in which requests wait reach at least 80% of N divided by the job duration (N = 8 and 20, 18 ms a job, the 1,217 tiles/s of ADR 0032 on 20 threads) and that the previous refill, with no queue ahead, does not.

Verify: A
Status: active
Source: ADR 0032 points 1-4 (accepted, owner 2026-10-15), ADR 0031 point 3, CON-04, CON-20, STRM-002, TEST-020, STRM-020, perf window 2 (2026-10-16)

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
The performance harness SHALL provide the scenes `dive`, `dive-fast`, `teleport-1m`, `teleport-2km`, `look-1m`, `look-2km` and `dive-cold` (`test-render --perf --scene NAME`, or `streaming` for all seven in one process, which is a convenience and not the way to measure RAM; `cargo xtask perf stream NAME`, and `cargo xtask perf stream all` (also `perf stream` without a scene), which runs the seven one after another, each in its own child process of the same binary, so that every scene starts cold and its RAM figure is its own), each driving the pipelined managed loop (STRM-016) on the real renderer along the camera script of TEST-018 at 60 frames per second of script time, paced by the real clock, which only paces and measures. A scene SHALL take `--store DIR`, a stored bake (the production source is 16384 cells per face edge; `--global-res` names another resolution), read through a fresh reader per scene, and SHALL refuse to run without it unless `--allow-memory-bake` is given, in which case the run reads the in-memory bake of `--global-res` cells, is labelled `NOT the production source` and its session is not valid for streaming budgets; the log SHALL record `protocol.source` (`store DIR, N cells` or `in-memory N cells ...`) beside `protocol.field`, followed by what the tile pack held at the start of the scene (STRM-021, ADR 0045 point 3): `store DIR, N cells; prefilled coarse pack (levels 0-L present at start)` when the harness checked, at the start of the scene, that every tile of levels 0 to L (`--prefill-levels L`, default 4, the owner's decision of 2026-10-16) is in the pack for the run's cells, finest level and tile form and the scene reads them, `...; no pre-filled coarse pack (P of T tiles of levels 0-L present)` when some are missing, and `...; cold: the pack's coarse tiles are ignored, every tile is generated on demand` for `dive-cold`, whose reader (the same bake, the same field) never reads a tile from the pack; the scenes `dive` and `dive-fast` SHALL refuse to start, before the readiness gate and naming the count and the command that pre-fills the store (`generate --prefill-dir`), unless the pack holds all those tiles (a smoke run only records the marker), and `dive-cold` is a scene of its own so that cold numbers stay cold; the counts carry `prefill_tiles_used`, the tiles the scene took from the pack instead of generating. Each scene SHALL record per frame the plan, scheduler, upload, record, GPU (not measured here, and then absent) and wall time as separate columns, the detail lag against the fresh plan of the frame's camera (TEST-020) and the count of uncovered nodes, the blank-node count, the uploaded bytes, the resident, in-flight and queued counts, the dispatched, uploaded and cancelled counts and the degraded tiles, and the allocated terrain bytes, and write them to `<scene>.frames.txt` and the column statistics to the scene's log; print the verdicts of TEST-018 in the style of the terrain scene's lines, the frame verdicts holding at any clock state and no GPU verdict being given; log the clock and P-state before, during and after the case with the lowest sample in the log's `after` slot; report the RAM as the peak working set of the process that ran the scene, labelled `scene's own peak working set (child process)` when one scene ran in the process (its own world open, bake or store cache included), and judged against 8 GiB (CON-21), the driver-reported VRAM labelled as the whole GPU (an upper bound for the scene) beside the allocator's `slots allocated of share slots` (the terrain pool's slots, its ceiling since blocks are allocated on demand, against the slots the tier's terrain share holds after the pool's fixed items, in whole blocks, with the most slots resident; OVER if the pool exceeds the share's slots and counted with the gated measures), each or `not measured`, and write the same figures to the log's counts (`process_peak_bytes`, `driver_vram_bytes`, `pool_slots`, `share_slots`, `max_resident_slots`, `scene_alone`: 1 when the process ran this scene alone, so that the RAM figure is known to be the scene's own: TEST-021); print one `SCENE-MEMORY` line per scene for the orchestration; record the CPU's effective clock at the start and at the end of each case (`cpu_clock_before_mhz`, `cpu_clock_after_mhz` in the counts, or `not measured` on the console; the reading is the clock at that idle moment, not during the frames) and the active power plan with its GUID (`power_plan` on the console, `power_scheme` in the log), recorded and never judged and never compared between logs; fail the run, after writing the logs, naming the scene and the frame if any frame drew a blank tile (nothing resident to show; the draw counts such nodes, it has no ids for them, so the tile ids are given only where known) or, in every scene but `dive-fast` and `dive-cold`, left a node of its fresh selection uncovered (`BLANK TILE` lines name the scene, the frame and the tile ids); and compare a valid session only with the last valid log of the same scene (each scene has its own log and baseline), source and field, never across a source change. `perf stream all` SHALL pass `--store`, `--global-res`, `--out`, `--machine-ready`, `--smoke` and `--allow-memory-bake` to every child (`--out DIR` becomes `DIR/<scene>.json`, so each scene keeps its own log and baseline), run the readiness gate in each child before that child's measured case (each scene is gated on the machine as it then is; the gate is not shared between children), not stop at a failing child, print after the last child a summary that lists all seven scenes with each child's exit status, its own RAM against 8 GiB, its whole-GPU VRAM and its slots against the share, and exit non-zero if any child failed. A labelled smoke mode (`--perf-smoke --scene NAME`, `cargo xtask perf stream NAME --smoke`) SHALL run the same path on any adapter for a few frames and never be a measurement.

Verify: A, C
Status: active
Source: ADR 0032 points 4 and 8, ADR 0034, ADR 0036, TEST-012, TEST-018, STRM-016, TEST-020

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
- **WHEN** a store pre-filled to levels 0-L is read by the scenes
- **THEN** `dive`, `dive-fast` and the others log `protocol.source` with `prefilled coarse pack (levels 0-L present at start)` and count the tiles they took from the pack, `dive-cold` logs `cold: the pack's coarse tiles are ignored` and takes none, and a store with fewer tiles makes `dive` and `dive-fast` refuse before the gate naming `P of T tiles` and `generate --prefill-dir` while `dive-cold` and the other scenes go on

#### Scenario: Smoke
- **WHEN** the seven scenes run in smoke mode on the software adapter over a stored 256-cell bake pre-filled to level 1
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

### Requirement: TEST-021 Milestone M4 acceptance
`cargo xtask accept M4` SHALL run the automated steps of `accept M3` except the discrete-GPU tier C run (format, lints and layering, spec-lint, tiers A and B, tier C on the software adapter, the three run-mode smokes, the `test fast` timing step), and SHALL judge M4's hardware exit criteria only from the logs of a valid streaming perf window (`perf/logs/<scene>.json` and `<scene>.frames.txt`, or `--logs DIR`), never from a measurement of its own. The exit criteria are those of the architecture report's M4 row as amended by the owner on 2026-10-15 (ADR 0032): on the scenes `dive`, `teleport-1m`, `teleport-2km`, `look-1m` and `look-2km`, no blank tile (the draw's own count and the uncovered nodes of the fresh selection), p99 frame at most 20 ms (and the median and maximum budgets of TEST-018), VRAM within the terrain share and the tier (the allocator's slots and bytes, the driver's whole-GPU figure beside them) (the allocator's slots and bytes gate; the driver figure is a conservative whole-GPU upper bound that fails when read over the tier cap and is reported "not measured", not refused, when absent), RAM at most 8 GiB as the scene's own peak working set, and the detail lag gated for the dive descent and the still look and tracked, not gated, for the teleports and the dive's ground segment; `dive-fast` is run and reported and is not part of the verdict. The gated dive starts with levels 0-4 pre-filled in the tile pack (owner, 2026-10-16, ADR 0045 point 3): it SHALL be judged only from a log whose `protocol.source` says `prefilled coarse pack (levels 0-N present at start)` with N at least 4, `dive-fast` is shown only from such a log (otherwise as unusable, never a refusal), and the cold start is the scene `dive-cold`, which is tracked and reported and never gated, whose log must say `cold` (a log that does not is shown as unusable) and whose numbers are listed in the report beside, never in, the verdict. The other four gated scenes are judged from logs that say pre-filled or that carry no marker (logs from before the pre-fill, which read as they always did and are reported as `not stated`: they started from an empty tile cache, which is cold). The verdicts SHALL be the harness's own (`planet_perf::streaming::case_verdicts`) recomputed from the per-frame log, so the command and the scene cannot disagree. The command SHALL REFUSE, listing every reason, print the commands for the architect's window and the acceptance, write no report and exit non-zero, when: a gated scene has no log or no per-frame log; a session is invalid for frame verdicts (the readiness gate of ADR 0020, throttling per ADR 0036, a smoke run, another adapter or power scheme, an owner override, noise; thermal or hardware throttling invalidates; the software power cap, a GPU clock below the floor and a run that started with the GPU in an idle P-state do not, because those void only GPU verdicts and no M4 criterion is a GPU time; `planet_perf::validity` decides, and a log from before `frame_valid` is re-read from `valid_for_budgets` and its warnings); `protocol.source` is not a stored bake of 16384 cells per face edge (an in-memory bake is refused; the place is `protocol.source` before its `; ` marker); the gated `dive` log does not say pre-filled to level 4 or deeper (a cold run, a pack found incomplete, a shallower pre-fill, or a log from before the pre-fill, which has no marker and so counts as cold), the refusal naming the source, what it says and the commands (`generate --prefill-dir <store> --global-res 16384`, then `cargo xtask perf stream all --store <store>`); `protocol.field` is not the world's eroded field over that bake; the log is old-format (no `protocol.source`, counts missing, other `frames.txt` columns); the RAM peak is missing or not the scene's own (`scene_alone` not 1); the frames are not the full script (2400 for the dive, 600 otherwise) or the log's frame statistics differ from those of `frames.txt`; or the five scenes come from different places (the bake read, not the pack marker), fields, adapters or power schemes, or the log's upload and allocation maxima differ from those of `frames.txt`. Frame-time and count verdicts are valid at any clock state, which is printed per scene (GPU-valid or not; no GPU verdict is given, TEST-019); an earlier `.last-valid.json` is never used. A refusal is not a failure of M4. With valid logs the command SHALL write `docs/milestones/M4-report.md` (as `target/review/M4-report.generated.md` if that file already exists, never overwriting it, and never creating it when an automated step failed) holding the verdict, the criteria and tracked tables with the evidence of each scene, the clock and host facts, `dive-fast` and the automated steps, and `dive-cold` (as the cold start, with its own rows marked not gated), the tile pack's state at the start of the gated dive, and the slots for the findings no log carries (the cold start, the tracked teleport lag and dive ground segment, the upload cap of ADR 0033 point 9, the RAM split of point 8, the prefetch look-ahead of ADR 0031 question 2, the compact against the vertex GPU ratio, `gpu_ms` in the pipelined scenes), each marked TO FILL for the architect, and SHALL exit non-zero if a gated criterion fails or an automated step fails. The thresholds are named constants citing ADR 0032, ADR 0033 and CON-21.

Verify: A
Status: active
Source: ADR 0032 (accepted, owner 2026-10-15), ADR 0033 points 1 and 8, ADR 0020, ADR 0034, ADR 0036, TEST-018, TEST-019, TEST-020, CON-21; opus-planner decisions of 2026-10-15 (M4 acceptance built before the perf window)

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

### Requirement: TEST-022 Golden review after M5: per-adapter verdicts, first goldens escalate (ADR 0061)
Goldens are per-adapter regression checks. `golden-review` SHALL judge the candidate of each adapter against that adapter's own golden: agreement between adapters (the ADR 0017 agreement of changed-pixel counts, ADR 0037 point 1(b), ADR 0052) is neither a condition to bless nor a reason to escalate, so a declared change of a debug or shaded view is blessed on each adapter that shows it, whatever the other adapters show, and rounding noise (at most 32 pixels at delta 1) is blessed as before. The first golden of any class (`debug`, `shaded`, `lit`, `sky`, `water`), on an adapter that has no golden of that name, SHALL escalate to the owner's `/bless`: the window of ADR 0037 point 1 is closed (`ADR_0037_M5_CLASSES` is empty). Existing lit, sky and water goldens still escalate (ADR 0017), except the look changes of TEST-036 (`AUTO-L`). The driver gate of ADR 0017 (a candidate from another driver than the recorded one escalates) is unchanged. A bless of a change (`AUTO-E`, `AUTO-L`; not `AUTO-R`, which blesses no change) SHALL also need the golden's declared bound tests green in the same run (ADR 0061 point 3): the proposal declares them under `Bound tests:` as `- <golden>: <test>, <test>` (named bare, `binary::name` or `crate::binary::name`), and `golden-review` SHALL find each of them green in the nextest JUnit reports of the run passed with `--tests` (a test that is missing, failed, skipped or retried is not green); a golden with no `Bound tests:` entry, a run without a `--tests` report, or a test that is not green SHALL escalate, naming it. An adapter that has goldens but delivered no candidates SHALL be reported as not run.

Verify: A
Status: active
Source: CON-18, ADR 0061 (supersedes ADR 0037 point 1 and ADR 0052; amends ADR 0017), ADR 0017, ADR 0035

#### Scenario: A first golden escalates
- **WHEN** a golden of any class has no golden on the adapter that delivered its candidate, declared or not, on one, two or three adapters
- **THEN** the verdict is `ESCALATE` with the reason `first golden`

#### Scenario: Per-adapter blessing
- **WHEN** a declared change of a debug view shows 951 changed pixels on one adapter, 40 on another and 5,000 at delta 3 on a third, or does not change on one adapter, or only one adapter delivered a candidate
- **THEN** each adapter that shows it is `AUTO-E` on its own candidate against its own golden

#### Scenario: The bound tests are enforced
- **WHEN** the same declared change of a golden is reviewed with its bound tests green, with one missing from the reports, with one failing, with no `Bound tests:` entry, and with no `--tests` report
- **THEN** only the first is `AUTO-E` (or `AUTO-L` for a look change); the others escalate naming the test, the missing declaration or the missing report; rounding noise and existing escalations are not changed by the tests

#### Scenario: Look classes unchanged
- **WHEN** an existing lit, sky or water golden changes without a look-change declaration
- **THEN** it escalates (`is a lit view`); with a pure look change it is `AUTO-L` under TEST-036

#### Scenario: Missing adapter
- **WHEN** an adapter with goldens delivered no candidates
- **THEN** the output says it did not run, and the look path (TEST-036) escalates for want of its debug-view evidence

### Requirement: TEST-023 Host load during a perf case
Every perf case (a view of the terrain scene, a streaming scene) SHALL record the machine's CPU load while it runs: a sampler on its own thread reads the cumulative CPU time of every process (the performance counters' raw class, which is the same in every locale) when the case starts, every 5 seconds and when it ends, and turns each pair of readings into the machine's busy share (percent of all logical processors, `Idle` excluded), the harness's share (its own process and its helpers `powershell`, `pwsh`, `nvidia-smi`, `wmiprvse`, `conhost`) and the competing share (the rest), with the five largest competing processes by name (instances of one name added). The log SHALL carry, in the run's `load` object, the number of samples, the mean and maximum of the machine's and the competing share, the harness's mean, the top competitors over the case (mean share over all samples, largest share, samples seen), the first and last samples (start and end) and the series; the console SHALL print one `host load` line per case and one `LOAD NOTE` line per note. A note SHALL be written when the competing share averages more than 15 % (ADR 0020's limit on the machine's load, applied to what is not the harness) and for each process of ADR 0020's busy list (`cargo`, `rustc`, `cargo-nextest`, `nextest`, `planet`, `clippy-driver`) that used 1 % or more of the machine in a sample (judged before the top five are cut); the notes are in the run's `load.notes` and in the log's top-level `load_notes`. The load is recorded only: no note, no share and no missing reading changes a validity answer, a verdict, a comparison or the acceptance of a log, and a log without `load` reads as before. A case whose load cannot be read (a smoke run, a host without the counters; the terrain views and the streaming scenes both print it) says `host load: not measured` and carries `"load":{"measured":false}` or no `load`.

Verify: A, C
Status: active
Source: M5 item 8 (status.md; tier C here is the unmeasured path of the smoke runs, the measuring sampler is read in the architect window and by an ignored live test), ADR 0020, ADR 0034, ADR 0036, M4 perf windows 1 to 5, TEST-012, TEST-019

#### Scenario: Harness, helpers and competitors
- **WHEN** two readings 10 s apart show the harness process at 25 % of an 8-processor machine, its `powershell` at 1 %, two `chrome` processes at 20 % and 10 % and `msedge` at 5 %
- **THEN** the machine's share is 61 %, the harness's 26 %, the competing 35 %, and the top list reads `chrome` 30 % then `msedge` 5 % without the harness

#### Scenario: Notes at ADR 0020's numbers
- **WHEN** a case's competing share averages 20 %, or exactly 15 %, or `rustc` used 2.5 % in one sample
- **THEN** the first and the third write a note naming the figures and ADR 0020, the second writes none, and in no case does `valid_for_budgets`, `frame_valid` or `gpu_session_valid` differ from the same log without `load`

#### Scenario: Not measured
- **WHEN** the host cannot be read, or the run is a smoke run
- **THEN** the console says `host load: not measured`, the log carries no numbers and no notes, and nothing is judged

### Requirement: TEST-024 Store check in the smoke run
A smoke run over a stored bake (`test-render --perf-smoke --scene S --store DIR`, `cargo xtask perf stream S --smoke --store DIR`) SHALL first check the store read-only and print the result before any scene: the store, `GLOBAL_VERSION` and `MESH_VERSION`, the ocean channel read at a fixed grid of 288 directions (12 latitudes by 24 longitudes) with the counts of sea and land, and the number of tiles of levels 0 to L (`--prefill-levels L`, default 4) the tile pack holds in the compact form and in the vertex form, each present tile read and decoded through the pack's checks. The run SHALL fail before any scene, naming the problem and the command that fixes it, when the store has no ocean channel, when the channel reads all land or all sea on the grid, or when the compact form lacks tiles of levels 0 to L (the vertex form is shown for information: the production run draws the compact form). The check SHALL write nothing to the store, and the smoke run is labelled as no measurement.

Verify: A, C
Status: active
Source: opus-planner request of 2026-10-17 (M5 item 8, before window 6), TEST-019, STRM-021, GEN-022, ADR 0045 point 3

#### Scenario: A complete store
- **WHEN** the smoke run reads a stored bake pre-filled to level 1 with `--prefill-levels 1`
- **THEN** it prints `store check (smoke, read-only, NOT a measurement)`, `ocean channel: present, 288 directions read, N sea and M land`, `compact form 30 of 30 present` and `store check ok`, runs its scenes, and the listing of the store directory (names, sizes, modification times) is unchanged afterwards

#### Scenario: An incomplete pack
- **WHEN** the same store is smoked with `--prefill-levels 2`
- **THEN** the run fails with `store check FAILED`, the count `of 126 compact tiles of levels 0-2`, `MESH_VERSION` and the `generate --prefill-dir` command, before any scene runs, and the store is unchanged

#### Scenario: A store without a usable ocean
- **WHEN** the channel is missing, or reads all land or all sea
- **THEN** the problems name `no ocean channel` and `generate --global-dir`, or `reads land on all 288` or `reads sea on all 288`

### Requirement: TEST-025 M5 lit perf scenes
The performance harness SHALL provide the scene `lit` (`test-render --perf --scene lit`, `cargo xtask perf lit`) that measures the lit path (the look's sun, the atmosphere, the water pass, adaptive exposure, the tone map) at 1440p over the views ground 1 m, aerial 20 km, coast 2 km, coast 3 m and orbit (the viewer's cameras at 20 N 30 E and two cameras 2 km and 3 m above the water at the first shoreline east of 30 E whose sea reaches 2 km out and land 500 m in, found by `planet_frame::litscenes::find_coast`) and over the two coastal flights (`coast-glide`: 6 m over the water from 400 m out, 228 m towards the shore; `coast-descent`: 300 m to 4 m, log-spaced, 1,200 m out closing to 500 m; 240 cameras each, `--flight-frames N`), the flights once with `water.shore_filter` at the look's default and once at the other setting (`<flight>@filter0` or `@filter1`, for inspection). It SHALL refuse a world without a coast or without the ocean channel, naming the remedy. Each held view SHALL be planned and uploaded once, drawn in waited timed frames (`render(.., true)`) after `enable_lit`, the atmosphere and the water pass were enabled and warmed with an empty water plan, the exposure was seeded at the view and the timed warm-up frames ran (REND-017: no first-use work in a measured frame; an empty water plan draws no water, so the first frame with water nodes of a streaming scene on the lit path may pay the water pass's first use, which the held views and flights absorb in their warm-up frames), brought to the GPU's loaded state (ADR 0034, ADR 0036) and measured in three runs of `--frames` frames with the run-to-run noise rule and its one retry (ADR 0020); each flight camera SHALL be planned, its new tiles uploaded, drawn once untimed and twice timed, in one run. The log of a run SHALL carry, besides the waited frame time, the metrics `cpu_ms`, `gpu_ms` (the terrain pass, as in the terrain scene), `gpu_water_ms`, `gpu_sky_view_ms`, `gpu_aerial_perspective_ms`, `gpu_sky_composite_ms`, `gpu_exposure_histogram_ms`, `gpu_exposure_adapt_ms`, `gpu_tonemap_ms`, `gpu_scene_copy_ms`, the per-frame sums `gpu_atmosphere_ms` (sky view, aerial perspective and composite) and `gpu_water_total_ms` (water pass and scene copy) and `gpu_span_ms` (REND-028), and the counts nodes, draw calls, water nodes and draws, tile format and the lit and atmosphere flags; the static tables' upload has no row (a queue write on a body's load, not a pass of a steady frame) and the console says so. A static view SHALL be judged, when it is valid for GPU figures (ADR 0036), against the owner's budgets of 2026-10-15: the median of the atmosphere sum at most 1 ms, the median of the water sum at most 2 ms, 2.2 ms for the held view coast 3 m (ADR 0060; a view with no water node says `not in view`), and the terrain pass at most 8 ms, and, at any clock state, the p99 of the waited frame at most 20 ms; the GPU span of the lit frame is printed as a reported figure (timestamps serialise the passes, so it is an upper bound) and the owner's frame median of 16.6 ms is the streaming scenes' with `--lit`; a view not valid for GPU figures prints its figures with `NOT VALID for GPU budgets (<state>)` and no verdict; a flight prints the same rows as reported (`over (not gated)` when above) and gates nothing; a flight's clock samples are taken after its warm-up. The console SHALL print per view one row of every pass's median, p99 and maximum, the host load (TEST-023), and for the atmosphere and water sums the run-to-run noise, its absolute spread and whether the proposed floor of ADR 0036 point 3 (also above 0.1 ms) would have kept the view noisy (recorded, not applied). `--views a,b,c` SHALL restrict the views (a partial log is not a window and never becomes a baseline). The scene `lit-formats` SHALL measure ground 1 m, aerial 20 km, coast 2 km and orbit in the vertex format and then in the compact format, view by view, log `lit-formats/<view>/<format>`, print the compact over vertex ratio of the median of the terrain pass, the atmosphere sum, the water sum and the GPU span, and store the ratios in the compact run's counts (`ratio_terrain_permille`, `ratio_atmosphere_permille`, `ratio_water_permille`, `ratio_span_permille`) only when both views are valid for GPU figures at equal clock state (ADR 0034 point 5, ADR 0036 point 2), else the line says `not comparable: clock state A vs B` or `not compared` and names the invalid view; it gates nothing. The console SHALL also print how many frames stand behind the atmosphere and water sums (a frame that lost a pass drops out of the median). The streaming scenes SHALL take `--lit` (`perf stream NAME|all --lit`): the same scripts, the same pipelined loop and the same verdicts of TEST-018 and TEST-019 with each frame drawn lit with the atmosphere and the water of its camera (the lit plan and the water plan inside the timed frame, the lit path warmed before the settle), the log named `<scene>-lit.json` in a log directory, the path `stream-lit/<scene>` and `protocol.field` ending in `; lit path (atmosphere, water)`, so that a lit log is never compared with a debug-path log; GPU time stays not measured in them. A smoke run (`--perf-smoke`, `perf lit --smoke`) SHALL run the same path on any adapter at 640 by 360 for the lit scenes, print the pass rows and no budget rows, run the vertex format on a software adapter and never be a measurement. `cargo xtask perf m5 --store DIR [--out DIR]` SHALL run `lit`, `lit-formats` and the seven streaming scenes with `--lit` in that order, each under the readiness gate and the streaming scenes each in their own process, continue after a failing part and exit non-zero if one failed.

Verify: A, C
Status: active
Source: M5 item 8 (status.md), the owner's M5 decisions of 2026-10-15 (atmosphere 1 ms, water 2 ms, frame 16.6 ms and 20 ms), ADR 0060 (coast 3 m water 2.2 ms), ADR 0034, ADR 0036, ADR 0041, ADR 0053, ADR 0020, REND-017, REND-028, TEST-012, TEST-019, TEST-023

#### Scenario: Budget rows of a static view
- **WHEN** a view valid for GPU figures has an atmosphere sum of exactly 1 ms and a water sum of exactly 2 ms, and another has 1.5 ms and 2.5 ms
- **THEN** the first is within both budgets and the second fails the atmosphere and water rows, and the GPU span row is reported in both

#### Scenario: The water budget of coast 3 m
- **WHEN** the held view coast 3 m has a water sum of exactly 2.2 ms, of 2.21 ms, and another held view has 2.1 ms (all valid for GPU figures)
- **THEN** coast 3 m is within its budget at 2.2 ms and over it at 2.21 ms, the other view fails its 2 ms budget, and the row prints the budget it was judged against

#### Scenario: A view below the clock floor
- **WHEN** a view's clock samples read 1,150 MHz in P2 and its waited frame p99 is 31 ms
- **THEN** every GPU row prints its figure with `NOT VALID for GPU budgets (P2 1150 MHz ...)` and no verdict, and the frame row is OVER at that clock state

#### Scenario: Water and atmosphere not in the view, and flights
- **WHEN** a view has no water node, or the atmosphere is not drawn, and a flight is over a budget
- **THEN** the row says `not in view` and fails nothing, and the flight's row says `over (not gated)`

#### Scenario: The coast and the flights
- **WHEN** a world has sea on the western half of a parallel and land on the eastern half
- **THEN** the coast is the first sea-to-land crossing east of 30 E with the heading towards the land, the glide holds 6 m over the water and the descent runs from 300 m to 4 m with any frame count, and a world without a shoreline, or with a pond whose sea does not reach 2 km out, has no coast

#### Scenario: Smoke on the software adapter
- **WHEN** the scenes `lit` and `lit-formats` and a streaming scene with `--lit` run in smoke mode over an in-memory bake
- **THEN** every view and both flights with both shore-filter settings print a pass row and a waited frame time, the log carries the per-pass metrics where the adapter times its passes, the lit scenes print no budget row, the streaming log is `stream-lit/<scene>` with the lit field, and `--views` with an unknown name, `--views` or `--flight-frames` with another scene and `--lit` with the terrain scene are refused

### Requirement: TEST-026 Milestone M5 acceptance
`cargo xtask accept M5` SHALL run the automated steps of `accept M4` (format, lints and layering, spec-lint, tiers A and B, tier C on the software adapter, the three run-mode smokes, the `test fast` timing step; the discrete-GPU tier C run stays on the manual list so that acceptance may sit beside the architect's window) and SHALL judge M5's hardware criteria only from the logs of a valid M5 window (`lit.json`, `lit-formats.json`, and `<scene>-lit.json` with `<scene>-lit.frames.txt` for the seven streaming scenes, in `perf/logs/` or `--logs DIR`: what `cargo xtask perf m5` writes), never from a measurement of its own. The gated rows SHALL be, for each of the held views ground 1 m, aerial 20 km, coast 2 km, coast 3 m and orbit of `lit.json` (the median of the runs' medians, the largest p99): the atmosphere (sky view, aerial perspective and composite summed per frame) at most 1 ms, the water (pass and scene copy) at most 2 ms (2.2 ms for the view coast 3 m, ADR 0060), the terrain pass at most 8 ms, each only when the view is valid for GPU figures (ADR 0036) and the session fit for GPU verdicts, and the p99 of the waited frame at most 20 ms at any clock state; and, for the scenes `dive`, `teleport-1m`, `teleport-2km`, `look-1m` and `look-2km` on the lit path, every verdict of TEST-021 recomputed by the harness's own functions from the per-frame logs. The report-only streaming logs (`dive-fast`, `dive-cold`) are shown only when they come from the source, adapter and power scheme of `lit.json`. The terrain pass's 8 ms is M2's budget carried to the lit path (ADR 0034 point 4); the p99 of the waited frame is judged at any clock state (ADR 0034 as amended by the owner, 2026-10-15). The coastal flights (both shore-filter settings), the compact against vertex ratios, the GPU span of a waited frame, `dive-fast` and `dive-cold` on the lit path and the host load SHALL be reported and never gated. The outcome SHALL be PASS, FAIL (a gated row over its budget) or INCOMPLETE (no row failed but a gated GPU row is not valid for GPU budgets or not measured, or a coast view has no water node in some run), and only PASS exits zero. The command SHALL REFUSE, listing every reason, print the commands of the window and the acceptance, write no report and exit non-zero, when a gated log is missing, a session is invalid for frame verdicts (the readiness gate, throttling, smoke, another adapter or power scheme, the owner's override, noise: `planet_perf::validity`), the frame size is not 2560 by 1440, the source is not the stored production bake, `protocol.field` of `lit.json`, `lit-formats.json` or a streaming log is not the production field drawn on the lit path with compact tiles (each log states its tile format), a held view has fewer than three runs (a partial log, `--views`, is not a window), a streaming log is old-format, partial or not one scene's own process, the gated dive does not say pre-filled to level 4 or deeper, or the logs come from different sources, adapters or power schemes. With valid logs it SHALL write `docs/milestones/M5-report.md` (as `target/review/M5-report.generated.md` when that file exists or an automated step failed) holding the verdict, the gated and the reported rows, the per-pass table, the facts of the window and a slot marked TO FILL for every finding no log carries (the look review of ADR 0037, the M4 budgets on the lit path, the orbit atmosphere cost, the water pass, the compact against vertex ratio, the per-pass costs, ADR 0036 point 3, the shore filter in motion, the store regeneration, the host load, the proposed ADRs, the ADR 0037 window, the viewer's manual checks, the recommendation).

Verify: A
Status: active
Source: M5 item 8 and item 10 (status.md), the owner's M5 budgets of 2026-10-15, ADR 0034, ADR 0036, ADR 0037, TEST-021, TEST-025

#### Scenario: A valid window
- **WHEN** the logs come from one valid session over the production store with every gated row within its budget
- **THEN** the outcome is PASS, the report is rendered with its gated, reported and per-pass sections, and every finding slot is marked TO FILL

#### Scenario: A row over its budget
- **WHEN** the orbit view's atmosphere median is 1.3 ms, or a streaming scene on the lit path has a frame median above 16.6 ms
- **THEN** the outcome is FAIL naming the view or scene and the measure, and the flights over a budget change nothing

#### Scenario: Rows that cannot be judged
- **WHEN** a view's clock samples read 1,150 MHz, or the session started with the GPU in P8, or a coast view has no water node
- **THEN** the outcome is INCOMPLETE naming each row (`NOT VALID for GPU budgets (...)` or `a coast view must hold water`), the frame rows are still judged, and the command exits non-zero

#### Scenario: Logs that cannot be trusted
- **WHEN** a session is throttled, the frame size is 640 by 360, the bake is in memory, the tiles are vertex (in `lit.json` or in a streaming log), a view has one run, a view is missing, a stream's source differs, or `lit.json` and a gated stream log are missing
- **THEN** the command refuses with every reason, the commands of the window and `cargo xtask accept M5`, writes nothing and exits non-zero

#### Scenario: Optional parts
- **WHEN** `lit-formats`, the flights, `dive-fast` and `dive-cold` are absent
- **THEN** the window is judged and the report says each was not run

### Requirement: TEST-027 Flight capture for the look review
`cargo xtask capture flights` (and `test-render --scene capture-flights --out-dir DIR`, with `--store DIR` or `--allow-memory-bake`, `--size WxH` default 2560x1440 and `--flight-frames N` default 240) SHALL draw the two coastal flights of TEST-025 on the lit path, each with `water.shore_filter` at the look's default and at the other setting, and write every frame as `DIR/<flight>-filter<k>/frame_NNNN.png` with an index `capture.txt` (directory, frames, size, shore filter); `cargo xtask capture flights` SHALL then encode each sequence at 60 frames per second as `<name>.mp4` (H.264, yuv420p) when `ffmpeg` runs, and otherwise say that the PNG sequences stand. The default directory is `target/review/m5-flights`. The capture is a tool beside the harness and not a measurement: it takes no timestamp query, applies no gate, writes no perf log, prints no budget and is not to be run during a perf window (a rule of the project, not enforced by the tool); the world needs the ocean channel and a coast, and a missing output directory or world source is refused by the run before it draws.

Verify: A, C
Status: active
Source: M5 item 10 (status.md: the 1440p flight videos of the glide and the descent with shore_filter 0 and 1 for the owner's look review, ADR 0037), TEST-025, ATMO-017

#### Scenario: Frames per flight and filter
- **WHEN** the capture runs on the software adapter over an in-memory bake at 320 by 180 with two cameras per flight
- **THEN** four directories `coast-glide-filter1`, `coast-glide-filter0`, `coast-descent-filter1` and `coast-descent-filter0` each hold `frame_0000.png` and `frame_0001.png` of 320 by 180, `capture.txt` lists them, and no perf log or budget is written

#### Scenario: Shore flights and close stills
- **WHEN** the capture runs (all flights by default, or the ones named by `--views A,B`; an unknown name is refused)
- **THEN** beside the two coastal flights it draws the capture-only flights `shore-run-30m` (20 s at 60 frames per second: 30 m above the water level, 150 m seaward of the waterline, which it follows over 500 m of beach from the terrain-height crossings, looking along the shore 25 degrees toward the land, pitched 8 degrees down) and `shore-approach-3m` (15 s: 3 m above the level, from 1 km out straight at the beach, ending 40 m past the waterline over the sand where it keeps 2 m above the ground), and four one-frame stills at eye heights above the higher of the terrain and the water: 0.5 m above the wet sand at the waterline looking along the shore, 0.5 m above the water 20 m out looking at the beach, 0.2 m above the sand at the waterline looking seaward, 0.1 m above the water looking along the shore; `--flight-frames N` replaces the frame counts of the flights
- **AND** the stills set the camera directly, below the viewer's 1 m ground clamp (VIEW-004), and the output and `capture.txt` say so; a one-frame entry is a PNG and gets no video; the perf scenes' flights (`coast-glide`, `coast-descent`) are unchanged

#### Scenario: Command lines
- **WHEN** `capture flights` is given a store, a frame count and the smoke flag
- **THEN** the cargo arguments run the capture scene (on the hardware adapter without the flag, on the software adapter with it), carry no perf or gate option and end with the output directory, and the `ffmpeg` arguments read the sequence at 60 frames per second and write H.264 in yuv420p

#### Scenario: Refusals
- **WHEN** no output directory, or neither a store nor `--allow-memory-bake`, is given, or the sub-command is not `flights`
- **THEN** the run refuses before any work and names what is missing

### Requirement: TEST-028 CPU settle between the children of perf m5
`cargo xtask perf m5` SHALL, after every child except the last, poll the competing CPU load (TEST-032: the mean over one readiness-gate window, ADR 0020) until a reading is below the gate's limit, at most 30 polls. It SHALL print one line per wait: the child, the last reading, the limit, the polls, `settled` or `NOT settled`. An unreadable load ends the wait. The next gate decides.

Verify: A
Status: active
Source: window 6 (three gate refusals after heavy children), ADR 0020, TEST-019

#### Scenario: Settling between children
- **WHEN** a session of three children runs with a settle between children
- **THEN** the settle is called after the first and the second child and not after the third, polls until a reading is below 15 %, stops at the cap or at an unreadable load, and its line says which

### Requirement: TEST-029 Held clock before the lit views
The warm-load phase of the lit views and flights (TEST-025; it replaces the stop condition of the one-reading warm-up of TEST-012 for them, the terrain scene and the streaming scenes keep it) SHALL render chunks of at least 1 s and 60 frames and end after 3 consecutive readings of the graphics clock at the floor of ADR 0036, a reading below it restarting the count, capped at 30 s. The outcome SHALL name the readings held.

Verify: A
Status: active
Source: window 6 (coast-2km, coast-3m and the flights GPU-invalid), ADR 0034, ADR 0036, TEST-012, TEST-025

#### Scenario: Held clock
- **WHEN** the readings are 1702, 1327, 1450, 1420, 1410 MHz
- **THEN** the warm-up with 3 readings ends after the fifth, and with readings that alternate above and below the floor it ends at the cap, not reached

### Requirement: TEST-030 Log path of a single scene
`--out` of a single scene (`perf stream <scene>`, `perf lit`, `perf lit-formats`) SHALL take a directory (existing, or a path without an extension) and write `<scene>[-lit].json` inside it, or a path ending in `.json` as the file. Any other value SHALL be refused by the option check before the case runs, naming the file name the directory would get.

Verify: A
Status: active
Source: window 6 (single-scene `--out DIR`), TEST-019, TEST-025

#### Scenario: Out path
- **WHEN** `--out` is an existing directory, a new path without extension, a `.json` path, or an existing `.txt` file, for a streaming scene and for a lit scene
- **THEN** the log path is `<dir>/<scene>-lit.json`, the same under the new directory, the file itself, and a refusal by the option check before the case

### Requirement: TEST-031 Water plan counters in the streaming frames log
The `frames.txt` of a streaming case SHALL end with the columns `queries`, `tile_misses`, `band_queries` and `lit_plan_ms`: the field queries, the tiles asked of the tile jobs' water book that had no entry, the coastal-band queries and the milliseconds of the lit path's water plan on the frame thread, per frame (all 0 on the debug path). They are recorded only: no verdict, count digest or comparison reads them. `WaterTileCache::counters()` gives the cumulative counts; a frame's share is the difference of two snapshots. A log without the columns is refused as old-format.

Verify: A
Status: active
Source: M5 item 9a.1 (status.md), TEST-012, TEST-021

#### Scenario: The columns round-trip
- **WHEN** a frames log with counter values is written and read back
- **THEN** the values are equal and the header ends `queries tile_misses band_queries lit_plan_ms`

#### Scenario: A memoised plan asks nothing
- **WHEN** the same terrain plan is planned twice with one cache
- **THEN** the second plan's counter difference is 0, 0, 0

### Requirement: TEST-035 Streaming smoke without a perf window
A tier-B test SHALL run the real streamer and pool through a short scripted dive (48 frames, `test-render --perf-smoke --scene dive`) from an empty cache over a small in-memory bake on the software adapter, with an explicit number of generation workers, and SHALL read the run's log for findings that name a configuration cap: the most jobs that ran at once below a third of the workers (at least two), fewer than 3 tile jobs finished or 3 resident slots at the peak (the streamer is alive; the counts depend on the machine, 187 jobs on a development machine and 5 on a 4-core CI runner), a pool below 23,109 slots, a median of the scheduler step above 0.5 ms or of the upload step above 1.6 ms (about a tenth, and ten times, the first measurements on the software adapter, 2026-10-17). It is not a performance verdict, and it never replaces a perf window. A run of the same dive with one worker where six are expected SHALL produce a finding that names the concurrency. Each run takes a few seconds.

Verify: B
Status: active
Source: M5 item 9 (owner, 2026-10-16), the M4 configuration cap, TEST-019

#### Scenario: A healthy dive
- **WHEN** the dive runs with 6 workers
- **THEN** there are no findings

#### Scenario: An injected cap
- **WHEN** the dive runs with 1 worker and the check expects 6
- **THEN** a finding says that `load_running_max` is below what 6 workers allow

#### Scenario: Each check on a fixture
- **WHEN** a log carries a pool of 20,000 slots, 1 job, 1 resident slot, a scheduler median of 0.6 ms and an upload median of 2 ms
- **THEN** one finding names each of them, and values exactly at the limits pass

### Requirement: TEST-036 Look changes after the M5 review
`golden-review` SHALL bless the changed candidates of existing goldens of the look classes (`lit`, `sky`, `water`; `LOOK_CLASSES`) as `AUTO-L` (ADR 0037 point 2) when the change's proposal declares look parameter changes under `Look changes:` (`- <look data file>: <key> = <old> -> <new>; affects=<class>[, <class>]` per line) and all of these hold: (1) the change touches only look data (a file of `tests/goldens/look_data.txt` as the base branch has it) and inert paths (`docs/`, `openspec/`, `tests/goldens/`), every added line of a look data file is exactly `key = value` of a declaration's new value and every removed line exactly `key = value` of its old value (a longer key or value, or a second change in a comment, is not), and each declaration is found in the diff; (2) no debug or shaded golden changed beyond ADR 0017's rounding on any adapter, and every adapter with goldens took part in the run (an adapter that is NOT RUN leaves its debug views unchecked); (3) the golden is listed under `Expected golden changes:`, is in a class a declaration affects, and has a changed candidate on every adapter that has it, at least two. Any other candidate escalates with the failed condition named. The driver gate, ADR 0017's own rules, the per-adapter rules and the first-golden escalation of ADR 0061 are unchanged.

Verify: A
Status: active
Source: ADR 0037 point 2, ADR 0017, ADR 0035, M5 queue item 10

#### Scenario: A pure look change
- **WHEN** the proposal declares `shore_filter = 1 -> 0; affects=water, lit`, only the look file changed by that line, no debug view moved, and a declared lit golden changed on three adapters
- **THEN** every adapter's candidate is `AUTO-L`

#### Scenario: A look change plus code
- **WHEN** the same change also edits a Rust or shader file, or a look data line that no declaration names
- **THEN** the candidates escalate and the reason names the file or the line

#### Scenario: A line that only looks declared
- **WHEN** the changed line is `shore_filter = 10`, `max_shore_filter = 0` or `sun_lux = 9 # shore_filter = 0` for a declared `shore_filter = 1 -> 0`
- **THEN** the candidates escalate

#### Scenario: A debug view moved
- **WHEN** a debug golden changed by 33 pixels, or by delta 2, on one adapter
- **THEN** the candidates escalate; 32 pixels at delta 1 do not

#### Scenario: A golden outside the affected classes
- **WHEN** a water golden changes but the declarations affect only `sky`
- **THEN** it escalates

### Requirement: TEST-037 Streaming scenes start cool and take no GPU rows
Before the measured span of a streaming scene (not in a smoke run) the harness SHALL apply the cool start of TEST-033. There is no warm-up: the loop is paced to 60 Hz and CPU-bound, so no amount of rendering it brings the GPU to the loaded clock (look-2km at 58 C stayed at P3, 1,087 MHz, after 30 s unpaced), the held views and flights are already unpaced back-to-back waited frames, and the streaming scenes take no GPU time for the clock to matter to. A streaming scene (`stream/<scene>`, `stream-lit/<scene>`) SHALL NOT make a session invalid for GPU verdicts by the clock it started at (it has none): the clock stays recorded in the log, the session line reads `no GPU rows (the pipelined scenes take no GPU time)`, and the acceptance facts of a scene say the same; logs written before this rule, which carry the idle-start warning, are still read as valid for frame verdicts. A view that takes GPU rows keeps ADR 0034's idle start rule. The camera scripts, the budgets and the streaming code are unchanged, and the frame verdicts too.

Verify: A
Status: active
Source: perf windows 7 and 8 (every streaming scene started at P5 or P8; the lit dive's two device losses came after long runs on a hot GPU), the look-2km diagnostic run of 2026-10-18, ADR 0057, TEST-033

#### Scenario: Which steps a scene takes
- **WHEN** a smoke run and a measured streaming scene are started
- **THEN** the smoke run takes no step and the measured scene cools down; no scene warms up

#### Scenario: An idle start of a streaming scene
- **WHEN** a streaming scene began at P8, 210 MHz
- **THEN** the session is valid for frame verdicts and for GPU rows (none exist), its line says `no GPU rows`, and the warnings are empty; the same start of a lit view still voids the session's GPU verdicts

### Requirement: TEST-033 Cool start of a view
Before the warm-up of every view (the terrain scene's and the lit scenes', not in a smoke run) the harness SHALL wait, idle, until the GPU temperature reads 60 C or less, polling every 5 s for at most 120 s, and SHALL print one line per view: the temperatures at the start and the end and the seconds, or `NOT cooled` with the cap. An unreadable sensor ends the wait at once. The wait adds nothing to a measured frame.

Verify: A
Status: active
Source: perf window 7 (the clock follows the GPU temperature under the software power cap), ADR 0057 point 1, TEST-012, TEST-029

#### Scenario: Cooling
- **WHEN** the temperature reads 74, 69, 64 and 59 C on four polls
- **THEN** the wait ends after three pauses (15 s) as cooled, and a GPU that stays at 72 C ends at the cap as `NOT cooled`

#### Scenario: Cool already, or no sensor
- **WHEN** the first reading is 55 C, or the sensor gives nothing
- **THEN** the wait ends at once, cooled in the first case

### Requirement: TEST-034 A pass below the clock floor
A view whose clock samples all read P0 and whose only throttle reason is the software power cap (0x4, with at most idle, applications-clock and sync-boost bits beside it; at least one sample reports it) SHALL be valid for GPU figures whatever its clock (ADR 0060), in the harness and in the re-derivation of `accept` (`clock::gpu_validity` over the logged samples and their throttle masks, one implementation). A view whose clock samples were all read, some below the 1,400 MHz floor of ADR 0036, in any other state SHALL be judged one-sidedly on its GPU rows (ADR 0057): a figure within its budget is a pass (`ok at a low clock`, with the state), a figure over its budget is inconclusive (`NOT VALID for GPU budgets`, not a failure, and the acceptance outcome is INCOMPLETE). A view with a missing or unreadable clock sample stays not valid, a view at or above the floor is judged as before, and comparisons between views still need both at or above the floor. The acceptance report SHALL print the clock state, the temperature range, the power and the software power cap reading beside the GPU rows of every view. The rule applies to the lit rows of the M5 scenes (TEST-025, TEST-026); the M4 terrain scene keeps its console wording (TEST-012).

Verify: A
Status: active
Source: perf window 7, ADR 0057 point 2, ADR 0060, ADR 0036 point 2, TEST-025, TEST-026

#### Scenario: Within budget at a low clock
- **WHEN** a view read 1,282 MHz and its atmosphere, water and terrain medians are within their budgets
- **THEN** its GPU rows are `ok at a low clock (P0 1282 MHz, below the 1400 MHz floor ...)` and it fails nothing

#### Scenario: Over budget at a low clock
- **WHEN** the same view's atmosphere median is 1.5 ms
- **THEN** that row is `NOT VALID for GPU budgets (over budget at a clock below the floor, inconclusive ...)`, no row fails, and the window is INCOMPLETE

#### Scenario: A power-capped P0 view is valid (ADR 0060)
- **WHEN** a view's samples all read P0 at 1,245 to 1,365 MHz with throttle mask 0x4 only
- **THEN** it is valid for GPU figures, a figure over its budget is OVER (not inconclusive), and the same samples with a thermal, hardware-slowdown or power-brake bit beside the cap, with an unreadable mask, in P2 or in mixed P-states, or with no throttle reason at all, keep the verdict of ADR 0057 (below the floor); a log whose samples carry no masks keeps it too

#### Scenario: Acceptance re-derives it
- **WHEN** `accept M5` reads a log whose run recorded `gpu_valid` false at P0 1,290 MHz with the power cap
- **THEN** it derives the view valid from the logged samples, prints that the verdict differs from the recorded one, and judges the rows with it

#### Scenario: At a valid clock
- **WHEN** a view's samples are all at or above the floor
- **THEN** a figure within its budget is ok and one over it is OVER, as before

#### Scenario: The log's words
- **WHEN** a run of a log has `gpu_valid` false and `gpu_clock_state` `P0 1282 MHz, below the 1400 MHz floor`
- **THEN** acceptance reads it as below the floor at P0 1282 MHz, and `no clock sample` stays not valid

### Requirement: TEST-032 One CPU-load reading for the gate and the settle poll
The CPU load of the readiness gate (ADR 0020, limit 15 %) and of the settle poll of `perf m5` (TEST-028) SHALL be one reading: the competing load, the share of the whole machine that processes other than the reading process and the helpers it starts to read the machine (`powershell`, `nvidia-smi`, `wmiprvse`, `conhost`) used over the 10 s window, from two readings of the raw per-process CPU times (TEST-023). The gate's failure line SHALL say `competing load`. The readiness gate's other conditions are unchanged.

Verify: A
Status: active
Source: window 7 (the settle poll read 8-14 % where the gate refused at 15.5-18.5 %), ADR 0020, TEST-023, TEST-028

#### Scenario: The harness start is not load
- **WHEN** a freshly started harness process uses 30 % of the machine and a browser uses 6 %
- **THEN** the gate's reading is 6 %

#### Scenario: The previous child's tail is load for the poll
- **WHEN** the settle poll of the parent reads the same interval
- **THEN** it counts the child as a competitor and reads 36 %, and a quiet machine reads 0 %
