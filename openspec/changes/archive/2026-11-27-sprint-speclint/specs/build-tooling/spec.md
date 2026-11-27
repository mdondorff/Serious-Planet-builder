## ADDED Requirements

### Requirement: BUILD-076 Spec-lint checks the sprint process's files
`cargo xtask spec-lint` SHALL fail the sprint process's files (ADR 0130) where they break the caps, the archive rule, the routing table, the research package header or the exit-cell diff check of the scenarios below.
A sprint file is a milestone file with an `Opened:` line (BUILD-074); an older milestone file and its folder keep no cap.
The diff check SHALL run against the merge base with `origin/main`, as BUILD-040 does, and SHALL check nothing without git or a base.

Verify: A
Status: active
Source: ADR 0130 point 6 (proposed, applied while proposed; owner 2026-11-27); ADR 0120 point 10; lead 2026-11-27 (the core build's item 4)

#### Scenario: The byte caps
- **WHEN** a sprint file (active, draft or closed) is over 4,096 bytes, a record `docs/milestones/<M>/<E>-record.md` of a sprint over 4,096, or a sprint's `docs/milestones/<M>-report.md` over 6,144
- **THEN** spec-lint names the file and its bytes and fails

#### Scenario: A closed sprint outside the archive
- **WHEN** a sprint file in `docs/milestones/` has a `Status:` starting `closed`
- **THEN** spec-lint fails, naming `docs/archive/<M>/` and `cargo xtask archive-milestone <M>`

#### Scenario: The routing table
- **WHEN** `docs/routing.md` has no `## Rules` section, the section is not a table with the columns # and Check, a row's Check cell is empty, or a numbered rule line stands outside the table
- **THEN** spec-lint names the rule and fails; a Check cell names the check that enforces the rule, or `judgement`

#### Scenario: The research package header
- **WHEN** a `spikes/<name>/results/results.md` opened on or after 2026-11-27 (its `Opened:` date) or titled `# SPIKE-<n>` with n from 145 lacks a value on any of `Opened`, `Exit`, `Question`, `Hypothesis`, `Constraints`, `Out of scope`, `Evaluation (quantitative)`, `Evaluation (visual)`, `Falsifier`, `Code location`; or, once it has a `Decision:` line, lacks `Finding:`, has a decision other than continue, revise, reject or promote, or promotes without `Research-Review: ready (...)`
- **THEN** spec-lint names the file and the label and fails; older spikes are exempt

#### Scenario: The exit-cell diff check
- **WHEN** an exit row of the active sprint file was added, removed or changed in its Item, Mode, Done when, Gate, Park or Lane cell since the merge base, and no added `## Changes` line `- YYYY-MM-DD | <cell> | <old -> new> | <who decided>` names its exit ID
- **THEN** spec-lint names the exit and fails; a malformed added line fails; State and Result move without a line; a draft and a sprint file not at the base are not checked
