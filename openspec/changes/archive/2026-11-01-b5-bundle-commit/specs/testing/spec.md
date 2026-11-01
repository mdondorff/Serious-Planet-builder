## MODIFIED Requirements
### Requirement: TEST-071 A bundle of a stored bake replays from the store
A terrain bundle that records a bake of more cells than a run makes in memory (1024 per face edge) SHALL replay with `--store DIR`, the stored bake of that size opened read-only, and SHALL be refused without it, naming the recorded size and `--store DIR`. The replay SHALL check the recorded definition hash, field id and radius as for any recorded world (TEST-008). A terrain bundle SHALL record the commit of the build that wrote it (`commit <sha>`; a bundle without the line still reads). A bundle of another generator or mesh version SHALL be refused naming its commit and `cargo xtask repro <bundle> --at-commit`, or, without a commit, how to find the build. `cargo xtask repro <bundle> --at-commit` SHALL replay in a worktree of the recorded commit under `target/repro-worktree/` and SHALL refuse a bundle without a commit, naming its versions and how to find the commit.

Verify: A
Status: active
Source: opus-planner (2026-10-30, B5; 2026-11-01, SPIKE-84 fix 5), CON-19, TEST-008

#### Scenario: A bake beyond the memory limit
- **WHEN** a bundle records a bake of 4096 cells and is replayed without a store
- **THEN** the replay is refused naming 4096 cells and `--store DIR`, before any replanning

#### Scenario: From the store
- **WHEN** a bundle of a small bake is replayed with the stored bake of the same size and again with the bake made in memory
- **THEN** both worlds give the same heights bit for bit, and a directory that is no store is refused naming it

#### Scenario: The recorded commit
- **WHEN** a bundle is written with a commit, read back, and then written and read without one
- **THEN** the first reads the same commit, the second reads none, and neither is refused for the line

#### Scenario: Replay at the commit
- **WHEN** `repro --at-commit` is given a bundle with a commit, and then one without
- **THEN** the first is replayed in the worktree of that commit, and the second is refused naming the file, the versions to look for and `git log -S`
