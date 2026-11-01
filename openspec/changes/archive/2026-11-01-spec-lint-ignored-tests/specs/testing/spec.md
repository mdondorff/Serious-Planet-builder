## MODIFIED Requirements

### Requirement: TEST-001 Spec traceability
Every requirement in the specs SHALL have a unique trace ID, a verification tier (A, B, C or D) and a status (active, planned or deprecated); every `// spec: ID` citation in the code SHALL name an existing requirement or constitution rule; every active tier A, B or C requirement SHALL be cited by at least one test of its own tier, where a citation counts only when the `// spec:` comment sits directly above a `#[test]` function (only comments and attributes, including wrapped multi-line ones, may sit between; only the `#[test]` attribute counts) and integration test files `tests/gpu_*.rs` are tier C, `tests/oracle_*.rs` tier B and all others (including unit tests under `src/`) tier A, matching the test runner, and a `// spec:` comment directly above a performance-harness scene entry point (a `fn perf_*` item under `crates/app/src/`, with the same comments-and-attributes rule between) counts as tier C evidence only (CON-17: the scene measures what no test may; there is no further tier); every tier D requirement SHALL name its review artefact; and every main spec SHALL have a Purpose section that is not empty and does not contain the placeholder "TBD" (in any case).

Verify: A
Status: active
Source: CON-16, ADR 0009, ADR 0012, planner inbox #44 (2026-11-01)

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

#### Scenario: Ignored test
- **WHEN** a `// spec:` comment sits above a `#[test]` function that is also marked `#[ignore]` (with or without a reason, before or after `#[test]`)
- **THEN** it counts for no tier, and a requirement cited only by ignored tests is reported as not cited
- **AND** the citation must still name an existing requirement; a wall-clock check (BUILD-003) is such a test

#### Scenario: Wrong tier
- **WHEN** an active tier C requirement is cited only by tests outside `gpu_*` files
- **THEN** `cargo xtask spec-lint` fails and says which tiers cite it and which tier is required

#### Scenario: Perf scene citation
- **WHEN** a `// spec:` comment sits directly above a `fn perf_*` item under `crates/app/src/`
- **THEN** it counts as tier C evidence of the requirement (and of no other tier); above any other function, in another crate or under `tests/` it covers nothing
