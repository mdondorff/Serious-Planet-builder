## Why
Two changes written in parallel took the same requirement ID (VIEW-025: region-stage and path-replay). Each passed `spec-lint` on its own branch; main went red when the second merged and blocked every PR's docs check. opus-planner (2026-10-20): `prepush` should check the branch merged onto the current `origin/main`.

## What Changes
- `cargo xtask prepush` gains a first step, `merged spec-lint`: it fetches `origin/main`, makes the merge of `HEAD` and `origin/main` with `git merge-tree --write-tree` (in the object database), checks that tree out in a temporary worktree and runs `spec-lint` on it. The branch, its index and its working tree are not touched; the temporary worktree is removed.
- A branch that already holds `origin/main` has nothing to add and passes; a failed fetch (offline) prints a note and passes; a branch that does not merge cleanly fails and tells the author to merge `origin/main` first.
- BUILD-022; tier-A tests of the plan and of the tree-id parser.

## Classes and bounds (ADR 0035)
Build tooling; no field, no shader value, no golden.

## Out of scope
CI already checks the merge ref of a PR; this brings the same result before the push.
