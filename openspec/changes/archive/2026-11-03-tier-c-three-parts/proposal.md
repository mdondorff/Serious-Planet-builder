## Why
BUILD-014's CI job budget is 16 min. After #1103 serialised the heavy WARP tests (`warp-heavy`), the two Windows tier-C halves took 21m13s and 20m33s. Inbox #74 (opus-planner, 2026-11-03).

## What Changes
- The Windows tier-C leg runs in three parts (`--partition count:N/3`, matrix entries for parts 1 to 3), and the job names and artifacts carry the part.
- The Linux leg stays whole: lavapipe took about 13.5 min.
- The repository check test (BUILD-018) and the golden-review skill's artifact list follow.

## Impact
- `.github/workflows/ci.yml`, `xtask/src/repocheck.rs`, `.claude/skills/golden-review/SKILL.md`; BUILD-018 is modified and renamed.
- The job names change from `C 1/2` and `C 2/2` to `C 1/3`, `C 2/3` and `C 3/3`. main has no branch protection, so no required-check names need to change.
- The three job times are reported from this PR's own CI run.
