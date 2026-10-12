# testing Specification

## Purpose
TBD - created by archiving change m0-foundations. Update Purpose after archive.

## Requirements

### Requirement: TEST-001 Spec traceability
Every requirement in the specs SHALL have a unique trace ID, a verification tier (A, B, C or D) and a status (active, planned or deprecated); every `// spec: ID` citation in the code SHALL name an existing requirement or constitution rule; every active tier A, B or C requirement SHALL be cited by at least one test of its own tier, where a citation counts only when the `// spec:` comment sits directly above a `#[test]` function (only comments and attributes, including wrapped multi-line ones, may sit between; only the `#[test]` attribute counts) and integration test files `tests/gpu_*.rs` are tier C, `tests/oracle_*.rs` tier B and all others (including unit tests under `src/`) tier A, matching the test runner; every tier D requirement SHALL name its review artefact.

Verify: A
Status: active
Source: CON-16, ADR 0009, ADR 0012

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
Every visual bug SHALL be reproducible from a bundle containing camera pose or path, the definition hash, generator version, settings and adapter information, replayed by `cargo xtask repro <bundle>` (tile scenes and terrain scenes) in every run mode that renders (`test-render` and `editor --offscreen`); replay SHALL refuse a version mismatch, SHALL refuse a definition hash this build cannot rebuild (the single planet of the recorded seed and radius is the only definition it can) and a missing (zero) hash, each with the recorded and rebuilt hashes, and SHALL report any difference between the rebuilt and the recorded plan in numbers.

Verify: A
Status: active
Source: CON-19, ADR 0006

#### Scenario: Replay
- **WHEN** a bundle is replayed
- **THEN** the same FramePlan is produced and the run reports any difference in numbers

#### Scenario: Foreign definition
- **WHEN** a bundle records a definition hash this build cannot rebuild, or none
- **THEN** replay refuses it with the hashes and what to do, before replanning

#### Scenario: Terrain replay
- **WHEN** a terrain bundle recorded with a seed is replayed with another seed, another generator or mesh version, or a tampered plan
- **THEN** the replay refuses a version mismatch, rejects a corrupt bundle by its plan hash, and reports the differing plan lines (for example mesh keys) in numbers; an unchanged bundle replays to the recorded pixels

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
The performance harness SHALL provide a terrain frame workload at 1440p over scripted views from orbit to 1 m that generates and uploads meshes once, then reports per run the GPU time (timestamp queries), CPU planning time, CPU record time, wall time and the deterministic counts (nodes, draw calls, triangles, resident vertex bytes), checks the terrain budget (median GPU time at most 8 ms, p99 frame time at most 20 ms) and the CPU planning budget (median time to plan a frame at most 8 ms per view in steady state: with the per-tile selection cache warm and the camera stepping a few centimetres per plan; the cold first plan is reported beside it), measures the machine before any measured run and refuses, listing every failed condition with its measured value, unless it is ready (ADR 0020): AC power connected, a performance power scheme, the reference adapter, no `cargo`, `rustc`, `cargo-nextest`, `planet` or `clippy-driver` process outside the harness's own process tree, mean CPU load below 15 % and mean GPU utilisation below 10 % over 10 s, GPU temperature below 70 degrees C; the owner's `--machine-ready` overrides a failed gate and the override and the failed conditions are recorded in the log (`owner_override`); after the run the session is valid only if no run reported GPU throttling in the clock samples taken at its start and end (a missing sample counts) and the run-to-run noise of every view is at most 10 % (one retry of that view's runs), otherwise the session says so before it prints any verdict (`SESSION INVALID for budgets`, and in the log) and its verdicts do not count; it offers a labelled smoke mode that exercises the path on any adapter without being a measurement. A valid session SHALL be compared with the last valid log of the same scene and frame size, and every median (frame, GPU, CPU, plan) that rose by more than 15 % of the old median, and by at least 0.1 ms, SHALL be reported with both figures, as well as every count that changed (report only: the comparison fails nothing); an invalid session SHALL be neither compared nor kept as the baseline (the baseline is `terrain.last-valid.json` beside the log; before it exists, the previous log stands in if it was valid). Every gate and validity rule is a pure function over measured facts.

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
Golden tests SHALL check every golden of a test before failing and write every candidate of the run. `cargo xtask golden-review [--proposal <file>] [--extra <dir>]... [--apply]` SHALL compare every candidate with its golden on every adapter, write a golden | candidate | diff triptych and `report.md` to `target/review/golden-review/`, and classify each candidate under ADR 0017: auto-bless rounding noise (a debug or shaded view, at most 32 changed pixels, max delta 1) and declared changes (listed under `Expected golden changes:` in the change's proposal; for debug views also on at least two adapters, with changed pixels agreeing within the larger of 5 % and 8 pixels and max deltas within the larger of 5 % and 1), and escalate everything else (a changed driver, lit views, first goldens, new adapters, disagreement, a size change on any adapter, undeclared changes, unclassified goldens) with a reason. `--apply` SHALL copy only the auto verdicts and never touch `driver.txt`. The test's own oracles, open review findings and earlier owner reverts are checked by the agent applying the `golden-review` skill.

Verify: A
Status: active
Source: CON-18, ADR 0009, ADR 0017

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
