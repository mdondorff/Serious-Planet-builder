# build-tooling Specification

## Purpose
The workspace and its tooling: the layered crates with downward-only dependencies and the graphics API confined to `render` (CON-02, ADRs 0004, 0008, 0015), the single `cargo xtask` command surface and test tiers (CON-25, ADR 0009), the three run modes (CON-01), CI on Windows and Linux (CON-26) and the licence policy (CON-27).

## Requirements

### Requirement: BUILD-001 Layered crates
The workspace SHALL contain the crates `core`, `world-def`, `generators`, `cache`, `streaming`, `frame`, `render`, `editor` and `app` such that every crate depends only on crates below it in that order, and `core` through `frame` SHALL have no graphics crate (wgpu, naga, winit, ash, egui) anywhere in their dependency tree. No crate other than `render` SHALL list a graphics-API crate (wgpu, wgpu-core, wgpu-hal, naga, ash) in any dependency section, dev and build dependencies included, and `render`'s public API SHALL NOT expose a wgpu type; `render`'s public API is the seam to the graphics API (ADR 0004). The `render` feature `reference-renderer` (REND-031) SHALL be enabled only by `[dev-dependencies]`: no `[dependencies]` or `[build-dependencies]` entry of any crate, and no `default` feature, SHALL name it.

Verify: A
Status: active
Source: CON-02, ADR 0004, ADR 0008, ADR 0015, ADR 0062

#### Scenario: Upward dependency
- **WHEN** a crate lists a crate at its own or a higher layer under `[dependencies]`
- **THEN** `cargo xtask layering` fails and names both crates and the chain

#### Scenario: Graphics crate in a GPU-free tree
- **WHEN** a GPU-free crate gains a transitive graphics dependency
- **THEN** `cargo xtask layering` fails and names the crate and the dependency (also for the table form `[dependencies.wgpu]` and for a renamed package)

#### Scenario: wgpu type in render's public API
- **WHEN** a public item of `render` names a wgpu type
- **THEN** `cargo xtask layering` fails with file and line

#### Scenario: Graphics API outside render
- **WHEN** a crate other than `render` (including a test-support crate or xtask) lists `wgpu` under any dependency section
- **THEN** `cargo xtask layering` fails and names the crate and the dependency (also for the table form `[dependencies.wgpu]` and for a renamed package)

#### Scenario: Reference renderer in a product build
- **WHEN** a `[dependencies]` or `[build-dependencies]` entry of any crate enables the `reference-renderer` feature of `render` (inline, table form or through a renamed package), or a `default` feature lists it
- **THEN** `cargo xtask layering` fails and names the crate and the entry

#### Scenario: wgpu type in render's public API
- **WHEN** a public item of `render` names a wgpu type
- **THEN** `cargo xtask layering` fails with file and line

### Requirement: BUILD-002 Single command surface
All build, test and tooling commands SHALL be available through `cargo xtask`, the repository SHALL contain no `.sh` or `.ps1` files, and the performance command SHALL leave readiness to the harness's measured gate (ADR 0020), passing the owner's `--machine-ready` only as an override of a failed gate and rejecting other options.

Verify: A
Status: active
Source: CON-25, CLAUDE.md

#### Scenario: Script file added
- **WHEN** a `.sh` or `.ps1` file exists in the repository outside `target/`
- **THEN** the repository check test fails and lists the file

#### Scenario: Perf options
- **WHEN** `cargo xtask perf` is built into a command line
- **THEN** `--machine-ready` is passed through only when given, and any other option is an error naming the override

### Requirement: BUILD-003 Tiered test commands
`cargo xtask test fast` SHALL run tier A tests for the changed crates and every crate above them, `cargo xtask test` SHALL run tiers A and B, and `cargo xtask test gpu` SHALL run tier C; tiers SHALL be selected by test-binary name (`gpu_*` is tier C, `oracle_*` tier B, all others tier A) on the software adapter unless `--real` is given. `cargo xtask test gpu --real` SHALL also run the tier-B oracles, on the real adapter. After a `test fast` run the command SHALL print the five slowest tests of that run, the compile time apart from the test time, and, when the test time (nextest's run time, not the compile) of a selection of changed crates exceeded the 20 s budget (the owner's decision of 2026-10-16), a warning (a printed warning, not a failure: a hard limit on a loaded machine is flaky; `cargo xtask accept` enforces the budget in its timing step). A run that selects every crate (nothing changed, a root manifest changed, or `--all`) SHALL report its time and SHALL NOT warn. The changed files SHALL be those against `origin/main` when it exists, else `main`.

Verify: A
Status: active
Source: ADR 0009, report §16, ADR 0020, planner inbox #44 (2026-11-01)

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
- **AND** a wall-clock check is evidence of the reference machine outside the tiers: its `// spec:` citation does not stand in for a tier-A test of the same scenario, and CI and `test` do not run it
- **AND** today the list is the real spin within 5 % (BUDG-011), the lock held 40 ms (VIEW-056) and the cost of the margin of a seam cell against an interior one (the picking query of VIEW-022; no spec states its number, the check guards the seam form of `with_margin` against a return to the geometric walk); a hang guard (a deadline after which a test fails rather than stalls) is not a wall-clock check

#### Scenario: The wall-clock checks run under the readiness gate
- **WHEN** `cargo xtask perf wallclock` runs, on its own or as the last step of `cargo xtask accept`
- **THEN** it builds the checks, then runs the readiness gate of ADR 0020 without its reference-adapter condition (the checks use the CPU only), then each check; a refused gate is NOT RUN with the refused conditions (`INCOMPLETE:`, exit code 3), no verdict and never a failure, and `--machine-ready` (the owner's override) runs the checks anyway
- **AND** in `accept` a step that ends `INCOMPLETE:` without a locked-desktop marker lists its text as a NOT RUN line, so the step and the run are INCOMPLETE, never a pass (see TEST-073)

### Requirement: BUILD-004 Three run modes
The single binary SHALL provide the modes `editor`, `generate` and `test-render`, each of which starts and exits cleanly with `--smoke`; for `editor`, `--smoke` opens the viewer window (VIEW-005) and the display-free start check is `editor --smoke --no-window`; headless PNG output of `test-render` is covered by TEST-004 and REND-007.

Verify: A
Status: active
Source: CON-01, ADR 0008, ADR 0042

#### Scenario: Smoke start
- **WHEN** each mode is started with `--smoke` (the editor with `--no-window` where there is no display)
- **THEN** it prints a line containing `smoke ok` and exits with status 0

#### Scenario: Unknown mode
- **WHEN** an unknown mode or option is given
- **THEN** the program exits non-zero and prints the usage text

### Requirement: BUILD-005 CI on both operating systems
Continuous integration SHALL run check, spec-lint, licence check, tiers A to C and the run-mode smoke starts on Windows (software adapter WARP) and on Linux (software Vulkan lavapipe) for every pull request and every push to `main`. A change that touches only `docs/`, `openspec/`, `.claude/`, root `*.md` files or `spikes/**/results/**` (owner, 2026-10-16) SHALL run the light `docs` workflow (spec-lint on Linux) instead; both workflows SHALL name the same list of those paths, and any other file in a change runs the full workflow.

Verify: A
Status: active
Source: CON-26, report §9

#### Scenario: Workflow content
- **WHEN** the workflow file is parsed by the repository check test
- **THEN** it names both a Windows and a Linux runner and invokes `cargo xtask check`, `spec-lint`, `test` and `test gpu`

#### Scenario: Docs-only paths agree
- **WHEN** the repository check test reads both workflows
- **THEN** the paths the full workflow ignores equal the paths the docs workflow runs on, and include `spikes/**/results/**`

### Requirement: BUILD-006 Licence policy
Dependency licences SHALL be checked by `cargo deny` against an explicit allow-list in `deny.toml`, and CI SHALL run that check.

Verify: A
Status: active
Source: CON-27

#### Scenario: Policy present
- **WHEN** the repository check test runs
- **THEN** `deny.toml` exists with a licence allow-list and the CI workflow invokes `cargo deny`

### Requirement: BUILD-007 Walking skeleton in every mode
The walking skeleton (generate a tile, plan a frame, execute it, produce pixels) SHALL run in every run mode that can render: `test-render` and `editor --offscreen` (the window arrives in M2) SHALL produce the same frame for the same options, and `generate` SHALL print the content hash and cache key of a tile and write its height map and the face net; options a mode does not support SHALL be rejected, not ignored.

Verify: C
Status: active
Source: CON-01, ADR 0008

#### Scenario: Same frame
- **WHEN** the tiles scene is rendered by `test-render` and by `editor --offscreen` with the same options
- **THEN** the two PNGs are identical

#### Scenario: Generate
- **WHEN** `generate --tile 3,3,5,2` runs with output paths
- **THEN** it prints a content hash and a cache key and writes the height map and the face net

#### Scenario: Unsupported option
- **WHEN** `generate` is given `--repro`
- **THEN** it fails with a message naming the modes that support it

### Requirement: BUILD-008 No finished change stays unarchived
The repository check SHALL fail when a directory of `openspec/changes/` (other than `archive/`) has a `tasks.md` whose tasks are all checked off, naming the change: a finished change is archived, not left beside the open ones. A change with an unchecked task, or without a task list, SHALL pass.

Verify: A
Status: active
Source: housekeeping skill (coder part), CON-26

#### Scenario: Finished change left behind
- **WHEN** a change directory has a `tasks.md` with every task `- [x]`
- **THEN** the check names that change and fails

#### Scenario: Work in progress
- **WHEN** a change directory has at least one `- [ ]` task
- **THEN** the check passes it

### Requirement: BUILD-009 No conflict markers in tracked files
`cargo xtask spec-lint` SHALL fail when a tracked text file contains git conflict markers, naming the file and the line. A line counts as an opening marker when it is `<<<<<<<` or starts with `<<<<<<< `, as a closing marker likewise with `>>>>>>>`, and as a separator when it is exactly `=======`. A file fails when it holds an opening marker, then a separator, then a closing marker (a full block), or an opening or closing marker followed by a space and a ref name alone. A lone `=======` line (a markdown setext underline) SHALL pass. Files that are not UTF-8, contain a NUL byte or exceed 2 MiB SHALL be skipped.

Verify: A
Status: active
Source: CON-26, PR #222 incident

#### Scenario: Full block
- **WHEN** a tracked file has an opening marker, a separator and a closing marker in order
- **THEN** spec-lint fails and names the file and the line of the opening marker

#### Scenario: Setext underline
- **WHEN** a markdown file has a line `=======` under a heading and no other marker
- **THEN** the check passes it

#### Scenario: Binary file
- **WHEN** a tracked file contains a NUL byte
- **THEN** the check skips it

#### Scenario: Lone named marker
- **WHEN** a tracked file has a line `<<<<<<< ` followed by a ref name and no separator
- **THEN** the check fails and names that line

### Requirement: BUILD-010 Log entries are fragments
A new log entry SHALL be a file `docs/log.d/<YYYY-MM-DD>-<id>.md` holding exactly one entry line, `- YYYY-MM-DD **title** ...`; `<id>` is lower-case letters, digits, `-`, `_` or `.`, starting with a letter or digit. `cargo xtask spec-lint` SHALL fail on a fragment with a bad name, with other than one non-blank line, with a line that is not an entry, or with an entry date that differs from the file-name date, naming the file and the reason. `cargo xtask log [--since YYYY-MM-DD] [--limit N]` SHALL print the fragments and the entries of `docs/log.md` merged: date descending, then fragments before `log.md` lines of the same date, fragments by file name descending, `log.md` lines in file order. The output SHALL be deterministic.

Verify: A
Status: active
Source: housekeeping skill (root causes), PR #222 incident

#### Scenario: Fragment accepted
- **WHEN** `docs/log.d/2026-10-17-x.md` holds one line `- 2026-10-17 **x** ...`
- **THEN** spec-lint passes it

#### Scenario: Malformed fragment
- **WHEN** a fragment holds two entries, or its entry date differs from its file-name date, or its name is not `YYYY-MM-DD-<id>.md`
- **THEN** spec-lint fails and names the file and the reason

#### Scenario: Merged output
- **WHEN** fragments and `docs/log.md` entries of several dates exist
- **THEN** the output is newest first and identical for any read order

#### Scenario: One day, several fragments
- **WHEN** three fragments carry the same date
- **THEN** they print by file name descending, before the `log.md` lines of that date

### Requirement: BUILD-011 Fast edit-to-test build
On Windows the workspace SHALL link with `rust-lld` (`.cargo/config.toml`, target `x86_64-pc-windows-msvc`), and the dev profile SHALL build workspace crates with `debug = "line-tables-only"` and dependencies with `debug = false`, so backtraces keep file and line while local variables need a full-debug profile.

Verify: A
Status: active
Source: SPIKE-15

#### Scenario: Configuration present
- **WHEN** the repository check test reads `.cargo/config.toml` and `Cargo.toml`
- **THEN** the linker, `debug = "line-tables-only"` in `[profile.dev]` and `debug = false` in `[profile.dev.package."*"]` are all set

#### Scenario: Edit-to-test build time
- **WHEN** one line of a function body is edited and `cargo nextest run --no-run` builds the edited crate and every crate above it (`CARGO_BUILD_JOBS=10`, Rasierklinge, quiet, median of 3)
- **THEN** the frame crate edit builds in about 49 s (baseline 89 s) and the app crate edit in about 15 s (baseline 31 s)

### Requirement: BUILD-012 Every test run keeps its full output
Every `cargo xtask test`, `test fast` and `test gpu [--real]` run SHALL write the full output of its build step and of its nextest run (stdout and stderr, line by line in arrival order, including the stdout and stderr of every failing test and the run summary) to `target/test-logs/<command>-<YYYYMMDD>-<HHMMSS>.log`, where `<command>` is `test`, `test-fast`, `test-gpu` or `test-gpu-real` and the time is UTC (a second run in the same second gets `-2`, `-3`, ...). The command SHALL print `full output: <path>` as its last line, whether the run passed or failed, also when the build step failed. It SHALL keep the newest 20 logs of each command and delete older ones of that command (a log of another command, and any other file in the folder, is left alone). The console output is unchanged apart from the colour nextest drops when its output is not a terminal.

Verify: A
Status: active
Source: opus-planner (2026-10-18), BUILD-003

#### Scenario: A failing run
- **WHEN** a test of a `test fast` run fails
- **THEN** the log holds the build lines, the failing test's name and its output and the summary, the console shows the same, and the last line is `full output: target/test-logs/test-fast-<date>-<time>.log`

#### Scenario: Rotation
- **WHEN** 25 logs of `test` and 3 of `test-gpu` exist and another `test` run starts
- **THEN** the newest 20 of `test` remain (the new one among them), the 3 of `test-gpu` and `test-gpu-real` are untouched, and other files stay

#### Scenario: Naming
- **WHEN** a `test gpu --real` run starts at 2026-10-18 20:30:15 UTC
- **THEN** its log is `target/test-logs/test-gpu-real-20261018-203015.log`, and `test` is told apart from `test-gpu-...` by the date and time after the command

### Requirement: BUILD-014 Process budgets are reported by the tools
`cargo xtask test fast` SHALL print, after every run whose build succeeded, its edit-to-first-test time (the wall time of the build step plus the nextest run) with the two parts, and SHALL print `BUDGET EXCEEDED: edit-to-first-test <s> > 30 s (...)` when the total is over 30 s (the owner's budget of 2026-10-18; exactly 30 s is within it): a warning, never a failure. `cargo xtask ci-stats [--runs N]` SHALL read the last N (default 20) completed runs on `main` with `gh` and print, for each CI job, the median and the p90 of its duration (nearest rank; jobs that ended `success` or `failure` count, skipped and cancelled jobs do not) and the number of runs, and SHALL print `BUDGET EXCEEDED: CI median of '<job>' <m> min > 16 min` for each job whose median is over 16 min (the owner's budget of 2026-10-18); it does not fail on it. The tools only report: an exceeded budget becomes a Tracked item in `docs/status.md` (the `housekeeping` skill says so).

Verify: A
Status: active
Source: owner (2026-10-18; #315), BUILD-003, BUILD-011, SPIKE-15

#### Scenario: Edit-to-first-test
- **WHEN** a `test fast` run builds for 12.0 s and tests for 9.5 s, and another builds for 49.0 s and tests for 13.2 s
- **THEN** the first prints `edit-to-first-test 21.5 s (build 12.0 s + tests 9.5 s; budget 30 s)` and the second `BUDGET EXCEEDED: edit-to-first-test 62 s > 30 s`; both runs keep their exit status

#### Scenario: CI median
- **WHEN** one job took 15, 15, 15, 20 and 30 min and another 14, 17, 18, 19 and 25 min over five runs
- **THEN** the first reports median 15.0 min and p90 30.0 min without a flag, and the second median 18.0 min with the `BUDGET EXCEEDED` line, and no other line is flagged

### Requirement: BUILD-013 The owner's bless is applied by a command, on every adapter
`cargo xtask bless` SHALL list the candidate goldens of every adapter: those of `target/review/candidates/<adapter>/<name>.png` and of each `--extra <dir>` (`<dir>/<adapter>/<name>.png`, a later source replacing an earlier one), leaving out a candidate byte-equal to its golden. `cargo xtask bless --apply <name>[,<name>]... --approved-by <who>` SHALL promote the candidates of exactly the named goldens on every adapter that has one to `tests/goldens/<adapter>/<name>.png`, and remove the names from `tests/goldens/pending.txt`. `--apply` without a non-blank `--approved-by`, an unknown argument, and `--approved-by` without `--apply` SHALL be errors. A name without a candidate, or a candidate whose `driver.txt` (beside it) differs from the recorded `tests/goldens/<adapter>/driver.txt` (or has none recorded, or the candidate has none beside it where one is recorded), SHALL be an error naming it, and then nothing is written; an I/O error while copying stops there and says what was copied. `driver.txt` is never copied. The command does not decide what is blessed: the `bless` skill (ADR 0017) calls it only for what the owner approved.

Verify: A
Status: active
Source: opus-planner (2026-10-18), ADR 0017, TEST-022

#### Scenario: Three adapters from three sources
- **WHEN** the RTX candidate is in `target/review/candidates/`, the WARP candidate in one `--extra` directory and the llvmpipe candidate in another, and the owner approved the golden
- **THEN** `bless --extra ... --extra ... --apply v --approved-by owner` writes the three goldens, leaves goldens not named alone and removes `v` from `pending.txt`

#### Scenario: Refusals write nothing
- **WHEN** one of three candidates comes from another driver than the recorded one, or a name has no candidate
- **THEN** the command fails naming `<adapter>/<name>` and the reason, and the adapters that were fine are not written either

### Requirement: BUILD-015 One test binary per tier where it shortens the edit-to-test build
The crates whose conversion cuts the build of an edit of the chain by more than about 10 % (median of three edits on a machine with the same load, SPIKE-15 step 3; the build of an edit relinks the test binaries of every crate above it, so the chain, not one crate, is measured) SHALL build its integration tests as one binary per tier: `autotests = false` in its `Cargo.toml`, and a `[[test]]` for each tier it has, named `it` (tier A), `oracle_all` (tier B) and `gpu_all` (tier C) after the tier-prefix rule of BUILD-003, each of `tests/<name>.rs` including the crate's test files of that tier as modules (`#[path = "<file>.rs"] mod <file>;`, in the file's own directory, so `support/` and `data/` paths keep working). A new test file SHALL be added to the aggregator of its tier. The test names gain the module as prefix (`<file>::<test>`); the tier filters of BUILD-003 (`binary(/^gpu_/)`, `binary(/^oracle_/)`) are unchanged. The `real-gpu` profile of `.config/nextest.toml` SHALL name the heavy tests of such a binary by package, binary and module (`package(=planet) and binary(gpu_all) and test(/^(gpu_flight|...)::/)`), and the xtask guard SHALL check that every binary named there is a test file or a `[[test]]` and every module named is a test file. The crates done: `generators`, `cache`, `streaming` and `app` (a generators edit rebuilt 58.6 s before, 38.8 s with `generators` and `app` converted, 33.7 s with all four, 13 % below the two, at 35 to 36 % system load; an app edit 11.3 s to 6.5 s); `frame` and `render` followed after the removal of the vertex render path (#304): a `frame` edit rebuilt the test binaries of `frame`, `render`, `editor` and `app` in 33.5 s (median of 32.6, 33.5, 34.8) before and 10.7 s after (10.0, 10.7, 11.2), 68 % below, on the same machine one after the other. `render` has `it`, `oracle_all` and `gpu_all`, `frame` `it` and `oracle_all`.

Verify: A
Status: active
Source: SPIKE-15 (step 3), BUILD-003, BUILD-011, owner and opus-planner (2026-10-18)

#### Scenario: Tiers are still selected by name
- **WHEN** `cargo xtask test fast`, `test` and `test gpu` select tests of a converted crate
- **THEN** tier A is the `it` binary, tier B `oracle_all` and tier C `gpu_all`, no test is lost (each crate's test count is the number of its `#[test]` functions) and the tier-A filter `not binary(/^gpu_/) and not binary(/^oracle_/)` still matches `it`

#### Scenario: Every test file is in its aggregator
- **WHEN** a crate with `autotests = false` has a test file that no aggregator declares, declares twice, declares in the aggregator of another tier or only in a comment, or an aggregator declares a file that does not exist
- **THEN** the xtask guard fails naming the crate and the aggregator

#### Scenario: No test is lost
- **WHEN** `cargo xtask test` (tier B, `oracle_test_layout` of `testkit`) compares the tests `cargo nextest list` reports for the `it`, `oracle_all` and `gpu_all` binaries of a converted crate with the `#[test]` functions of its test files (an `#[ignore]`d one is not listed)
- **THEN** the two sets are equal as `<file>::<function>`, and a test file left out of an aggregator fails the comparison naming the missing tests

#### Scenario: The heavy GPU tests keep their group
- **WHEN** `test gpu --real` runs under the `real-gpu` profile
- **THEN** `cargo nextest show-config test-groups --profile real-gpu` lists the `gpu_flight`, `gpu_stream_perf`, `gpu_water_coverage` and `gpu_world_field` tests of `planet::gpu_all`, the `gpu_lit`, `gpu_water`, `gpu_atmosphere` and `gpu_compact_blocks` tests of `planet-render::gpu_all`, the `oracle_lit` tests of `planet-render::oracle_all` and the `oracle_concurrent_source` tests of `planet-cache::oracle_all` in the group `real-gpu-heavy`, and a misspelt binary or module name fails the xtask guard

#### Scenario: A test that starts itself again
- **WHEN** a test runs its own binary again by test name (`pack::a_writer_process_that_is_killed_releases_its_lock`)
- **THEN** it passes the helper's name with its module prefix (`module_path!()` without the crate), so it works in the per-tier binary

### Requirement: BUILD-016 The pre-push gate is one command
`cargo xtask prepush` SHALL run, in this order and stopping at the first failure, `cargo xtask check` (fmt, clippy, layering), `cargo xtask test fast`, and `cargo xtask test gpu` (tier C on the software adapter) when the branch's changed files (the diff against `origin/main` and the working tree, as `test fast` reads them) include a golden-producing path. A path is golden-producing when it is in `crates/render`, `crates/frame`, `crates/generators`, `crates/core`, `crates/streaming`, `crates/world-def`, `crates/cache`, `crates/app` or `crates/testkit`, ends in `.wgsl`, is under `tests/goldens/`, or is `Cargo.toml`, `Cargo.lock` or under `.cargo/`. `crates/editor`, `crates/perf`, `xtask/`, `docs/`, `openspec/`, `.github/` and root files are not. The command prints which of the two cases applies before it starts, the name of each step as it begins, and at the end one line with the time of every step and the total (`prepush OK in <s> s: check ok <s> s, test fast ok <s> s[, test gpu ok <s> s]`, or `prepush FAILED at <step> after <s> s: ...`); a failure ends the command with a non-zero status. Golden differences that appear on the lavapipe adapter only stay CI's job (ADR 0061).

Verify: A
Status: active
Source: owner and opus-planner (2026-10-19)

#### Scenario: Which paths need the GPU tier
- **WHEN** the changed paths are `crates/render/src/terrain.rs`, `crates/frame/src/look/water.rs`, a `.wgsl` file, `tests/goldens/software/x.png` or `Cargo.lock`, and when they are only `docs/status.md`, `crates/editor/src/lib.rs`, `crates/perf/src/calibration.rs`, `xtask/src/main.rs`, `CLAUDE.md` or `.github/workflows/ci.yml`
- **THEN** the first set includes `test gpu` and the second does not, backslash separators included

#### Scenario: One line verdict
- **WHEN** `check` took 20 s and `test fast` 31 s without a golden path, or `test fast` failed after 9 s
- **THEN** the lines are `prepush OK in 51 s: check ok 20 s, test fast ok 31 s; test gpu skipped (no golden-producing path changed)` and `prepush FAILED at test fast after 29 s: check ok 20 s, test fast FAILED 9 s`

### Requirement: BUILD-017 A runner fault in the tool install gets one second attempt
Each `taiki-e/install-action` step of `.github/workflows/ci.yml` SHALL be followed by a second identical step (same `tool:` list) that runs only when the first one failed (`if: steps.install.outcome == 'failure'`), the first having `id: install` and `continue-on-error: true`. The failure it covers is the runner fault "bash startup failure" (actions/partner-runner-images#169), not a code failure; any other install failure fails the second attempt too and stays visible.

Verify: A
Status: active
Source: opus-planner (2026-10-19), actions/partner-runner-images#169

#### Scenario: The install is paired
- **WHEN** `ci.yml` is read
- **THEN** every install step has its retry with the same tools right after it, the first with `continue-on-error`, the retry with the failure condition

### Requirement: BUILD-018 Tier C on Windows runs in two halves
The CI job that runs tier C SHALL run its Windows leg as two jobs, `windows (WARP) C 1/2` and `windows (WARP) C 2/2`, each `cargo xtask test gpu -- --partition count:N/2`, so that every tier-C test runs in exactly one half and each half's job time stays under the 16 min budget of BUILD-014. The Linux leg SHALL run whole. The artifacts of a half SHALL carry its number (`review-Windows-N`, `test-logs-Windows-gpu-N`).

Verify: A
Status: active
Source: opus-planner (2026-10-19), BUILD-005, BUILD-014

#### Scenario: Two halves
- **WHEN** the repository check test reads the workflow file
- **THEN** the tier-C matrix holds a Windows entry for part 1 and one for part 2, the test command takes `--partition count:{part}/2`, and the artifact names carry the part

### Requirement: BUILD-019 Test fast keeps a series of its times
After every `cargo xtask test fast` run whose build succeeded, the command SHALL append one tab-separated line to `target/test-logs/test-fast-series.tsv`: the UTC time (`YYYY-MM-DDTHH:MM:SSZ`), the selection (`every crate` or the selected crate names joined by `+`), the build seconds, the test seconds, their sum (one decimal each) and `pass` or `fail`. A file that cannot be written SHALL be a warning, not a failure. The log rotation of BUILD-012 SHALL leave the file alone.

Verify: A
Status: active
Source: opus-planner (2026-10-19), BUILD-014, BUILD-012, BUILD-003

#### Scenario: One line per run
- **WHEN** a run of the crates `planet-frame` and `planet-render` builds for 12.0 s, tests for 9.5 s and passes at 2026-10-19 17:25:38 UTC, and a later run of every crate builds for 1.3 s, tests for 35.1 s and fails
- **THEN** the series file holds `2026-10-19T17:25:38Z`, `planet-frame+planet-render`, `12.0`, `9.5`, `21.5`, `pass` and then a line with `every crate`, `1.3`, `35.1`, `36.4`, `fail`, and rotating the logs of `test-fast` removes neither

### Requirement: BUILD-020 CI evidence is recorded per job
`cargo xtask ci-stats --record --since YYYY-MM-DD [--until YYYY-MM-DD]` SHALL read, with `gh`, every completed run created in the range (pull requests and pushes, every branch) and append one tab-separated row per job that ended `success` or `failure` to `docs/perf/ci-history.tsv`: the run id, the creation time, the event, the branch, the job name, the conclusion, the duration in seconds and the names of the job's failed steps joined by `+` (empty for a success). A run whose id is already in the file SHALL be skipped, so that a repeated call adds nothing. The file SHALL start with a header row. A missing `--since` SHALL be an error.

Verify: A
Status: active
Source: opus-planner (2026-10-19), BUILD-014

#### Scenario: Rows and the skip
- **WHEN** run 101 (a pull request on `change/x`) has the jobs `linux (lavapipe) A+B` (success, 300 s) and `windows (WARP) C 1/2` (failure, 60 s, failed step `test gpu (tier C, software adapter)`) and a skipped job, and the file already holds run 99
- **THEN** two rows for run 101 are appended, the failed one ending in `test gpu (tier C, software adapter)`, the skipped job has none, and recording run 101 and run 99 again adds no row

### Requirement: BUILD-021 A change with no crate path selects no crate
`cargo xtask test fast` SHALL select the tier-A tests of the crates the changed files name and every crate above them (BUILD-003), of `xtask` for a file under `xtask/` or `.github/`, and of every crate when the list of changed files is empty (nothing changed) or holds a Cargo manifest, `Cargo.lock`, a file under `.cargo/` or `.config/`, or a file of `crates/testkit`, `crates/perf` or any other directory under `crates/` that is not in the crate chain (a crate added without being placed in it has no known crate above it). A file that a tier-A test reads selects that test's crate: `deny.toml`, `openspec/changes/`, `tests/goldens/` and any `.sh`, `.ps1`, `.bat` or `.cmd` file (in any letter case) select `xtask`, `clippy.toml` selects `core`, and `spikes/mouse-look/` selects `frame`. A list of changed files none of which is any of these (for example only files under `docs/`, the rest of `openspec/`, `.claude/`, other `spikes/`, root `*.md` files or other tool configuration) SHALL select no crate: `test fast` SHALL say so and succeed without building, and `cargo xtask prepush` SHALL run `spec-lint` instead of `check` and `test fast` (and `test gpu` when the diff holds a golden-producing path, BUILD-016).

Verify: A
Status: active
Source: opus-planner (2026-10-19), BUILD-003, BUILD-016, CLAUDE.md (docs-only changes)

#### Scenario: Docs only
- **WHEN** the changed files are `docs/status.md` and `openspec/specs/viewer/spec.md`
- **THEN** no crate is selected, `test fast` runs nothing, and `prepush` runs only `spec-lint`

#### Scenario: A crate path, the workspace, nothing
- **WHEN** the changed files are `docs/a.md` and `crates/streaming/src/lib.rs`, or `docs/a.md` and `Cargo.lock`, or none
- **THEN** the first selects `planet-streaming` and the crates above it, the second and the third every crate

### Requirement: BUILD-022 Prepush lints the branch merged onto main
`cargo xtask prepush` SHALL run, before its other steps, `spec-lint` on the tree that merging `HEAD` with the fetched `origin/main` makes (`git merge-tree --write-tree`, checked out in a temporary worktree that is removed afterwards), so that a requirement ID, a spec or a change that another PR added meanwhile collides in prepush and not on main. The branch, its index and its working tree SHALL NOT be changed. A branch that already holds `origin/main` SHALL pass this step without a merge; a fetch that fails SHALL print that the step is skipped and pass; a branch that does not merge cleanly SHALL fail naming `origin/main` and the merge to make first. The step SHALL be in every prepush plan, docs-only ones included.

Verify: A
Status: active
Source: opus-planner (2026-10-20), BUILD-016, BUILD-021

#### Scenario: The plan
- **WHEN** the plan of a docs-only diff, of a crate diff and of a diff with a golden path is listed
- **THEN** each starts with `merged spec-lint`, followed by the steps BUILD-016 and BUILD-021 name

#### Scenario: The tree id
- **WHEN** the first line of `git merge-tree --write-tree` is a 40 or 64 digit hexadecimal id, and when it is a conflict message or empty
- **THEN** the first is taken as the tree and the others are not

### Requirement: BUILD-023 The hot files stay within their caps
`cargo xtask spec-lint` SHALL fail when a file every session reads at its start is over its cap (ADR 0075 point 1): `docs/status.md` 15 KiB, `docs/learnings.md` 20 KiB, `docs/spikes.md` 10 KiB, `CLAUDE.md` 7,514 B and `docs/constitution.md` 8,909 B (its size after the latest owner amendment, ADR 0079), with a line end counted as one byte, naming the file, its size and the cap. A cap changes only by a new ADR. A test that cites a requirement ID that no spec holds already fails the lint (TEST-001); this requirement adds the caps.

Verify: A
Status: active
Source: opus-planner (2026-10-21, M7 H1), ADR 0075

#### Scenario: Over and at the cap
- **WHEN** each hot file in turn is one byte over its cap, and when every file is at its cap
- **THEN** the first fails naming only that file with its size and cap, the second passes, a missing or uncapped file is no error, a CRLF line end counts as one byte, and the repository's own hot files are within their caps

### Requirement: BUILD-024 Requirement statements stay short and impersonal
`cargo xtask spec-lint` SHALL fail a requirement statement (the lines before the first `Verify:`, `Status:`, `Source:` or `Review:` line or scenario) of more than 120 words, or with a first-person pronoun (`FIRST_PERSON` in `xtask/src/speclint.rs`; ADR 0055 point 4), naming the requirement and its count. Statements already longer are listed with their counts in `xtask/src/long_statements.txt`. The lint SHALL fail when a listed statement grew, shrank (lower its line), fits in 120 words (delete its line) or has no requirement, and when the list holds more than the 191 entries it started with: the list only shrinks. A removed requirement and the scenarios are not read.

Verify: A
Status: active
Source: opus-planner (2026-10-22, M7 H2), ADR 0055

#### Scenario: The word limit
- **WHEN** statements of 120, 121 and 130 words (two lines) are linted, with and without a listed entry of 200 words for the same ID
- **THEN** only the 121- and 130-word statements fail, naming the count and the limit, a listed ID passes up to its listed words and fails above them, and another ID is not covered by the entry

#### Scenario: First person
- **WHEN** a statement says "we", "I", "our" or "my", and another says "I/O" and "52,000 us"
- **THEN** the first fails and the second passes; a scenario or a removed requirement with the same words is not an error

### Requirement: BUILD-025 A maintained accepted ADR carries a Maintained line
`cargo xtask spec-lint` SHALL fail when the branch changes an accepted ADR (`docs/adr/NNNN-*.md` whose status line starts with `accepted` at the merge base with `origin/main`) beyond its status line and the ADR gains no line `Maintained: <YYYY-MM-DD>, <links|values|terms|erratum>: <what>` (ADR 0078 point 7), naming the ADR. The lint reads the files at the merge base and at `HEAD`. It SHALL pass when git or the base is missing (a fresh clone), for an ADR new on the branch, for one that is not accepted at the base, and when only the status line changed.

Verify: A
Status: active
Source: opus-planner (2026-10-23), ADR 0078

#### Scenario: What passes and what fails
- **WHEN** an accepted ADR changes only its status line, changes its body and gains a well-formed Maintained line, changes its body without one, or gains a Maintained line with a bad date or kind; and a proposed ADR changes its body
- **THEN** the first two pass, the third and fourth fail naming the ADR, the proposed ADR is not checked, and no base gives no error

### Requirement: BUILD-026 Heavy commands queue on one machine-wide lock and obey the owner's quiet flag
`cargo xtask test` (every form), `prepush`, `perf`, `capture`, `accept`, `store` and `heavy <command>` SHALL take one lock outside every worktree (the file `heavy.lock` in the state directory: `PLANET_STATE_DIR`, else `%LOCALAPPDATA%\planet`, else `$XDG_STATE_HOME/planet`, else `~/.local/state/planet`) before they run, and a second one SHALL wait for the first instead of running beside it, saying "waiting for <holder>" (what, process, worktree) at the start of the wait and every 30 seconds. The lock SHALL be an operating-system file lock, so that it is released when the holder's process ends however it ends.

Verify: A
Status: active
Source: opus-planner (2026-10-25): three lanes' runs each got about one core; the owner needs to clear the machine without messaging anyone

#### Detail
While the file `quiet` exists in the state directory every heavy command SHALL wait before it takes the lock and say that the flag is up (`cargo xtask quiet on|off|status` makes and removes it); a command already running SHALL finish. A flag older than 60 minutes SHALL be ignored, with a warning that names its age (the owner can forget it), and `quiet on` SHALL renew it. A heavy command SHALL set `PLANET_HEAVY_HELD` to its process id for its children, and a process whose `PLANET_HEAVY_HELD` names the process of the holder note SHALL take nothing and wait for nothing (a stale or wrong value is no pass). A state directory that cannot be used, or `PLANET_HEAVY_LOCK=off`, SHALL run the command without the lock and say so, and a heavy command run inside another in the same process SHALL take the lock once. `check`, `spec-lint`, `layering` and the other light commands SHALL not take it.

#### Scenario: A queue
- **WHEN** a second heavy command starts while the first runs, and then the first ends
- **THEN** the second says it is waiting for the first's holder note and runs once the first has let go, and no message is printed when nobody waited

#### Scenario: The quiet flag
- **WHEN** the quiet flag file exists and a heavy command starts, and the file is then removed
- **THEN** the command waits and says the flag is up, and goes once the flag is down; `quiet on`, `status` and `off` raise, show and lower it

#### Scenario: A flag the owner forgot
- **WHEN** the flag is two hours old, and then `quiet on` is run
- **THEN** the first is ignored with a warning naming the age and the second holds again

#### Scenario: A child of the holder
- **WHEN** a process whose `PLANET_HEAVY_HELD` is the holder's id asks for the lock, and then ones with another id, an empty or a non-numeric value, and none
- **THEN** the first goes at once and holds no lock of its own, the others wait, and after the holder is done the id is no pass

### Requirement: BUILD-027 No new Detail blocks under a requirement
`cargo xtask spec-lint` SHALL fail a requirement with a `#### Detail` block unless its ID is in `xtask/src/detail_blocks.txt`, fail a listed ID whose requirement has no such block (delete its line), and fail the list when it holds more than the 10 entries it started with: the list only shrinks. A removed requirement is not read.

Verify: A
Status: active
Source: opus-planner (2026-10-26), BUILD-024

#### Scenario: A Detail block
- **WHEN** a requirement with a `#### Detail` block is linted with and without its ID in the list, and a listed ID has no block
- **THEN** only the unlisted block fails, naming the requirement, and the listed ID without a block fails with the instruction to delete its line

#### Scenario: The list only shrinks
- **WHEN** the list holds more entries than it started with
- **THEN** the lint fails

### Requirement: BUILD-028 Retired terms stay out of the current specs and docs
`cargo xtask spec-lint` SHALL read the terms of the first column of the "Former terms" table in section 8 of `docs/domain.md` and fail a mention, as a whole phrase in any case with `-` and `_` read as a space, in the main specs, `domain.md` above section 8, `status.md`, `roadmap.md`, `README.md` and `docs/briefings/`, naming file, line, term and the row's "Now" column. A row marked *code removal pending* SHALL be reported as a count of warnings. A single-word term SHALL be skipped unless the row says `Lint: all`; `Lint: a; b` adds spellings and `Lint: none` skips the row.

Verify: A
Status: active
Source: owner (2026-10-30), opus-planner, docs/domain.md section 8

#### Scenario: A planted term
- **WHEN** a checked file holds a retired term whose row is not pending
- **THEN** spec-lint fails naming the file, line, term and the "Now" text, and a mention in an ADR, an archive or the log is not read

#### Scenario: A pending term
- **WHEN** a checked file holds a term whose row says code removal pending
- **THEN** spec-lint passes and prints the count of such mentions

### Requirement: BUILD-029 `cargo xtask terrain-score`
`cargo xtask terrain-score [--store DIR] [--dem DIR] [--sites FILE] [--out DIR] [--maps]` SHALL run the scorecard (TEST-079) in release under the heavy lock (BUILD-026), on the CPU only; `--maps` also writes a 512 px PNG per descriptor per raster under `maps/`. Defaults: the linked production store, `C:/DEV/SeriousPlanetData/dem/glo30-samples`, `crates/app/data/terrain-score-sites.toml`, and `target/review/terrain-score/<date>`. It SHALL print the report's path and its wall time, take at most 60 s on the reference machine, and refuse a missing store or tile directory by name.

Verify: A
Status: active
Source: SPIKE-57 (about 1 minute), BUILD-002, BUILD-026

#### Scenario: Arguments
- **WHEN** the command is parsed with no options, with every option, and with an unknown option
- **THEN** the first runs the example with the defaults, the second passes each option through, and the third is refused naming the known options

#### Scenario: Missing input
- **WHEN** the tile directory does not exist
- **THEN** the command fails naming the directory and the `--dem` option

### Requirement: BUILD-030 The gate records the load in loaded mode
`cargo xtask perf` with `--loaded` SHALL run the readiness gate (ADR 0020) with its hard conditions (AC power, power scheme, reference adapter, GPU temperature, GPU throttling) and SHALL record the load conditions (build and test processes, CPU load, GPU utilisation, the CPU calibration of ADR 0065) as readings instead of refusing on them. `--machine-ready` SHALL stay the owner's override, and `--loaded` with `--machine-ready` SHALL be refused. Quiet mode SHALL behave as before.

Verify: A
Status: active
Source: ADR 0087 consequences, ADR 0020, owner 2026-10-31

#### Scenario: A loaded machine
- **WHEN** the gate runs in loaded mode with a build process, 40 % CPU load and a calibration over its limit
- **THEN** it passes, and the readings are in the load record

#### Scenario: A hard condition
- **WHEN** the gate runs in loaded mode on battery, or with the GPU over its temperature limit
- **THEN** it refuses naming the reading and the limit

### Requirement: BUILD-031 The loaded verdict rules
A pure function SHALL give a time verdict measured under load per ADR 0087: a reading under budget by at least 5 % is a loaded pass; over budget is inconclusive, never a fail; within the margin goes to a window. The frame time with streaming, a cross-session regression comparison and a milestone acceptance run SHALL need a quiet window. A CPU-timed scene SHALL count only when pinned, or with a 30 % margin. A GPU timing SHALL carry the +18 % note.

Verify: A
Status: active
Source: ADR 0087 points 1 to 5

#### Scenario: Margins
- **WHEN** readings of 94 %, 97 % and 110 % of a GPU budget are judged
- **THEN** they are a loaded pass with the note, a window case and inconclusive

#### Scenario: CPU and exclusive kinds
- **WHEN** an unpinned CPU reading at 80 % of its budget and a frame p99 with streaming are judged
- **THEN** the first needs 70 % or less to be a loaded pass, and the second needs a quiet window

### Requirement: BUILD-032 The report states loaded or quiet
Every row of a perf report SHALL state `loaded` or `quiet` and carry the load readings (CPU load, competing load, the top processes, GPU utilisation, CPU performance %, calibration), and a loaded row SHALL carry its verdict of BUILD-031.

Verify: A
Status: active
Source: ADR 0087 points 1 and 2

#### Scenario: Rows
- **WHEN** a loaded and a quiet session are reported
- **THEN** each row names its mode and readings, and the loaded rows their verdicts

### Requirement: BUILD-033 The A/B helper
`cargo xtask perf --ab <ref> <perf command>` SHALL run the command in the working tree and in a worktree of `<ref>` in alternating order for N rounds (default 5), SHALL retry a gate refusal up to a cap and report how often and why, SHALL keep every run's log, and SHALL print one table: per metric both medians, the change in %, the verdict against the 5 % margin and the rule that judges the metric under load (the metric table of the design: tile-ready and frame p99 by default). It SHALL not write the store or the main tree's `perf/logs`.

Verify: A
Status: active
Source: ADR 0087, the job-system A/B of 2026-10-31

#### Scenario: Rounds
- **WHEN** the helper runs 5 rounds with fake runs of two builds
- **THEN** the order alternates, each log is kept, and the table gives both medians and the change
