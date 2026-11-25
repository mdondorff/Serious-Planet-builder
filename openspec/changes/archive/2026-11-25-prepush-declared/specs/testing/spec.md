## ADDED Requirements
### Requirement: TEST-154 The declared prepush mode's tier-A tests
Tier-A tests SHALL check BUILD-068 on synthetic nextest logs and proposals: the declared list's parsing (notes and repeats skipped, a missing section refused), the golden-check panic location from the real testkit source, a pass on exactly the declared candidates with known classes, a failure naming an extra and a missing candidate, a failure for a test that panicked elsewhere and for an unfinished run, and the unchanged verdict without the flag.

Verify: A
Status: active
Source: Lead 2026-11-25 (#204), BUILD-068

#### Scenario: Each verdict
- **WHEN** the judge reads a log with a golden check failure and a known abort against declared, extra and missing candidate sets
- **THEN** it passes only the exact set, and names the difference otherwise
