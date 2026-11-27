## ADDED Requirements
### Requirement: BUILD-077 The milestone archive command also moves a closed sprint's file, report and records
`cargo xtask archive-milestone <M>` (BUILD-072) SHALL also move the sprint file `docs/milestones/<M>.md`, when its first `Status:` line starts with `closed`, together with its report `docs/milestones/<M>-report.md` and the files of its records folder `docs/milestones/<M>/`, into `docs/archive/<M>/`.

Verify: A
Status: active
Source: ADR 0130 point 7 (proposed, applied on the owner's word of 2026-11-27); the combined sprint process plan's item 7 (lead, 2026-11-27); BUILD-072
Detail:
| From | To |
|---|---|
| `docs/milestones/<M>.md` | `docs/archive/<M>/<M>.md` |
| `docs/milestones/<M>-report.md` (when it exists) | `docs/archive/<M>/<M>-report.md` |
| `docs/milestones/<M>/<path>` (every file, at any depth) | `docs/archive/<M>/<path>` |
- The old milestone form moves the same way (P3: `P3.md` and `P3-report.md`, once `P3.md`'s status says closed).
- A sprint file that is not closed, or has no `Status:` line, stays with its report and records; the run prints why, and its log and spike steps still run.
- A move onto an existing file, onto a file the same run writes (the log or spike archive), or onto another move's destination refuses the run before any write or move.
- The emptied records folder is removed; `docs/milestones/` stays.
- The live files that still name a moved path (`docs/` outside the archive, `openspec/specs/`, `.claude/`, `xtask/src/`, the root `*.md`) are printed as `<file>: <old path>`, not rewritten.
- `--dry-run` prints the moves and writes nothing; a second run has nothing to move.

#### Scenario: A closed sprint archived
- **WHEN** the command runs for a closed sprint with a report, a record and a nested records file, while `docs/status.md` names the sprint file
- **THEN** the four files are under `docs/archive/<M>/` byte for byte, the records folder is gone, `docs/status.md` is printed and unchanged, and a second run moves nothing
