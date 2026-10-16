## ADDED Requirements

### Requirement: TEST-022 Golden review: first goldens of M5 classes (ADR 0037)
For the first golden of an M5 class (`lit`, later `sky` and `water`; the window is `ADR_0037_M5_CLASSES`, which the owner revisits at the M5 gate), `golden-review` SHALL bless under policy (verdict `AUTO-M5`) only when all of these hold, and SHALL escalate with the failing condition otherwise: (a) the golden's class is an M5 class, its spec names each attribute's class and bound (ADR 0035), and every bound test that the proposal declares under `ADR 0037 conditions:` appears green in the nextest JUnit reports of this run passed with `--tests` (a test that is missing, failed, skipped or retried is not green; no report is no evidence); (b) the declared debug view exists and agrees across the adapters that took part under ADR 0017's cross-adapter rule (a view with a blessed golden matches it or changes in a way ADR 0017 blesses; a new view agrees candidate against candidate to rounding noise, at most 32 pixels and delta 1), on at least two adapters; (c) the golden is declared under `Expected golden changes:`. An adapter that has goldens but delivered no candidates SHALL be reported as not run and SHALL NOT count as agreeing. The path SHALL apply only to the first golden of an M5 class: every other class, every existing golden (a changed lit view still escalates), a new adapter family of an existing golden, and the driver gate of ADR 0017 (a candidate from another driver than the recorded one escalates) SHALL behave exactly as under ADR 0017.

Verify: A
Status: active
Source: CON-18, ADR 0037, ADR 0017, ADR 0035

#### Scenario: All conditions met
- **WHEN** a first `lit` golden is declared, its bound tests are green in the reports and its debug view agrees on two adapters
- **THEN** the verdict is `AUTO-M5`

#### Scenario: One condition missing
- **WHEN** the bound tests are missing or red, or the debug view is missing or the adapters disagree, or the golden is not declared
- **THEN** the candidate escalates with the reason

#### Scenario: Other goldens unchanged
- **WHEN** the class is `debug` or `shaded`, or the lit golden already exists, or it exists on another adapter
- **THEN** the verdict is the one ADR 0017 gives

#### Scenario: Missing adapter
- **WHEN** an adapter with goldens delivered no candidates
- **THEN** the output says it did not run, it is not counted as agreeing, and fewer than two adapters escalate

## MODIFIED Requirements

### Requirement: TEST-016 Golden review policy
Golden tests SHALL check every golden of a test before failing and write every candidate of the run. `cargo xtask golden-review [--proposal <file>] [--extra <dir>]... [--tests <junit.xml>]... [--apply]` SHALL compare every candidate with its golden on every adapter, write a golden | candidate | diff triptych and `report.md` to `target/review/golden-review/`, and classify each candidate under ADR 0017: auto-bless rounding noise (a debug or shaded view, at most 32 changed pixels, max delta 1) and declared changes (listed under `Expected golden changes:` in the change's proposal; for debug views also on at least two adapters, with changed pixels agreeing within the larger of 5 % and 8 pixels and max deltas within the larger of 5 % and 1), and escalate everything else (a changed driver, lit views, first goldens, new adapters, disagreement, a size change on any adapter, undeclared changes, unclassified goldens) with a reason, except the first goldens of M5 classes under the conditions of TEST-022 (ADR 0037). `--apply` SHALL copy only the auto verdicts and never touch `driver.txt`. The test's own oracles, open review findings and earlier owner reverts are checked by the agent applying the `golden-review` skill.

Verify: A
Status: active
Source: CON-18, ADR 0009, ADR 0017, ADR 0037

#### Scenario: Owner decisions reproduced
- **WHEN** the policy runs on the PR #43 candidates (WARP, llvmpipe, NVIDIA) with the change's declarations
- **THEN** every candidate the owner approved is auto-blessed, and without the declarations every real change escalates

#### Scenario: Every mismatch of a run
- **WHEN** a test checks three goldens and two mismatch
- **THEN** it fails once, naming both, and writes both candidates
