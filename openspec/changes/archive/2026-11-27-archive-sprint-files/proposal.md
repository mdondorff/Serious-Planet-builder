## Why
- ADR 0130 point 7 (proposed, applied on the owner's word of 2026-11-27): a sprint closes with its report, and its files move to the archive. The combined sprint process plan's item 7 makes `archive-milestone` do the move.
- P3 closes tonight in the old milestone form, so `P3.md` and `P3-report.md` move the same way.

## What changes
- BUILD-077 (new): `cargo xtask archive-milestone <M>` also moves a closed sprint file, its report and its records folder into `docs/archive/<M>/`, refusing a move onto an existing file, and prints the live files that still name a moved path.
- TEST-164 (new): the tier-A fixture tests.

Expected golden changes: none (an xtask command).

## Impact
- `xtask/src/archivems.rs`, `xtask/src/main.rs` (usage line), `README.md` (the command's row), `.claude/skills/sprint-close/SKILL.md` (step 4 names the moves).
