## Why
Since the Windows update of 2026-10-30 (build 26300) the local software tier C aborts about one test in 90 per run with an access violation inside `d3d10warp.dll`; CI's WARP is fine (docs/learnings.md). Every lane's prepush fails on it, and a reader has to dig the cause out of the log. Targeted stress of the crashing tests at 1, 8 and 20 test processes (132 runs) did not reproduce it, so no change of parallelism is justified.

## What Changes
1. `cargo xtask test*` names a test aborted with `0xc0000005` on the software adapter in its summary: `WARP crash (d3d10warp), CI judges: <test>`.
2. The step still fails: no retry, no skip, no changed test, the exit code is unchanged (BUILD-003 scenario added).
3. The learning "WARP crashes in its own code" records the rate and the stress result.

## Impact
- `xtask/src/testcmd.rs` (a log parser with a tier-A test), `openspec/specs/build-tooling` (BUILD-003, one scenario), `docs/learnings.md`.
