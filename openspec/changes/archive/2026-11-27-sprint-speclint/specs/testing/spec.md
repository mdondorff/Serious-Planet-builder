## ADDED Requirements

### Requirement: TEST-163 Spec-lint's sprint checks are tested on fixtures
Tier-A tests SHALL check BUILD-076 on fixtures (`xtask/src/fixtures/`) and in-memory files: each scenario's failure and its pass, the exemptions, a temporary repository read through spec-lint's entry point, and the repository's own `docs/routing.md` table.

Verify: A
Status: active
Source: Lead 2026-11-27, BUILD-076

#### Scenario: Each failure and each exemption
- **WHEN** the tests run the three byte caps (with a draft), a closed sprint, the old milestone form, the routing table (empty Check cell, a list, a loose numbered rule, missing columns, no section), the research header (a missing and an empty label, an older spike, a SPIKE-145-or-later title, a bad decision, a missing finding, a promotion with and without its review) and the diff check (a changed gate, a logged change, a State move, an added row, a malformed line, a new file, a draft)
- **THEN** each failure names its file and cell, and each pass and exemption gives no error
