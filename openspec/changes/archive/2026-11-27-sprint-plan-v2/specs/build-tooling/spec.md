## ADDED Requirements

### Requirement: BUILD-074 The plan command checks a sprint file
A milestone file with an `Opened:` line SHALL be read as a sprint file (ADR 0130 point 1); any other keeps BUILD-053's form.
`cargo xtask plan` and `cargo xtask spec-lint` SHALL fail a sprint file whose header, exit rows, records or decisions break the shape and the limits of the scenarios below: at most 7 days, at most 5 exits, a record for each promoted or parked row, at most 3 open decisions, and only safe defaults taken on silence (A1).
`plan` SHALL print `overdue` past `Closes by`, and the decisions past their `By` date.
`plan --pr` SHALL check one PR against the R/P firewall and the overdue rule.

Verify: A
Status: active
Source: ADR 0130 points 1-6 (proposed, applied while proposed; owner 2026-11-27); amendments A1 and A4 (lead, 2026-11-27); lead 2026-11-27 (the draft state)

#### Scenario: The shape
- **WHEN** a sprint file is read
- **THEN** it holds a `Status:` line starting `active`, `draft` or `closed`; one line `Opened: <date>   Closes by: <date>   Owner look: <date> | none   R&D lane: <lane>` (dates `YYYY-MM-DD`); an `Objective:` line; an `Out of scope:` line with its `- ` lines; under `## Exit items` a table with the columns #, Item, Mode, Done when, Gate, Park, Lane, State, Result; under `## Decisions` a table with the columns #, Decision, Lead's recommendation, Default, By, State

#### Scenario: The header
- **WHEN** a date is missing or not a calendar date, `Closes by` is before `Opened` or more than 7 days after it, `Owner look` is neither a date nor `none`, `R&D lane` or `Objective` is empty, or `Out of scope` names nothing
- **THEN** plan names the line and fails

#### Scenario: The exit rows
- **WHEN** there is no exit row or more than 5, an exit ID twice, an empty Item, Done when, Gate, Park, Lane or State cell, a mode other than R, P, X, M, a Gate part (`;`-separated) not `<class>: ...` with a class of `num`, `func` (or `fun`), `dom` or `look`, a state other than `open`, `running`, `look-pending`, `waiting-owner`, `done <date>`, `promoted <record>`, `parked <record>`, `carried <M>`, or a promoted row that is not P
- **THEN** plan names the exit and the cell and fails

#### Scenario: The records
- **WHEN** a promoted or parked row's record (a bare name in `docs/milestones/<M>/`, a path with `/` from the repository root) is missing or has no table with the columns criterion, class and verdict; a promotion record has a `fail` or empty verdict; or a promotion or park record takes an exception on a `num` criterion
- **THEN** plan names the exit, the record and the criterion and fails

#### Scenario: The decisions
- **WHEN** more than 3 decisions are open (state empty, `open` or `waiting-owner`), a decision has no Default or no `By` date, or a decision in state `default taken ...` has a default other than park, keep-off or keep today's
- **THEN** plan fails, printing `capped` for the first; past `By`, an open decision is printed as firing when its default is park, keep-off or keep today's, and otherwise as `waiting-owner`

#### Scenario: The queue
- **WHEN** an active sprint's exit is open (not done, promoted, parked or carried) and no queue row of `docs/status.md` names it
- **THEN** plan fails as in BUILD-053

#### Scenario: The firewall
- **WHEN** `plan --pr TITLE [--branch B] [--body-file F] [--files F] PATH...` names an R or P exit that is not promoted (in the title or on a body `Exit:` line) and a path neither docs-only nor under `spikes/`, a `change/*` branch names no X or M exit and no promoted P exit, or the sprint is past `Closes by` and the PR is not docs-only and does not change the sprint file (the stand-in for closing, carrying or parking an exit)
- **THEN** it fails with a line per refusal; docs-only paths are `docs/`, `openspec/`, `.claude/`, a root `*.md`, `spikes/**/results/**` and `spikes/README.md`; without an active sprint file it checks nothing

#### Scenario: Not checked
- **WHEN** a sprint relies on the gate-ledger park counters, a criterion's date set against the exit's first ledger row, A2's `decision:`/`look:` rule or the capped-state row limit
- **THEN** plan does not check it (the combined plan's item 3b and the lead's reading)


## MODIFIED Requirements

### Requirement: BUILD-053 The plan command lists the milestone's exit items against the queue
`cargo xtask plan` SHALL read the active milestone (the file of `docs/milestones/` whose `Status:` line starts with `active`) and its exit table, and SHALL print per exit item its state, lane, done-when, its rows in the queue of `docs/status.md` (first column: the exit item) and the open PRs whose title names it.
It SHALL fail for a queue row whose Exit cell is not an exit ID, an open item without a row, a missing exit table, a duplicate exit ID, and a selection of milestones the scenarios below refuse.
`cargo xtask spec-lint` SHALL run the same checks, without the PR list. An unreadable PR list SHALL print a note and SHALL NOT fail.

Verify: A
Status: active
Source: ADR 0120 point 5 (owner, 2026-11-12), lead 2026-11-12; lead 2026-11-27 (the draft state)

#### Scenario: A queue row off the milestone
- **WHEN** a queue row's Exit cell is not an exit ID of the active milestone
- **THEN** the command names the row and exits non-zero

#### Scenario: An open item without a row
- **WHEN** an exit item is open (its state does not start with `done`, and in a sprint file, BUILD-074, not with `promoted`, `parked` or `carried`) and no queue row names it
- **THEN** the command names the item and exits non-zero; a closed item needs no row

#### Scenario: Between sprints
- **WHEN** no file is active and one draft is (`Status:` starting `draft`)
- **THEN** the command prints `no active sprint; <M> waits for the owner's decision: open`, passes with an empty queue and fails for any queue row

#### Scenario: The selection
- **WHEN** the files hold one active file, one active file and one draft, two active files, two drafts, neither, or a draft that is not a sprint file
- **THEN** the first two pass and the rest fail
