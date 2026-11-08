## ADDED Requirements
### Requirement: BUILD-038 Test scratch lives under target and is cleared by the next run
`cargo xtask test`, `prepush` and `accept` SHALL point `TMP`, `TEMP` and `TMPDIR` of every child process at `target/test-tmp` of the worktree, clear that folder when the next run starts (the last run stays for diagnosis) and create it empty; a run, killed or not, SHALL leave nothing in the system temp directory. No source of `crates/` or `xtask/` SHALL name a system temp folder; tests ask `std::env::temp_dir`.

Verify: A
Status: active
Source: Owner 2026-11-08 (C: reached 100 %; about 8,500 `planet-*` folders in the system temp directory), planner inbox #93

#### Scenario: A killed run
- **WHEN** a run leaves a folder with files in the scratch and the next run starts
- **THEN** the scratch is empty again and the three variables name it

### Requirement: BUILD-039 Worktrees are listed and pruned safely, and the disk is gated
`cargo xtask worktrees` SHALL list every worktree with size (time-limited), branch, uncommitted count, last commit and merged PR. `--prune` SHALL remove, through `git worktree remove` without `--force`, only a clean, unlocked worktree whose branch's merged PR ended at its head commit or that is detached at an ancestor of `origin/main`; never the current worktree or the root; it SHALL print what it removed and kept. `prepush` SHALL warn above 12 worktrees naming the prune command. `prepush`, `store` and `bake` SHALL refuse below 20 GB free on the worktree's drive, naming the free space, the largest folders of `target/` and the prune command, and warn below 50 GB.

Verify: A
Status: active
Source: Owner 2026-11-08 (41 worktrees, many forgotten inside other worktrees' `target/` and scratch folders), planner inbox #93, BUILD-037

#### Scenario: Prune in a repository
- **WHEN** a repository has a clean worktree detached at an ancestor of `origin/main`, one with an untracked file and one detached at a commit ahead of `origin/main`
- **THEN** only the clean ancestor one is removed, the others are kept naming "uncommitted" and "not in origin/main", and the root is kept

#### Scenario: The gate
- **WHEN** 80, 35 and 12 GB are free
- **THEN** the first passes, the second warns and the third refuses, naming the free space, the largest folders and `cargo xtask worktrees --prune`
