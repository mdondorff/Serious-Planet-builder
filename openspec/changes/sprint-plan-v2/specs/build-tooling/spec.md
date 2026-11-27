## ADDED Requirements

### Requirement: BUILD-074 The plan command checks a sprint file
A milestone file with an `Opened:` line SHALL be read as a sprint file (ADR 0130, the combined plan's 2.2). `cargo xtask plan` and `cargo xtask spec-lint` SHALL fail:
- a header without `Opened:` and `Closes by:` dates, with `Closes by` before `Opened` or more than 7 days after it, an `Owner look:` that is neither a date nor `none`, an empty `R&D lane:` or `Objective:`, or no `Out of scope:` line;
- no exit row or more than 5; an empty Item, Done when, Gate, Park, Lane or State cell; a mode other than R, P, X, M; a Gate part not `<class>: ...` with a class of num, fun, dom or look; a state other than open, running, look-pending, waiting-owner, `done <date>`, `promoted <record>`, `parked <record>`, `carried <M>`; a duplicate exit ID; a promoted row that is not P;
- a promoted or parked row whose record is missing or has no criteria table (criterion, class, verdict); a promotion with a `fail` or empty verdict; an exception on a `num` criterion;
- more than 3 open decisions (state `open` or `waiting-owner`; printed `capped`); a decision without a Default or a `By` date; a `default taken` decision whose default is not park, keep-off or keep today's (A1).

`plan` SHALL print `overdue` for an active sprint past `Closes by`, and, for an open decision past `By`, that a safe default fires or that any other waits for the owner. An active sprint's open exits SHALL each have a queue row in `docs/status.md`, as in BUILD-053. `plan --pr TITLE [--branch B] PATH...` SHALL fail a PR whose title names an R or P exit that is not promoted and that touches a path outside `docs/` and `spikes/`; a `change/*` branch whose title names no X or M exit and no promoted P exit; and, past `Closes by`, a PR touching more than `docs/` that does not change the sprint file. A docs-only PR SHALL NOT fail. The gate-ledger park counters are not part of this requirement.

Verify: A
Status: active
Source: ADR 0130 points 1-6 (proposed, applied on the owner's word of 2026-11-27); the combined plan's 0 (A1, A4 item 3a), 2.2, 2.3, 2.5, 3; lead 2026-11-27 22:45 (the draft state)

#### Scenario: A sprint longer than seven days
- **WHEN** `Closes by` is 8 days after `Opened`
- **THEN** plan names both dates and fails

#### Scenario: A promotion with a failed criterion
- **WHEN** a P row is `promoted E2-record.md` and the record has a `fail` verdict or an exception on a `num` criterion
- **THEN** plan names the criterion and fails

#### Scenario: A non-safe default on silence
- **WHEN** an open decision's `By` date has passed and its default is `promote E2`
- **THEN** plan prints that it waits for the owner (A1) and does not fire it; a `default taken` state with that default fails

#### Scenario: An R row's PR touches crates
- **WHEN** `plan --pr` gets a title naming an R exit and a path under `crates/`
- **THEN** it names the exit and the path and fails; the same PR with only `docs/` and `spikes/` paths passes

## MODIFIED Requirements

### Requirement: BUILD-053 The plan command lists the milestone's exit items against the queue
`cargo xtask plan` SHALL read the active milestone (the file of `docs/milestones/` whose `Status:` line starts with `active`) and its exit table, and SHALL print per exit item its state, lane, done-when, its rows in the queue of `docs/status.md` (first column: the exit item) and the open PRs whose title names it. It SHALL fail for a queue row whose Exit cell is not an exit ID, an open item (state not starting `done`) without a row, a missing exit table and a duplicate exit ID. It SHALL accept one active file, one active file and one draft (`Status:` starting `draft`, the next sprint, BUILD-074), or one draft alone; then it prints `no active sprint; <M> waits for the owner's decision: open`, and any queue row is an error. It SHALL fail for two active files, two drafts, or neither, and for a draft that is not a sprint file. `cargo xtask spec-lint` SHALL run the same checks, without the PR list. An unreadable PR list SHALL print a note and SHALL NOT fail.

Verify: A
Status: active
Source: ADR 0120 point 5 (owner, 2026-11-12), lead 2026-11-12; lead 2026-11-27 22:45 (the draft state)

#### Scenario: A queue row off the milestone
- **WHEN** a queue row's Exit cell is not an exit ID of the active milestone
- **THEN** the command names the row and exits non-zero

#### Scenario: An open item without a row
- **WHEN** an exit item is open and no queue row names it
- **THEN** the command names the item and exits non-zero; a done item needs no row

#### Scenario: Between sprints
- **WHEN** no file is active and one draft is
- **THEN** the command prints that the draft waits for the owner's open, passes with an empty queue and fails for any queue row
