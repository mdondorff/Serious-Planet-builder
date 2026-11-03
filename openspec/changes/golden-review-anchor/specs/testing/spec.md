## MODIFIED Requirements

### Requirement: TEST-016 Golden review policy
Golden tests SHALL check every golden of a test before failing and write every candidate of the run. `cargo xtask golden-review [--proposal <file>] [--extra <dir>]... [--tests <junit.xml>]... [--apply]` SHALL compare every candidate with its golden on every adapter, write a golden | candidate | diff triptych and `report.md` to `target/review/golden-review/`, and classify each candidate under ADR 0017: auto-bless rounding noise (any class, at most 32 changed pixels, max delta 1 on that adapter; a lit, sky or water view only if its candidate is also within rounding of its anchor: the golden's file at the latest commit with an `Owner-approved:` trailer, else at its first commit, ADR 0096) and declared changes (listed under `Expected golden changes:` in the change's proposal; judged per adapter, each adapter's candidate against its own golden: ADR 0061), and escalate everything else (a changed driver, a lit, sky or water view that drifted beyond rounding from its anchor or has none, first goldens of any class, new adapters, a size change, undeclared changes, unclassified goldens) with a reason (TEST-022). `--apply` SHALL copy only the auto verdicts and never touch `driver.txt`. The test's own oracles, open review findings and earlier owner reverts are checked by the agent applying the `golden-review` skill.

Verify: A
Status: active
Source: CON-18, ADR 0009, ADR 0017, ADR 0061, ADR 0096

#### Scenario: Owner decisions reproduced
- **WHEN** the policy runs on the PR #43 candidates (WARP, llvmpipe, NVIDIA) with the change's declarations
- **THEN** every candidate the owner approved is auto-blessed, and without the declarations every real change escalates

#### Scenario: Every mismatch of a run
- **WHEN** a test checks three goldens and two mismatch
- **THEN** it fails once, naming both, and writes both candidates

#### Scenario: A slow walk reaches the owner
- **WHEN** a lit candidate is 5 pixels from its golden (an earlier auto-bless) but 40 pixels from the golden's file at the last `Owner-approved:` commit
- **THEN** it escalates as drift from the owner's bless, and one within 32 pixels and delta 1 of both is `AUTO-R`
