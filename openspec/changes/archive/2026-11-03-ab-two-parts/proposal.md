## Why
With the Windows tier-C parts running one archive (BUILD-018, #1133), the A+B job sets the critical path of a CI round. Windows A+B took 16.4 min in #1128's run: a test step of 806 s (257 s build, 548 s for 2,302 tests), the determinism manifest 96 s and the checks about 40 s. Linux A+B ran 15-17 min in recent rounds (opus-planner, 2026-11-03).

## What Changes
- The `test` job runs as two parts per OS (`cargo xtask test -- --partition count:N/2`), each building for itself (no archive, the simple form).
- Part 2 also runs the determinism manifest and its upload, check, spec-lint, licences and the run-mode starts: the count partition gives part 1 the slower tests (Windows 495 s against 272 s in this PR's first run, which had the checks in part 1: 17m41s against 12m03s). The `det-manifest-<OS>` artifact and the `determinism` job are unchanged.
- The test-log and review artifacts carry the part (`-ab-N`).
- BUILD-036 states it; a repository check test reads the workflow.

## Impact
- `.github/workflows/ci.yml`, `xtask/src/repocheck.rs`; BUILD-036 added.
- Two more jobs per CI round. Estimate per part: about 2.5 min setup + 4.3 min build + 4.6 min tests ≈ 11.5 min, the part with the checks about 2 min more.
- The job names change from `<OS> A+B` to `<OS> A+B 1/2` and `2/2`; main has no required check names.
