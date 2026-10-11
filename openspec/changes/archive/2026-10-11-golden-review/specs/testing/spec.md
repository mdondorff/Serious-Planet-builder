## ADDED Requirements

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

## MODIFIED Requirements

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
