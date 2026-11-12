## ADDED Requirements
### Requirement: BUILD-053 The plan command lists the milestone's exit items against the queue
`cargo xtask plan` SHALL read the current milestone, the file of `docs/milestones/` whose `Status:` line starts with `active`, and its exit table (columns `#`, `item`, `done when`, `lane`, `state`), and SHALL print for each exit item its state, lane, done-when, its rows in the queue of `docs/status.md` (the first column of the queue table is the exit item) and the open PRs whose title names it as a whole token. It SHALL exit non-zero for a queue row whose Exit cell is not an exit ID of the milestone and for an open exit item (state not starting `done`) that has no queue row, and for no active milestone, several active milestones, a missing exit table or a duplicate exit ID. A PR list that cannot be read SHALL print a note and SHALL NOT fail the command.

Verify: A
Status: active
Source: ADR 0120 point 5 (owner, 2026-11-12), lead 2026-11-12

#### Scenario: A queue row off the milestone
- **WHEN** a queue row's Exit cell is not an exit ID of the active milestone
- **THEN** the command names the row and exits non-zero

#### Scenario: An open item without a row
- **WHEN** an exit item is open and no queue row names it
- **THEN** the command names the item and exits non-zero; a done item needs no row
