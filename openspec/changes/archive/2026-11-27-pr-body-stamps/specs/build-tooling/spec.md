## ADDED Requirements
### Requirement: BUILD-075 The PR-body check also needs the review lines by path, for records and for the active sprint
`cargo xtask pr-body-check` (BUILD-044, the pr-gate check) SHALL also refuse a PR whose body lacks a review line its changed paths need, a record's research review or owner look, or the active sprint's `Sprint: <M>` line; and it SHALL run the active sprint's R/P firewall (`cargo xtask plan --pr`, BUILD-074) on the PR's title, branch, body and changed paths.

Verify: A
Status: active
Source: ADR 0130 point 6 (proposed, applied on the owner's word of 2026-11-27); the combined sprint process plan's item 6; the lead's ruling of 2026-11-27 on when each line applies; CLAUDE.md "How to work" points 3 and 4; BUILD-044
Detail:
| Line | Needed when | Form |
|---|---|---|
| `Numerics: PASS <sha>` | a PR with a non-docs path changes `crates/core/`, `crates/generators/` or a `*.wgsl` | the sha (7 to 64 hex digits) a commit of the PR, as BUILD-044's verifier line |
| `Spec-Guardian: PASS <sha>` | a PR with a non-docs path changes `openspec/` | as above |
| `Research-Review: READY <sha>` | a PR adds or changes a record: `docs/milestones/<M>/<E>-record.md`, or a `spikes/<name>/results/results.md` whose `Decision:` line says `promote` (the file at the PR's head) | as above |
| `Owner-Look: yes` or `Owner-Look: no` | as `Research-Review` | the verdict as the first word (`yesterday` is none); the date and the sheet follow |
| `Sprint: <M>` | a sprint file is active: `docs/milestones/<M>.md` with `Status: active` in the sprint form (an `Opened:` line); not in the old milestone form, a draft or between sprints | `<M>` the active sprint's name |
- A docs-only PR (BUILD-044's paths) needs no path line, `openspec/` included; it still needs the record lines and the `Sprint:` line.
- A record deleted at the PR's head (moved to the archive) and a file under `docs/archive/` are not records.
- Every problem is one line naming the path and the reviewer to run; the check lists them all (the verifier line, the review lines, the firewall's), then fails.
- The PR's title and head branch come from `PR_TITLE` and `HEAD_REF` in the environment, like the body.

#### Scenario: A generator PR without its numerics line
- **WHEN** a PR changes `crates/generators/src/erosion.rs` and its body has `Verifier: PASS <sha>` but no `Numerics:` line, or one naming no commit of the PR
- **THEN** the check fails naming the path and the numerics-reviewer agent; with `Numerics: PASS <sha>` of a commit of the PR it passes
