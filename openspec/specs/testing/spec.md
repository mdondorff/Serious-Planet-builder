# testing Specification

## Purpose
TBD - created by archiving change m0-foundations. Update Purpose after archive.

## Requirements

### Requirement: TEST-001 Spec traceability
Every requirement in the specs SHALL have a unique trace ID, a verification tier (A, B, C or D) and a status (active, planned or deprecated); every `// spec: ID` citation in the code SHALL name an existing requirement or constitution rule; every active tier A, B or C requirement SHALL be cited by at least one test; every tier D requirement SHALL name its review artefact.

Verify: A
Status: active
Source: CON-16, ADR 0012

#### Scenario: Uncited active requirement
- **WHEN** an active tier A requirement has no citing test
- **THEN** `cargo xtask spec-lint` fails and names the requirement and the comment to add

#### Scenario: Unknown citation
- **WHEN** a test cites an ID that is not in the specs or the constitution
- **THEN** `cargo xtask spec-lint` fails and names the file and the ID

### Requirement: TEST-002 Per-adapter goldens, owner-only blessing
Golden images SHALL be stored per adapter under `tests/goldens/<adapter>/`; a test SHALL never create or overwrite a golden; when a golden is missing the test SHALL write a candidate to `target/review/candidates/<adapter>/` and report it as pending (failing in strict mode, and always when the adapter already has blessed goldens) unless its name is listed in `tests/goldens/pending.txt`; debug views SHALL compare exactly.

Verify: A
Status: active
Source: CON-18, ADR 0009

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
The performance harness SHALL discard warm-up samples, report median and 99th percentile (nearest rank) per run, reject non-finite samples, log GPU clocks, power, temperature, throttle reasons and the active power scheme, record the owner's machine-ready confirmation, and report run-to-run noise as the relative spread of run medians, and SHALL mark a session invalid for time budgets (with the reasons in the log and on the console) when the power scheme is not a performance scheme or the GPU was in a low performance state when a run started.

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
Every visual bug SHALL be reproducible from a bundle containing camera pose or path, definition hash (zero until the WorldDefinition exists in M3), generator version, settings and adapter information, replayed by `cargo xtask repro <bundle>` in every run mode that renders (`test-render` and `editor --offscreen`); replay SHALL refuse a version mismatch and report any difference between the rebuilt and the recorded plan in numbers.

Verify: A
Status: active
Source: CON-19

#### Scenario: Replay
- **WHEN** a bundle is replayed
- **THEN** the same FramePlan is produced and the run reports any difference in numbers

### Requirement: TEST-009 FramePlan snapshots
A FramePlan SHALL be serialisable to a stable text form that can be snapshot-tested, diffed between versions and stored in a repro bundle.

Verify: A
Status: active
Source: CON-03, ADR 0009

#### Scenario: Round trip
- **WHEN** a FramePlan is serialised and read back
- **THEN** it is equal to the original

### Requirement: TEST-010 Debug view per visual feature
Every visual feature SHALL ship with a flat-colour debug view and at least one tier-C test; debug views SHALL be independent of lighting and compare exactly.

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
Draw calls, triangles, resident tiles, upload bytes and allocated VRAM SHALL be asserted in tests on any adapter; time-based budgets SHALL be measured only on the reference machine.

Verify: A
Status: planned
Source: CON-20

#### Scenario: Budget exceeded
- **WHEN** a frame plan exceeds its draw-call budget
- **THEN** the test fails and prints the count and the budget
