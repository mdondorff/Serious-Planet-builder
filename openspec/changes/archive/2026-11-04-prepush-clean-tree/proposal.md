## Why
- `cargo xtask prepush` checks the working tree while the push sends the commit: #1162 passed prepush, then a `cargo fmt` after the commit changed files that were not in the pushed commit, and CI failed on fmt (planner, inbox #60).

## What Changes
- `prepush` refuses before any step when a tracked file differs from `HEAD` or an untracked file lies under `crates/` or `xtask/` (BUILD-037), naming the count and the first paths and saying to commit or stash them. Scratch notes elsewhere are not part of the check.

## Impact
- Code: `xtask/src/prepush.rs` (`uncommitted`, `require_committed`, a test with a real temporary repository). Requirement BUILD-037 added to the build-tooling spec and synced; the change is archived with this PR.
