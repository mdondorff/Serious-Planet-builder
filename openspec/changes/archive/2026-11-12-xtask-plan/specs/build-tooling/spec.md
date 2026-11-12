## ADDED Requirements
### Requirement: BUILD-053 The plan command lists the milestone's exit items against the queue
`cargo xtask plan` SHALL read the active milestone (the file of `docs/milestones/` whose `Status:` line starts with `active`) and its exit table, and SHALL print per exit item its state, lane, done-when, its rows in the queue of `docs/status.md` (first column: the exit item) and the open PRs whose title names it. It SHALL fail for a queue row whose Exit cell is not an exit ID, an open item (state not starting `done`) without a row, no or several active milestones, a missing exit table and a duplicate exit ID. `cargo xtask spec-lint` SHALL run the same checks, without the PR list. An unreadable PR list SHALL print a note and SHALL NOT fail.

Verify: A
Status: active
Source: ADR 0120 point 5 (owner, 2026-11-12), lead 2026-11-12

#### Scenario: A queue row off the milestone
- **WHEN** a queue row's Exit cell is not an exit ID of the active milestone
- **THEN** the command names the row and exits non-zero

#### Scenario: An open item without a row
- **WHEN** an exit item is open and no queue row names it
- **THEN** the command names the item and exits non-zero; a done item needs no row
