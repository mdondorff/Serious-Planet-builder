## ADDED Requirements
### Requirement: BUILD-037 Prepush checks what the push sends
`cargo xtask prepush` SHALL refuse to run, before any step, when the working tree differs from `HEAD`: any tracked file that differs (staged or not) or any untracked file under `crates/` or `xtask/`. The refusal SHALL name the count and the first paths and say to commit or stash them, because the checks read the working tree while the push sends the commit (a `cargo fmt` after the commit passes the checks and fails CI).

Verify: A
Status: active
Source: Planner 2026-11-04 (inbox #60; #1162 failed CI on fmt after a post-commit `cargo fmt`), BUILD-016

#### Scenario: A dirty tree
- **WHEN** the porcelain status lists tracked files changed (staged, unstaged, renamed) and untracked files under `crates/`, `xtask/` and elsewhere, and a real repository holds a committed tree, then a changed tracked file and a new file under `crates/`
- **THEN** the tracked paths (the new path of a rename) and the untracked ones under `crates/` and `xtask/` are named and the others are not, the committed tree passes, and the dirty one is refused naming the count, the paths and "commit or stash"
