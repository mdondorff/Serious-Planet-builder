## Why
BUILD-014's CI job budget is 16 min. With three Windows tier-C parts, C 2/3 ran 21-22 min in every run since #1114 (21m54s, 22m20s; #1125 22m53s). `count:N/3` dropped 878 s of the serialised `warp-heavy` tests into that part (lit_stream_flicker 334 s, window flight 208 s and five more, one at a time). Option (2) of the planner's three (opus-planner, 2026-11-03).

## What Changes
- The Windows tier-C leg runs in four parts (`--partition count:N/4`, matrix entries for parts 1 to 4); the job names and artifacts carry the part.
- The Linux leg stays whole.
- The repository check test (BUILD-018) and the golden-review skill's artifact list follow.

## Impact
- `.github/workflows/ci.yml`, `xtask/src/repocheck.rs`, `.claude/skills/golden-review/SKILL.md`; BUILD-018 is modified and renamed.
- One more Windows runner per CI run. The expected `warp-heavy` per part is about 370 s; a bad draw is still possible, so the four job times of this PR's run and of the next two runs are reported.
- The job names change from `C N/3` to `C N/4`; main has no required check names.
- Next: a shared build job with a nextest archive (option 4) removes the about 5.5 min build from every part.
