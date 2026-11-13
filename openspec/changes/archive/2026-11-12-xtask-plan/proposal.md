## Why
- ADR 0120 point 5 (owner, 2026-11-12: the plan is accepted): work follows the current milestone's exit items; a check keeps the queue and the exit table in step.

## What changes
- `cargo xtask plan` (BUILD-053): prints each exit item of the active milestone with its state, queue rows and open PRs naming it; fails for a queue row whose Exit cell is not an exit ID and for an open item without a row.
- Tier A tests on fixture text (`xtask/src/plancmd.rs`).

## Not in this change
- Wiring the check into spec-lint: after it passes on main (lead's order).
- The milestone file `docs/milestones/P1.md` and the queue's Exit column: the lead's docs PR.

## Impact
xtask (plancmd.rs, main.rs), build-tooling spec delta.
