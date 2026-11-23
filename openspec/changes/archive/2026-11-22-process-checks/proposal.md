# process-checks (BUILD-040 to BUILD-044)

## What
Management rules become checks (owner, 2026-11-09; planner inbox #94). Five small PRs, in this order:
| PR | Requirement | Check |
|---|---|---|
| 1 | BUILD-040 | spec-lint: a change to the gate table or the owner-decision table needs a line in "Contract changes" |
| 2 | BUILD-041 | `docs/gates/ledger.md` and `cargo xtask gates`; spec-lint: every gate of A3 12.1 has a row |
| 3 | BUILD-042 | spec-lint: an item that becomes done moves the briefing's re-estimate date |
| 4 | BUILD-043 | `cargo xtask lanes`: last commit, open PR, latest inbox number, running cargo or example processes per lane |
| 5 | BUILD-044 | `pr-gate` fails a non-docs PR whose body lacks `Verifier: PASS <sha>` of a commit of the PR |

## Not in this change
- No generator code. No change to any gate's pass condition.
- The ledger holds measurements already on main; a value without a traceable commit is `not run`, never an estimate.
