## MODIFIED Requirements

### Requirement: TEST-022 Golden review: first goldens of M5 classes (ADR 0037)
For the first golden of an M5 class (`lit`, later `sky` and `water`; the window is `ADR_0037_M5_CLASSES`, which the owner revisits at the M5 gate), `golden-review` SHALL bless under policy (verdict `AUTO-M5`) only when all of these hold, and SHALL escalate with the failing condition otherwise: (a) the golden's class is an M5 class, its spec names each attribute's class and bound (ADR 0035), and every bound test that the proposal declares under `ADR 0037 conditions:` appears green in the nextest JUnit reports of this run passed with `--tests` (a test that is missing, failed, skipped or retried is not green; no report is no evidence); (b) the declared debug view exists and agrees across the adapters that took part under ADR 0017's cross-adapter rule (a view with a blessed golden matches it or changes in a way ADR 0017 blesses; a new view agrees candidate against candidate pairwise when at most 64 pixels differ by more than delta 1, by any amount, and besides those at most 32 pixels differ at exactly delta 1 (ADR 0017 rounding noise), and no other pixel differs: ADR 0052 as read by opus-planner on 2026-10-17; the report names the cap exceeded and the counts), on at least two adapters; (c) the golden is declared under `Expected golden changes:`. An adapter that has goldens but delivered no candidates SHALL be reported as not run and SHALL NOT count as agreeing. The path SHALL apply only to the first golden of an M5 class: every other class, every existing golden (a changed lit view still escalates), a new adapter family of an existing golden, and the driver gate of ADR 0017 (a candidate from another driver than the recorded one escalates) SHALL behave exactly as under ADR 0017.

Verify: A
Status: active
Source: CON-18, ADR 0037, ADR 0052, ADR 0017, ADR 0035

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
