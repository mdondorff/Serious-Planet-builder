## Why
- `cargo xtask prepush` checks the working tree while the push sends the commit: #1162 passed prepush, then a `cargo fmt` after the commit changed files that were not in the pushed commit, and CI failed on fmt (planner, inbox #60).

## What Changes
- `prepush` refuses before any step when a tracked file differs from `HEAD` or an untracked file lies under `crates/` or `xtask/` (BUILD-037), naming the count and the first paths and saying to commit or stash them. Scratch notes elsewhere are not part of the check.

- The GPU tier (and `gate --small`) are narrowed to the files a golden test or the gate can reach: an `examples/` or `benches/` file and a test file other than `tests/gpu_*.rs` of the golden-producing crates are not golden-producing (BUILD-016 modified; the planner's addendum to #60: prepush ran `test gpu` for 29 min on a one-option change of an app example, explorer's #1165).

## Impact
- Code: `xtask/src/prepush.rs` (`uncommitted`, `require_committed`, a test with a real temporary repository). Requirement BUILD-037 added to the build-tooling spec and synced; the change is archived with this PR.
