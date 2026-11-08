# disk-guardrails (BUILD-038, BUILD-039)

## What
- `xtask/src/scratch.rs`: `test`, `prepush` and `accept` export `TMP`, `TEMP` and `TMPDIR` as `target/test-tmp`, cleared at the start of the next run.
- `xtask/src/worktrees.rs`: `cargo xtask worktrees [--prune] [--no-size]`; `prepush` warns above 12 worktrees.
- `xtask/src/disk.rs`: the free-space gate (refuse below 20 GB, warn below 50 GB) in `prepush`, `store` and `bake`.
- CLAUDE.md, section Git: three lines (worktrees under `.claude/worktrees/`, remove yours when its PR is merged, temp output under `target/`).

## Not in this change
- No cleaning of the existing `%TEMP%\planet-*` folders and no removal of existing worktrees: the owner or the planner runs `cargo xtask worktrees --prune`.
