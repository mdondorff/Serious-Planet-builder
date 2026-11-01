## Why
A repro bundle of another generator or mesh version is refused with "check out the matching commit", and the bundle does not say which commit (SPIKE-84, fix 5). Finding it is a manual bisection.

## What changes
- A terrain bundle records the commit of the build that wrote it (`commit <sha>` in its head, optional on read).
- The version-mismatch refusal names that commit and the command that replays there, or, for a bundle without one, how to find the build.
- `cargo xtask repro <bundle> --at-commit` checks the commit out in a worktree under `target/repro-worktree/`, builds there into `target/repro-target/` and replays; a bundle without a commit is refused with the way forward.

## Impact
- Spec: TEST-071 (modified).
- Code: `planet_frame::TerrainBundle`, `crates/app` (writers, refusal text), `xtask` (`repro`).

## Limits
- The recorded commit is `git rev-parse HEAD` of the writing tree: a dirty tree records a commit that does not reproduce its build, and `--at-commit` cannot replay uncommitted changes.
- `target/repro-worktree/` is not cleaned up; `git worktree remove` it when done. A build older than this change cannot read a bundle with the `commit` line (the format version is unchanged).
