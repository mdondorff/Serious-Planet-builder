## ADDED Requirements
### Requirement: BUILD-068 Prepush passes a change's declared golden candidates
`cargo xtask prepush --declared <proposal.md>` SHALL pass a failed tier C when the run ended, the names of the golden candidates it wrote are exactly the goldens of the proposal's `Expected golden changes:` section, and every failed test either panicked at the testkit's golden check (`GoldenChecks`, "N golden check(s) failed") or matches a known class of BUILD-056 by name; otherwise it SHALL fail the step, naming the extra and the missing goldens or the uncovered test.

Verify: A
Status: active
Source: Lead 2026-11-25 (#204: the gap PR #1662 hit, pushed under a stated exception), BUILD-056, ADR 0017
Detail:
- the declared goldens are the names of the section's `- <name>: <reason>` lines up to the next `## ` heading; a line that is not `- <name>:` (a note) is skipped; a proposal without the section is refused;
- the golden check's panic location is read from `crates/testkit/src/lib.rs`, so it tracks an edit of the testkit;
- the candidates are the local adapter's (`target/review/candidates/`), counted by name once whatever the adapter; CI's adapters are golden-review's concern;
- without `--declared`, BUILD-056 is unchanged: any written candidate fails the step;
- the mode writes, copies and blesses no golden: the candidates go to golden-review (ADR 0017, 0061, 0096), and lit, sky and water views to the owner's `/bless`;
- any other argument to `prepush` is refused.

#### Scenario: Exactly the declared candidates
- **WHEN** tier C fails on a golden test's golden check and a known WARP abort, and the candidates are the proposal's declared goldens
- **THEN** the step passes and prints the declared list, the golden tests and the known classes for the PR body

#### Scenario: An extra or a missing candidate
- **WHEN** a candidate is not declared, or a declared golden wrote no candidate
- **THEN** the step fails, naming both lists
