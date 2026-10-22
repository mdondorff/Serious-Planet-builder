# Change: spec-lint-caps

## Why
- M7 H1 (opus-planner, 2026-10-21; ADR 0075): the hot files must stay small and current, and the rule must be a check, or the housekeeping at each milestone is the only thing holding it.

## What changes
- `cargo xtask spec-lint` fails when `docs/status.md` (15 KiB), `docs/learnings.md` (20 KiB), `docs/spikes.md` (10 KiB), `CLAUDE.md` (7,514 B) or `docs/constitution.md` (8,574 B) is over its cap, with the file, the size and the cap named (`HOT_FILE_CAPS`, `hot_file_errors`, `lf_size`: a CRLF line end is one byte, so a Windows and a Linux checkout agree). BUILD-023.
- Already in place and tested (TEST-001): a test that cites a requirement ID that no spec holds fails the lint (`citing_an_unknown_id_is_reported_and_known_ids_pass`).

## The mechanical pass (docs/archive/README.md, 2026-10-21 run)
- Searched the live specs, the code comments, `CLAUDE.md` and `.claude/` for paths that no longer exist (every `docs/`, `openspec/`, `crates/`, `xtask/`, `tests/`, `spikes/` path in a comment or a spec, checked against the tree): none of them points to the deleted change archive or a deleted briefing. What the scan lists is test fixtures (paths a test builds in a temp tree), crate-relative `tests/...` names, and references inside the frozen milestone reports (`docs/milestones/M5-report.md` names a deleted change; frozen, left as it is) and ADR 0032 (accepted ADRs are not edited). Nothing to fix.

## Tests
- Tier A (`xtask/src/speclint.rs`): over and at each cap, the caps' values, a missing or uncapped file, CRLF counting, and the repository's own hot files.

Expected golden changes: none.
