## Why
ADR 0130 point 6 makes spec-lint enforce the sprint process's files, and ADR 0120 point 10 (every rule is a check) mechanical: the combined plan's item 4 of the core build (lead, 2026-11-27).

## What Changes
- Byte caps: a sprint file and a record at most 4 KB, a sprint's report at most 6 KB (drafts too; the old milestone form is exempt).
- A closed sprint file in `docs/milestones/` fails: it moves to `docs/archive/<M>/`.
- `docs/routing.md`'s rules become a table with a `Check` column (rule text unchanged); an empty Check cell or a numbered rule outside the table fails.
- The research package header of spikes opened from 2026-11-27 (or numbered from SPIKE-145), with `Finding:`, `Decision:` and, for a promotion, `Research-Review:` at close; older spikes are exempt.
- The exit-cell diff check: a changed exit or gate cell of the active sprint needs an added `## Changes` line naming its exit (BUILD-040's mechanism, `process.rs`).

## Impact
- Specs: BUILD-076 and TEST-163 added.
- Code: `xtask/src/sprintlint.rs` (new), `xtask/src/speclint.rs`, `xtask/src/main.rs`; `docs/routing.md`'s Rules table; fixture `xtask/src/fixtures/research-open.md`.
