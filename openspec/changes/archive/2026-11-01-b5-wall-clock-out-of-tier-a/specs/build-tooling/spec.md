## MODIFIED Requirements

### Requirement: BUILD-003 Tiered test commands
`cargo xtask test fast` SHALL run tier A tests for the changed crates and every crate above them, `cargo xtask test` SHALL run tiers A and B, and `cargo xtask test gpu` SHALL run tier C; tiers SHALL be selected by test-binary name (`gpu_*` is tier C, `oracle_*` tier B, all others tier A) on the software adapter unless `--real` is given. `cargo xtask test gpu --real` SHALL also run the tier-B oracles, on the real adapter. After a `test fast` run the command SHALL print the five slowest tests of that run, the compile time apart from the test time, and, when the test time (nextest's run time, not the compile) of a selection of changed crates exceeded the 20 s budget (the owner's decision of 2026-10-16), a warning (a printed warning, not a failure: a hard limit on a loaded machine is flaky; `cargo xtask accept` enforces the budget in its timing step). A run that selects every crate (nothing changed, a root manifest changed, or `--all`) SHALL report its time and SHALL NOT warn. The changed files SHALL be those against `origin/main` when it exists, else `main`.

Verify: A
Status: active
Source: ADR 0009, report §16

#### Scenario: Change in a middle crate
- **WHEN** only files of `streaming` changed
- **THEN** `test fast` selects `streaming`, `frame`, `render`, `editor` and `app`

#### Scenario: Budget on a selection
- **WHEN** `test fast` selects some crates and the tests take longer than 20 s
- **THEN** it prints the warning and the five slowest tests; when it selects every crate it prints a note and no warning; the compile time is never counted

#### Scenario: Acceptance reads the same budget
- **WHEN** `cargo xtask accept <milestone>` runs its `test fast` timing step
- **THEN** the step judges the test time (not the compile) of a selection of changed crates against 20 s and fails over it; a run of every crate passes the step with its number in the row, marked `whole workspace: reported (BUILD-003)`

#### Scenario: Real-GPU parallelism cap
- **WHEN** `cargo xtask test gpu --real` runs on the discrete adapter
- **THEN** it runs nextest under the `real-gpu` profile (`.config/nextest.toml`) with at most 4 test processes alive at once (`--test-threads 4`, unless the caller passes `-j` or `--test-threads`), and the heavy binaries (the lit, water, atmosphere and streaming-perf scenes, `gpu_compact_blocks`, `oracle_concurrent_source`, `oracle_lit`) share a test group of at most 2; about 20 parallel processes lost the device (`wgpu device lost (Unknown)`)
- **AND** no test is skipped, retried or changed, the software and WARP runs keep the default profile, and the JUnit report of a `--real` run is `target/nextest/real-gpu/junit.xml`

#### Scenario: The GPU tests have a hang limit of 300 s
- **WHEN** a test of a `gpu_*` binary runs under the default nextest profile (the software-adapter tier C, CI's WARP and lavapipe runs, and the `real-gpu` profile that inherits it)
- **THEN** nextest marks it slow every 60 s and terminates it after 300 s (5 periods), where every other test terminates after 180 s; the limit catches a hang and is not a budget: the heavy `gpu_*` tests take 40 to 66 s alone and up to 144 s beside the other processes of a full run, and grouping them did not shorten the run
- **AND** a test with a declared limit in `.config/nextest.toml` (its measured durations and the planner's approval in a comment beside it) terminates after 450 s (5 x 90 s) instead; today the window flight, the coastal flight, the lit coast view, the edit-latency smoke scene and the stills scene; no test is skipped, retried or changed

#### Scenario: The GPU tier runs every test
- **WHEN** `cargo xtask test gpu` runs (software or `--real`)
- **THEN** it passes `--no-fail-fast` to nextest (unless the caller passes `--fail-fast` or `--no-fail-fast`), so that a failing golden test does not cancel the others and every candidate golden of the run is written for `golden-review`

#### Scenario: Stale local main
- **WHEN** a branch is checked out in a worktree whose local `main` is behind `origin/main`
- **THEN** the changed files are those of `origin/main...HEAD` (and the working tree), not of `main...HEAD`; `main` is used only when there is no `origin/main`

#### Scenario: Shared crate changed
- **WHEN** `testkit`, `perf` or a root manifest changed, or nothing changed
- **THEN** `test fast` selects every crate

#### Scenario: Slowest tests named
- **WHEN** a nextest JUnit report of a `test fast` run is read
- **THEN** the five tests with the greatest times are listed, slowest first, each as its binary and test name

#### Scenario: A WARP access violation is named, not hidden
- **WHEN** a `cargo xtask test`, `test fast` or `test gpu` run on the software adapter ends with a test process aborted by `0xc0000005` (the access violation in `d3d10warp.dll`, docs/learnings.md)
- **THEN** the summary names each such test as `WARP crash (d3d10warp), CI judges: <test>` before the log path
- **AND** the step still counts as failed with its own exit code: no retry, no skip, no test changed

#### Scenario: The software GPU tier runs at most 8 GPU tests at once
- **WHEN** a `gpu_*` test runs under the default nextest profile
- **THEN** it is in the test group `gpu-tier` of at most 8 processes at once, unless an earlier override puts it in a heavier group; the window, water-coverage, stream-perf, envelope-perf, edit-perf and stills tests and the three loop tests of `gpu_skeleton` are in `heavy-window` of at most 2; tier A and B tests are not capped
- **AND** a machine with 8 cores or fewer (CI's runners) runs as before; on the 16-thread laptop the software tier C no longer runs about 20 rasterising processes at once (SPIKE-84: the 12 timeouts of 2026-10-20 and 3 load timeouts on 2026-10-31)

#### Scenario: Wall-clock checks run with perf, not in tier A
- **WHEN** a test would assert on the time real work takes (a timing ratio, a spin's milliseconds, a lock wait on real threads)
- **THEN** it is an ignored test with the reason `wall clock: cargo xtask perf wallclock`, listed in xtask's `WALLCLOCK`, and `cargo xtask perf wallclock` runs each alone in release, naming each PASS or FAIL; tier A asserts the counts, the results and the order of events instead, and a tier-A test checks that every listed check exists as such an ignored test
- **AND** a wall-clock check is evidence of the reference machine outside the tiers: its `// spec:` citation does not stand in for a tier-A test of the same scenario, and CI, `test` and `accept` do not run it; the command runs no readiness gate (tolerance checks, not measurements, ADR 0020), so it is run on a quiet Rasierklinge outside a perf window
- **AND** today the list is the real spin within 5 % (BUDG-011), the lock held 40 ms (VIEW-056) and the cost of the margin of a seam cell against an interior one (the picking query of VIEW-022; no spec states its number, the check guards the seam form of `with_margin` against a return to the geometric walk); a hang guard (a deadline after which a test fails rather than stalls) is not a wall-clock check
