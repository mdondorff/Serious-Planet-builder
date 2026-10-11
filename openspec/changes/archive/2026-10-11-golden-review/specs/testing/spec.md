## ADDED Requirements

### Requirement: TEST-016 Golden review policy
Golden tests SHALL check every golden of a test before failing and write every candidate of the run. `cargo xtask golden-review [--proposal <file>] [--extra <dir>]... [--apply]` SHALL compare every candidate with its golden on every adapter, write a golden | candidate | diff triptych and `report.md` to `target/review/golden-review/`, and classify each candidate under ADR 0017: auto-bless rounding noise (a debug or shaded view, at most 32 changed pixels, max delta 1) and declared changes (listed under `Expected golden changes:` in the change's proposal; debug views also agreeing across adapters within +-5 %), and escalate everything else (lit views, first goldens, new adapters, disagreement, undeclared changes, size changes, unclassified goldens) with a reason. `--apply` SHALL copy only the auto verdicts.

Verify: A
Status: active
Source: CON-18, ADR 0009, ADR 0017

#### Scenario: Owner decisions reproduced
- **WHEN** the policy runs on the PR #43 candidates (WARP, llvmpipe, NVIDIA) with the change's declarations
- **THEN** every candidate the owner approved is auto-blessed, and without the declarations every real change escalates

#### Scenario: Every mismatch of a run
- **WHEN** a test checks three goldens and two mismatch
- **THEN** it fails once, naming both, and writes both candidates
