## Why
ADR 0130 (proposed; applied while proposed, owner 2026-11-27) makes the sprint the milestone: one file with at most five exit rows, a mode and a gate on every row, records at mode changes, at most three open decisions with defaults, and a 7-day cap. Its point 6 makes `cargo xtask plan` the check. Today's `plan` (BUILD-053) reads only the old exit table.

## What Changes
- `plan` v2 reads a sprint file (`docs/milestones/<M>.md` with an `Opened:` line; the combined plan's 2.2) and checks the header, the exit rows, the records of promoted and parked rows, the decisions (cap and A1's safe defaults) and `overdue`.
- `plan --pr TITLE [--branch B] [--body-file F] [--files F] PATH...`: the R/P firewall and the overdue rule for one PR (pr-gate calls it, BUILD-075).
- A `draft` state: one draft alone passes between sprints, with an empty queue; one active file and one draft pass during a close (lead, 2026-11-27 22:45).
- Old milestone files (P3) keep BUILD-053's form until they close.
- Not built: the gate-ledger park counters (combined plan 0, item 3b).

## Impact
- Specs: BUILD-074 and TEST-161 added; BUILD-053 modified (the draft state).
- Code: `xtask/src/sprintfile.rs` (new), `xtask/src/plancmd.rs`, `xtask/src/main.rs`; fixtures in `xtask/src/fixtures/`.
