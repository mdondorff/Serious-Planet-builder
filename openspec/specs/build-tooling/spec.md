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
Source: ADR 0009, report §16, ADR 0020, planner inbox #44 (2026-11-01), opus-planner (2026-11-03, #1108)

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
- **AND** a test with a declared limit in `.config/nextest.toml` (its measured durations and the planner's approval in a comment beside it) terminates after 450 s (5 x 90 s) instead; today the window flight, the coastal flight, the lit coast view, the edit-latency smoke scene, the two stills scenes, the small-world window gate and the draw-flag loop test of `gpu_skeleton`; no test is skipped, retried or changed

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
- **THEN** it is in the test group `gpu-tier` of at most 8 processes at once, unless an earlier override puts it in a heavier group; the window, water-coverage, stream-perf, envelope-perf, edit-perf and stills tests and two loop tests of `gpu_skeleton` are in `heavy-window` of at most 2; the coastal flight, the real-window tests, the lit flight and lit streaming process tests and the draw-flag loop test of `gpu_skeleton` are in `warp-heavy` of 1, which comes first; tier A and B tests are not capped
- **AND** a machine with 8 cores or fewer (CI's runners) runs as before; on the 20-thread reference laptop the software tier C no longer runs about 20 rasterising processes at once (SPIKE-84: the 12 timeouts of 2026-10-20 and 3 load timeouts on 2026-10-31)

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
Continuous integration SHALL run check, spec-lint, licence check, tiers A to C and the run-mode smoke starts on Windows (software adapter WARP) and on Linux (software Vulkan lavapipe) for every pull request and every push to `main`. A change that touches only `docs/`, `openspec/`, `.claude/`, root `*.md` files, `spikes/**/results/**` or `spikes/README.md` (owner, 2026-10-16; the spike catalogue, 2026-11-04) SHALL run the light `docs` workflow (spec-lint on Linux) instead; both workflows SHALL name the same list of those paths, and any other file in a change runs the full workflow.

Verify: A
Status: active
Source: CON-26, report §9

#### Scenario: Workflow content
- **WHEN** the workflow file is parsed by the repository check test
- **THEN** it names both a Windows and a Linux runner and invokes `cargo xtask check`, `spec-lint`, `test` and `test gpu`

#### Scenario: Docs-only paths agree
- **WHEN** the repository check test reads both workflows
- **THEN** the paths the full workflow ignores equal the paths the docs workflow runs on, and include `spikes/**/results/**` and `spikes/README.md`

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
`cargo xtask prepush` SHALL run, in this order and stopping at the first failure, `cargo xtask check` (fmt, clippy, layering), `cargo xtask test fast`, and `cargo xtask test gpu` (tier C on the software adapter) when the branch's changed files (the diff against `origin/main` and the working tree, as `test fast` reads them) include a golden-producing path. A path is golden-producing when it is in `crates/render`, `crates/frame`, `crates/generators`, `crates/core`, `crates/streaming`, `crates/world-def`, `crates/cache`, `crates/app` or `crates/testkit` (but not an `examples/` or `benches/` file of them, nor a test file other than `tests/gpu_*.rs`), ends in `.wgsl`, is under `tests/goldens/`, or is `Cargo.toml`, `Cargo.lock` or under `.cargo/`. `crates/editor`, `crates/perf`, `xtask/`, `docs/`, `openspec/`, `.github/` and root files are not. The command prints which of the two cases applies before it starts, the name of each step as it begins, and at the end one line with the time of every step and the total (the scenario below); a failure ends the command with a non-zero status. Golden differences that appear on the lavapipe adapter only stay CI's job (ADR 0061).

Verify: A
Status: active
Source: owner and opus-planner (2026-10-19)

#### Scenario: Which paths need the GPU tier
- **WHEN** the changed paths are `crates/render/src/terrain.rs`, `crates/frame/src/look/water.rs`, a `.wgsl` file, `tests/goldens/software/x.png` or `Cargo.lock`, and when they are only `docs/status.md`, `crates/editor/src/lib.rs`, `crates/perf/src/calibration.rs`, `xtask/src/main.rs`, `CLAUDE.md` or `.github/workflows/ci.yml`
- **THEN** the first set includes `test gpu` and the second does not, backslash separators included; an `examples/` file, a `benches/` file and a test file other than `tests/gpu_*.rs` of the golden-producing crates do not either, and `gate --small` is not part of the run for them

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

### Requirement: BUILD-018 Tier C on Windows runs in four parts from one build
The CI job that runs tier C SHALL build the tier-C binaries once on Windows into a nextest archive (`windows (WARP) C build`, `test gpu --archive-out FILE`) and SHALL run them as four jobs, `windows (WARP) C 1/4` to `C 4/4`, each `test gpu --archive-file FILE -- --partition count:N/4` with no build of its own, so that every tier-C test runs in exactly one part and each part's job time stays under the 16 min budget of BUILD-014. The test groups (the serialised `warp-heavy` group among them) hold within each part. The Linux leg SHALL run whole. The artifacts of a part SHALL carry its number (`review-Windows-N`, `test-logs-Windows-gpu-N`).

Verify: A
Status: active
Source: opus-planner (2026-10-19; 2026-11-03, inbox #74: two halves took 21.2 and 20.6 min after #1103; 2026-11-03, options 2 and 4: with three parts one drew 878 s of serialised `warp-heavy` tests and ran 21-22 min, #1125, and each part spent 5-6 min building), BUILD-005, BUILD-014

#### Scenario: Four parts from one archive
- **WHEN** the repository check test reads the workflow file
- **THEN** the build job makes the archive and uploads it, the tier-C matrix holds a Windows entry for parts 1 to 4 that needs the build job and runs the archive with `--partition count:{part}/4`, and the artifact names carry the part

#### Scenario: The archive arguments
- **WHEN** `test gpu --archive-out FILE` and `--archive-file FILE` are given, alone, together, or with `test fast`
- **THEN** the first builds only the `gpu_all` binaries of `planet` and `planet-render` into FILE; both together and either with `test fast` are refused with their reason

### Requirement: BUILD-019 Test fast keeps a series of its times
After every `cargo xtask test fast` run whose build succeeded, the command SHALL append one tab-separated line to `target/test-logs/test-fast-series.tsv`: the UTC time (`YYYY-MM-DDTHH:MM:SSZ`), the selection (`every crate` or the selected crate names joined by `+`), the build seconds, the test seconds, their sum (one decimal each) and `pass` or `fail`. A file that cannot be written SHALL be a warning, not a failure. The log rotation of BUILD-012 SHALL leave the file alone.

Verify: A
Status: active
Source: opus-planner (2026-10-19), BUILD-014, BUILD-012, BUILD-003

#### Scenario: One line per run
- **WHEN** a run of the crates `planet-frame` and `planet-render` builds for 12.0 s, tests for 9.5 s and passes at 2026-10-19 17:25:38 UTC, and a later run of every crate builds for 1.3 s, tests for 35.1 s and fails
- **THEN** the series file holds `2026-10-19T17:25:38Z`, `planet-frame+planet-render`, `12.0`, `9.5`, `21.5`, `pass` and then a line with `every crate`, `1.3`, `35.1`, `36.4`, `fail`, and rotating the logs of `test-fast` removes neither

### Requirement: BUILD-020 CI evidence is recorded per job
`cargo xtask ci-stats --record --since YYYY-MM-DD [--until YYYY-MM-DD]` SHALL read, with `gh`, every completed run created in the range (pull requests and pushes, every branch) and append one tab-separated row per job that ended `success` or `failure` to `docs/tools/perf/ci-history.tsv`: the run id, the creation time, the event, the branch, the job name, the conclusion, the duration in seconds and the names of the job's failed steps joined by `+` (empty for a success). A run whose id is already in the file SHALL be skipped, so that a repeated call adds nothing. The file SHALL start with a header row. A missing `--since` SHALL be an error.

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

### Requirement: BUILD-026 Heavy commands queue on machine-wide slots and obey the owner's quiet flag
`cargo xtask test` (every form), `prepush`, `capture`, `accept`, `store` and `heavy <command>` SHALL take one of the heavy-run slots outside every worktree (`PLANET_HEAVY_SLOTS`, default 2) before they run, and a command that finds every slot held SHALL wait, saying "waiting for <holders>" (what, process, worktree, slot) at the start of the wait and every 30 seconds. `perf`, `gate` (but `gate --small`), `repro --window` and `heavy --exclusive <command>` SHALL wait until no slot is held and hold every slot while they run. A slot SHALL be an operating-system file lock, so that it is released when the holder's process ends however it ends.

Verify: A
Status: active
Source: opus-planner (2026-10-25): three lanes' runs each got about one core; the owner needs to clear the machine without messaging anyone; lead 2026-11-14 (queue order, overdue warning: a later waiter took the lock first; a hung holder blocks everyone without a word); owner 2026-11-14 ("Please do": two slots, exclusive timing and window runs; short jobs waited behind 30-60 min probes); lead 2026-11-15 (explorer #156: a docs-only prepush waited 45 min for a 5 s spec-lint; two later arrivals took both slots over older live tickets)

#### Detail
The slots are the files `heavy.lock` (slot 1, the single lock's name, so an older xtask shares it), `heavy.lock.1`, ... in the state directory: `PLANET_STATE_DIR`, else `SeriousPlanetData/tmp/heavy` in the nearest ancestor of the workspace that holds the data root (ADR 0121), else `%LOCALAPPDATA%\planet`, else `$XDG_STATE_HOME/planet`, else `~/.local/state/planet`. Each held slot has a holder note naming its slot, or "exclusive".
The data root comes before `%LOCALAPPDATA%` because the desktop app redirects `%LOCALAPPDATA%` for its sessions, and a terminal and a session must see one lock.
Waiters SHALL go in arrival order: a command that finds no slot it can take, or other waiters queued, SHALL take a ticket (a file in `tickets/` of the state directory named by its arrival time, held under an operating-system file lock, with a note beside it), and only the oldest live ticket SHALL try the slots (an ordinary one the first free slot, an exclusive one every slot); a newcomer SHALL queue behind the tickets even while a slot is free, so an exclusive waiter is never starved by later ordinary jobs. A prepush whose plan is spec-lint alone (a docs-only diff) SHALL take no slot and prepare no scratch folder. A ticket that cannot be opened (but one that is gone) SHALL count as live, so that its waiter keeps its place. A holder note that names no slot (a heavy command of an xtask older than the slots and the ticket queue, which takes `heavy.lock` without queueing: a `target/debug/xtask.exe` run directly) SHALL be named as such in the waiting message. A ticket whose file lock is free and that is older than 5 seconds belongs to an ended process and SHALL be removed by the next reader. Each holder that has held its slot longer than the overdue limit (`PLANET_HEAVY_OVERDUE_MIN`, default 90 minutes) SHALL be named in the waiters' messages as "overdue: <holder note>, held N min"; a slot SHALL never be broken automatically.
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

#### Scenario: Arrival order
- **WHEN** two commands queue behind a holder one after the other, and a third arrives while the lock is free but one is still queued
- **THEN** the first queued runs first, the second next, and the third waits saying how many are queued ahead; a dead waiter's ticket is removed

#### Scenario: An overdue holder
- **WHEN** a holder has held its slot 100 minutes with the limit at 90, beside another holder of 10 minutes
- **THEN** the waiters' message says "overdue: <its holder note>, held 100 min" and nothing about the other, and the waiters still wait

#### Scenario: Two slots
- **WHEN** two ordinary jobs start, and then a third
- **THEN** the first two run side by side in slots 1 and 2, the third waits naming both holders, and it takes the slot the first one frees

#### Scenario: An exclusive run
- **WHEN** `perf` starts while one slot is held, and an ordinary job arrives after it while the other slot is free
- **THEN** `perf` waits until no slot is held and then holds both, its notes say "exclusive", and the ordinary job runs only after `perf`

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
Every row of a perf report, and every perf log (the one-off logs `global-bake`, `coupled`, `coupled-make`, `envelope` and `edit-latency` included), SHALL state `loaded` or `quiet`; a loaded one SHALL carry the load readings (CPU load, competing load, the top processes, GPU utilisation, CPU performance %, calibration). A loaded row with a time reading and a budget (the streaming and terrain rows, the lit passes, the bake and coupled times, the gated edit rows) SHALL carry its verdict of BUILD-031.

Verify: A
Status: active
Source: ADR 0087 points 1 and 2, planner (2026-11-01, perf-one-off-mode)

#### Scenario: Rows
- **WHEN** a loaded and a quiet session are reported
- **THEN** each row names its mode and readings, and the loaded rows their verdicts

#### Scenario: One-off logs
- **WHEN** a one-off log is written in a quiet and in a loaded session
- **THEN** both open with their mode, the loaded one also with its load record, and a log with a time budget closes with its verdicts in the loaded case only

#### Scenario: Lit rows
- **WHEN** a loaded session reports a `lit/` row
- **THEN** its GPU passes and its frame p99 carry the verdicts of BUILD-031 against their budgets

### Requirement: BUILD-033 The A/B helper
`cargo xtask perf --ab <ref> <perf command>` SHALL run the command in the working tree and in a worktree of `<ref>` in alternating order for N rounds (default 5), SHALL retry a gate refusal up to a cap and report how often and why, SHALL keep every run's log, and SHALL print one table: per metric both medians, the change in %, the verdict against the 5 % margin and the rule that judges the metric under load (the metric table of the design: tile-ready and frame p99 by default). It SHALL not write the store or the main tree's `perf/logs`.

Verify: A
Status: active
Source: ADR 0087, the job-system A/B of 2026-10-31

#### Scenario: Rounds
- **WHEN** the helper runs 5 rounds with fake runs of two builds
- **THEN** the order alternates, each log is kept, and the table gives both medians and the change

### Requirement: BUILD-035 The duty idle between perf runs
`cargo xtask perf --ab` (BUILD-033), `cargo xtask perf reserve` (BUDG-012), `cargo xtask perf stream all` and `cargo xtask perf window` (BUILD-034) SHALL idle between two runs for half the previous run's wall time, at most 45 s, and print it (ADR 0094): never before the first run, and not between a refused try and its retry, which waits for the gate. `planet_perf::clock::duty_idle_s` SHALL be the one rule.

Verify: A
Status: active
Source: ADR 0094 (SPIKE-95: 45 s idle before each run held SPIKE-90 window 2 at 52-58 C with no cool-down wait), BUILD-033

#### Scenario: The duty idle
- **WHEN** the previous run took 40 s, 77 s or 160 s, or no run came before
- **THEN** the idle is 20 s, 38.5 s, 45 s and none

#### Scenario: Between runs only
- **WHEN** the A/B helper runs 5 rounds with fake runs of two builds and 3 refused tries
- **THEN** the runner rests 9 times: between the 10 runs, not before the first and not for the refused tries

### Requirement: BUILD-034 The perf window runner
`cargo xtask perf window <plan> [--no-preflight]` SHALL run a plan of configurations and flights in one process: one release build, the preflight (TEST-040; a refusal stops the window), then the plan's rounds, configurations rotated each round, and the duty idle between runs (BUILD-035). Invalid or refused runs SHALL be retried up to the plan's count; a failed run stops the window, keeping earlier attempts. A configuration SHALL stop once settled against the reference: after a valid round more than 15 % worse on a metric, or after two valid rounds within 5 % on every metric (ADR 0087 point 7). Every attempt SHALL keep its start, end and idle time; a table gives medians, changes and minutes per valid run.

Verify: A
Status: active
Source: ADR 0087 point 7, ADR 0094, SPIKE-95 (PR 3 of its plan; SPIKE-90 windows 1 and 2: shell-loop runners, a copied log, a runner that outlived TaskStop), opus-planner (inbox #72)

#### Scenario: A plan
- **WHEN** a plan names 3 rounds, 1 retry, the reference `c`, three configurations with their environment and a `stream dive` flight, and plans with a zero round count, an unknown statement or metric, a missing log name or no flight are parsed
- **THEN** the first gives those values and the log `dive.json`, and each other is refused naming the fault; round k runs the configurations rotated by k

#### Scenario: The stop rule
- **WHEN** a configuration reads p99 +62 % against the reference after one valid round, another +3 % after one and after two, another +10 % after two
- **THEN** the first settles worse after round 1, the second stays open after one and settles within after two, and the third runs every round

#### Scenario: A window
- **WHEN** a fake window of 3 configurations and one 40 s flight runs, one configuration far worse and one within the margin
- **THEN** the runs are c, d, e, then e, c; the runner rests 20 s between runs, never before the first; every attempt has its start, end and idle; the table names who settled after which round and the minutes per valid run

#### Scenario: Retries and failures
- **WHEN** the first run is refused and its retry comes out invalid with retries 1, and another window's child fails
- **THEN** the refusal waits once with no duty idle before its retry, both attempts are kept with their reasons, nothing settles without a valid reference round, and the failure stops the window naming the run with the attempts before it kept and the table saying STOPPED

### Requirement: BUILD-036 Tiers A and B run in three parts per operating system
The CI job that runs tiers A and B SHALL run as three jobs per operating system, `windows (WARP) A+B 1/3` to `3/3` and `linux (lavapipe) A+B 1/3` to `3/3`, each `cargo xtask test -- --partition hash:N/3`, so that every tier-A and tier-B test runs in exactly one part, a test's part depends on its name alone, and each job stays under the 16 min budget of BUILD-014. Part 2 SHALL also run the determinism manifest and its upload (GEN-003), check, spec-lint, the licence check and the run-mode starts (BUILD-005); parts 1 and 3 run only their tests. The artifacts of a part SHALL carry its number (`test-logs-<OS>-ab-N`, `review-<OS>-ab-N`).

Verify: A
Status: active
Source: opus-planner (2026-11-03: the Windows A+B job took 16.4 min in #1128's run, its test step 806 s of which 257 s build and 548 s tests; the critical path of a CI round after BUILD-018's archive; 2026-11-03, inbox #79, #1134; ab-balanced-parts), BUILD-005, BUILD-014

#### Scenario: Three parts, the checks once in part 2
- **WHEN** the repository check test reads the workflow file
- **THEN** the A+B matrix holds parts 1 to 3 for Windows and for Linux, the test command takes `--partition hash:{part}/3`, the manifest, its upload, check, spec-lint, licences and the run-mode starts run in part 2 only, and the artifact names carry the part

### Requirement: BUILD-037 Prepush checks what the push sends
`cargo xtask prepush` SHALL refuse to run, before any step, when the working tree differs from `HEAD`: any tracked file that differs (staged or not) or any untracked file under `crates/` or `xtask/`. The refusal SHALL name the count and the first paths and say to commit or stash them, because the checks read the working tree while the push sends the commit (a `cargo fmt` after the commit passes the checks and fails CI).

Verify: A
Status: active
Source: Planner 2026-11-04 (inbox #60; #1162 failed CI on fmt after a post-commit `cargo fmt`), BUILD-016

#### Scenario: A dirty tree
- **WHEN** the porcelain status lists tracked files changed (staged, unstaged, renamed) and untracked files under `crates/`, `xtask/` and elsewhere, and a real repository holds a committed tree, then a changed tracked file and a new file under `crates/`
- **THEN** the tracked paths (the new path of a rename) and the untracked ones under `crates/` and `xtask/` are named and the others are not, the committed tree passes, and the dirty one is refused naming the count, the paths and "commit or stash"

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

### Requirement: BUILD-040 A change of a gate leaves a contract-change line
Spec-lint SHALL fail a branch that adds, removes or changes a row of the gate table under `### 12.1` of `docs/design/terrain-target.md`, against the merge base with `origin/main`, unless the same branch adds to that document's `## 14. Contract changes` list a line `- YYYY-MM-DD | gate or section | what changed | who decided` that names the changed row. A malformed line covers nothing, and a branch that removes the target document with its gate table fails.

Verify: A
Status: active
Source: Owner 2026-11-09 (management rules become checks), planner inbox #94; owner 2026-11-27 (D10: the terrain prototype briefing archived, its owner-decisions table no longer checked, its contract lines moved to the target document)

#### Scenario: A gate changes
- **WHEN** a branch changes the G-SURF row and adds no line, adds a line naming G-CON, adds a malformed line, or adds a dated line naming G-SURF
- **THEN** the first three are refused naming G-SURF and the last passes; a line already on the base covers no new change

### Requirement: BUILD-041 A gate ledger holds every measurement of a gate
`docs/tools/gates/ledger.md` SHALL hold one row per measurement of a gate of `docs/design/terrain-target.md` 12.1: gate, seed, region, resolution, commit, value, verdict (`pass`, `fail`, `partial`, `reported`, `not run`) and the lane that measured it. Spec-lint SHALL fail a gate of 12.1 with no row, a row of a gate not in 12.1, a bad verdict or an empty cell, and a measured row whose commit does not resolve; a `not run` row SHALL have `-` as commit. `cargo xtask gates` SHALL print per gate the newest verdict, the rows and the lanes, and name the gates measured by one lane only. CI SHALL check out the history spec-lint reads (BUILD-040).

Verify: A
Status: active
Source: Owner 2026-11-09 (management rules become checks), planner inbox #94 (#1259 reached green with its gates unmeasured)

#### Scenario: The ledger
- **WHEN** a gate has no row, a row names an unknown gate, a measured row has a commit that does not resolve, or all gates have rows including `not run`
- **THEN** the first three are refused naming the gate or commit and the last passes; the summary names the gates measured by one lane

### Requirement: BUILD-043 The lanes are listed with their state
`cargo xtask lanes` SHALL print, for each lane of `docs/tools/lanes.txt` (a lane name and the prefix of its worktree directory under `.claude/worktrees/`; no machine paths), the last commit of its worktree with its age, the open PR of its branch, the highest `## #N` heading of its inbox file (`PLANET_INBOX_DIR`, else `SeriousPlanetData/lanes` beside the main checkout) and the running `cargo`, `rustc`, `nextest` or `target/` executables whose path lies in its worktree; other build processes SHALL be listed as not attributed. A missing piece SHALL print `-` and the command SHALL NOT fail for it.

Verify: A
Status: active
Source: Owner 2026-11-09 (management rules become checks), planner inbox #94

#### Scenario: A lane block
- **WHEN** a lane has no worktree, or a worktree with no commit, PR, inbox file or process
- **THEN** its block prints `-` for each missing piece, and a process under a lane's worktree is listed under that lane

### Requirement: BUILD-044 The pr-gate requires the verifier line on a code PR
`cargo xtask pr-body-check` SHALL read `PR_BODY`, `BASE_SHA` and `HEAD_SHA` from the environment and fail when the PR changes a path that is not docs-only (the paths `ci.yml` ignores) and its body has no line `Verifier: PASS <sha>` (at least 7 hex digits) naming a commit of `BASE..HEAD`. A PR whose every path is docs-only SHALL pass. The `pr-gate` workflow SHALL run it on every pull-request event including a body edit, with the full history, and a sha that is not the head SHALL pass with a note naming the later commits.

Verify: A
Status: active
Source: Owner 2026-11-09 (management rules become checks), planner inbox #94 (#1259 reached green with the verifier not run)

#### Scenario: The verifier line
- **WHEN** a code PR has no line, names a commit that is not in the PR, names its head, or names an earlier commit, and a docs-only PR has no line
- **THEN** the first two are refused, the head passes, the earlier commit passes with a note, and the docs-only PR passes

### Requirement: BUILD-045 The class-source store tests take slots in proportion to their work
The nextest profile SHALL give every test of `planet-cache`'s `it` binary whose name starts `class_source_store::` `threads-required = 4`, and SHALL NOT change their slow-timeout (the default's 180 s) or any test body. The comment of the override SHALL state the measured durations.

Verify: A
Status: active
Source: Architect 2026-11-09 (inbox #96: the keys test of the store hit the 180 s cap in three full `test fast` runs; 23 s alone), BUILD-003

#### Scenario: The override
- **WHEN** `.config/nextest.toml` is read
- **THEN** the override for those tests has `threads-required = 4` and no slow-timeout of its own

### Requirement: BUILD-046 The sheet probe writes fixed-scale sheets and numbers in a bounded buffer
The sheet probe SHALL write for each field of a stage a PNG `<run>/<stage>-<field>.png` whose colour scale is fixed by the field's kind (the same value is the same colour in every run and at every scale) and a line in `<run>/stages.txt`: cells, minimum, 5th, 50th and 95th percentile (from a 4,096-bin histogram), maximum, a 64-bit hash of the data, the numbers the stage gave, and the probe's own milliseconds and bytes. A height field SHALL also get `<stage>-<field>-shade.png`, a scale-free hillshade of its bins. A sheet SHALL have at most `max_bins` bins and the probe's buffers SHALL be bounded by them and the histogram, whatever the grid. The sheets are review aids, not goldens.

Verify: A
Status: active
Source: Architect order 2026-11-09 (inbox #100, briefing item 18: the gate margin is 1 %, so the probe must not raise H1's peak), GEN-161

#### Scenario: The sheet
- **WHEN** a 900 x 700 field is probed with 10,000 bins, two sheets of different data show the same value, and a bit of one cell changes
- **THEN** the sheet has at most 10,000 bins with the aspect kept, the value has one colour on both sheets, the percentiles are within a bin of the exact ones, and the data hash differs

### Requirement: BUILD-047 The stage manifest is committed and required up to date
`docs/tools/stages/manifest.txt` SHALL hold, for the stage probe's fixture (a compact landmass of 24 cells a face edge through the rounds, H1 and the sediment pass), one line per stage and field: the cells, minimum, 5th, 50th and 95th percentile, maximum, the scale counts, the hash of the data (never of a PNG) and the stage's numbers, without the probe's cost fields. A tier-A test SHALL run the fixture and fail when the file differs, naming the stage, the field and the first differing column. `cargo xtask stages` SHALL run that check and `cargo xtask stages --update` SHALL rewrite the file. The fixture's sheets SHALL be written to `target/review/stages/ci/`, which CI's review artifact carries.

Verify: A
Status: active
Source: Owner 2026-11-09 (stage previews: a committed text manifest; images in target/ and CI), architect order inbox #100, BUILD-046

#### Scenario: A stale manifest
- **WHEN** a number, a line or a whole stage differs between the file and the run
- **THEN** the test fails naming the stage, the field and the column, and says to run `cargo xtask stages --update`

### Requirement: BUILD-048 The stage reviewer advises on a manifest change and the committed sheets stay small
`.claude/agents/stage-reviewer.md` SHALL define a read-only agent (tools Read, Glob, Grep, Bash). When a PR changes `docs/tools/stages/manifest.txt`, the PR's author SHALL run it before the push and paste its report, unedited, into the PR body. No CI step runs it and its report never blocks a merge. It judges the manifest diff and the before and after sheets against the declared intent, never the author's assessment; a sheet it cannot open gives `cannot judge`; `consistent` means matching the intent, not right terrain. Sheets are committed only at a gate, under `docs/tools/stages/sheets/<gate>/`. `cargo xtask spec-lint` SHALL fail when a sheet is over 1,000,000 bytes or one gate's sheets total over 5,000,000 bytes, naming the file or gate, its size and the limit.

Verify: A
Status: active
Source: Architect 2026-11-10 (briefing item 18: advisory reviewer agent and sheet size cap), BUILD-046, BUILD-047

#### Scenario: The size check
- **WHEN** a sheet is one byte over the per-sheet limit, or the sheets of one gate total over the per-gate limit, or all are at or under their limits
- **THEN** the first two fail naming the file or gate, its size and the limit, and the third passes

### Requirement: BUILD-049 The tests over 60 s on the Windows runner run one at a time
On Windows the nextest profile SHALL put the `gpu_all` tests that took over 60 s in the Windows CI runner's `C` shards into the `warp-heavy` group (at most one at a time), with the heavy tests that group already holds: the two lit and look smokes of `gpu_stream_perf`, `gpu_stills` (one pose by name), `gpu_still_flicker` (streaming settle), `gpu_ground_look` (hills march shadow), `gpu_indirect`, `gpu_overlay` (inspection images) and `gpu_steep_terrain`. It SHALL NOT change a slow-timeout, a test body, a skip or a retry.

Verify: A
Status: active
Source: Architect 2026-11-10 (the `look-2km` smoke took 248 s beside nine heavy tests in CI run 42780685812, `C 1/4`), BUILD-003, BUILD-045

#### Scenario: The override
- **WHEN** `.config/nextest.toml` is read
- **THEN** one override with `platform = 'cfg(windows)'` names those tests, sets `test-group = "warp-heavy"` and sets no slow-timeout

### Requirement: BUILD-050 The build environment is pinned
`rust-toolchain.toml` SHALL name one exact Rust release (X.Y.Z, never a channel name), and every `dtolnay/rust-toolchain` step of `.github/workflows/ci.yml` SHALL install that release. `Cargo.lock` SHALL be committed and not ignored. No build file (`.cargo/config.toml`, a `Cargo.toml`, the CI workflow) SHALL set a target CPU, a target feature, a fast-math style flag or `RUSTFLAGS`; the build profile stays as `Cargo.toml` defines it.

Verify: A
Status: active
Source: Architect 2026-11-11 (ADR 0114 point 6; external review condition 2), owner 2026-11-12 (audit lists: every approved list a queue row), BUILD-005

#### Scenario: The pin
- **WHEN** the repository files are read
- **THEN** the toolchain is one exact release, every CI toolchain step names it, `Cargo.lock` is committed, and no file sets a target CPU, feature or fast-math flag; a moving `stable` in CI, a missing `Cargo.lock` or a `target-cpu` line fails the test naming the file and the line

### Requirement: BUILD-052 K7 changes report their difference from an alternate tile layout
`cargo xtask k7-layout` SHALL run K7 on the region bake fixture under the canonical layout and the alternate layout (twice the tile side, same halo), unquantised, and SHALL print: cells differing in bits and their share, p50, p99 and maximum height difference in metres, and the cells over 0.25 m and over 1 m, in total, on and off the channel lines, and for all cells by distance to the nearest alternate seam (0 to 7, 8 to 31, 32 to 127, 128 or more cells). `cargo xtask prepush` SHALL run it when the diff touches K7, print the committed baseline beside it, and `REVIEW` above twice the baseline p99 or maximum. The exit code SHALL be 0.

Verify: A
Status: active
Source: ADR 0119 point 5 (owner, 2026-11-12), architect 2026-11-12 (K7 continuity list, item 3), GEN-177

#### Scenario: The report is a report
- **WHEN** the fixture's two layouts give a maximum difference above twice the baseline
- **THEN** the output names the row and prints `REVIEW`, and the exit code is 0
- **AND** with the layouts equal the output says 0 cells differ

#### Scenario: Determinism
- **WHEN** the command runs at one thread and at eight
- **THEN** the printed lines are equal

### Requirement: BUILD-053 The plan command lists the milestone's exit items against the queue
`cargo xtask plan` SHALL read the active milestone (the file of `docs/milestones/` whose `Status:` line starts with `active`) and its exit table, and SHALL print per exit item its state, lane, done-when, its rows in the queue of `docs/status.md` (first column: the exit item) and the open PRs whose title names it.
It SHALL fail for a queue row whose Exit cell is not an exit ID, an open item without a row, a missing exit table, a duplicate exit ID, and a selection of milestones the scenarios below refuse.
`cargo xtask spec-lint` SHALL run the same checks, without the PR list. An unreadable PR list SHALL print a note and SHALL NOT fail.

Verify: A
Status: active
Source: ADR 0120 point 5 (owner, 2026-11-12), lead 2026-11-12; lead 2026-11-27 (the draft state)

#### Scenario: A queue row off the milestone
- **WHEN** a queue row's Exit cell is not an exit ID of the active milestone
- **THEN** the command names the row and exits non-zero

#### Scenario: An open item without a row
- **WHEN** an exit item is open (its state does not start with `done`, and in a sprint file, BUILD-074, not with `promoted`, `parked` or `carried`) and no queue row names it
- **THEN** the command names the item and exits non-zero; a closed item needs no row

#### Scenario: Between sprints
- **WHEN** no file is active and one draft is (`Status:` starting `draft`)
- **THEN** the command prints `no active sprint; <M> waits for the owner's decision: open`, passes with an empty queue and fails for any queue row

#### Scenario: The selection
- **WHEN** the files hold one active file, one active file and one draft, two active files, two drafts, neither, or a draft that is not a sprint file
- **THEN** the first two pass and the rest fail

### Requirement: BUILD-054 repro replays a recorded path in a window and counts the stalls
`cargo xtask repro <frame-N-path.txt | bundle> --window [--real] [-- OPTIONS]` SHALL replay the camera path in real time in `planet editor --smoke` (the gate's path replay, TEST-061) over the world its bundle records (VIEW-071); options after `--` go to the editor. The run measures: the gate's failures are printed, not raised, and its output is kept under `target/repro-window`. It SHALL print the gate line (stalls of 1 s or more with the GPU phases counted, each with its frame's resident, written and evicted tiles; the held-back tiles and how many lie over the commit's region). With `--real` a locked desktop is NOT RUN. It SHALL refuse a non-terrain bundle, a missing path file, and `--at-commit`, `--every` or `--store`.

Verify: A
Status: active
Source: Lead 2026-11-13 (P2 E3), crusader's #124 diagnosis (no windowed path replay), VIEW-026, TEST-061

#### Scenario: The stalls of a path
- **WHEN** the gate line of a replay reads "stalls 7 (frames of 1000 ms or more with the GPU phases, longest 6400 ms)"
- **THEN** the verdict says 7 stalls and the options of the world the run opened

#### Scenario: A stall with its counts
- **WHEN** a frame of 4.5 s has 23,100 tiles resident, 120 written and 4 evicted
- **THEN** the gate line names the frame, its longest phase and those three counts, so that a resident count is told from an upload burst

#### Scenario: A render submit counts
- **WHEN** a frame spends 4.5 s in its render submit and nothing else is slow
- **THEN** it is a stall of the report and not a failure of the gate's frame limit

#### Scenario: A replay that the gate would fail
- **WHEN** the plan holds tiles back with no edit in reach during a replay
- **THEN** the gate line says how many distinct tiles were held, at which levels and how many lie over the commit's region, the failure is printed as not judged, and the run ends normally with its closing lines

### Requirement: BUILD-051 The determinism manifest covers the region bake at several thread counts
`generate --manifest` SHALL write, after the existing lines, the lines `regionbake t1`, `regionbake t1again` and `regionbake t8`: a 64-bit hash of every file of the region bake fixture (`planet_cache::region_bake::determinism_fixture`), baked at one thread twice and at eight threads. The three hashes of one run SHALL be equal. The CI determinism job's manifest diff SHALL compare them across Windows and Linux.

Verify: A
Status: active
Source: Architect 2026-11-11 (ADR 0114; external review condition 3), owner 2026-11-12 (approved lists get a queue row), GEN-003, GEN-177

#### Scenario: A thread count or a repeat that changes a byte
- **WHEN** the fixture is baked at one thread twice and at eight threads
- **THEN** the three hashes are equal; a bake whose bytes differ at another thread count, in a repeat, or on the other operating system fails the tier A test or the manifest diff naming the line

### Requirement: BUILD-055 The river gate prints the river numbers of a region
`cargo xtask river-gate <store dir | window>` SHALL print, in release and under the heavy lock, on one region at L: the share of river cells inside in closed basins (at least 0.5 m deep and 16 cells, `--min-cells`) beside SPIKE-114's 5 cm reference; the valley depth (surface 1 to 2 cells out above the river cell, the canyon a viewer sees) and the burn (surface above the river level, K7's cut), each a share over 50 m; ring 2-4 and the erosion depth; the share in lakes deeper than D (`--d`, default 20 m); p50 relief at 2 x 2 and 10 x 10 cells beside the start's; H1's edge crossings matched within 2 km. It SHALL exit 0.

Verify: A
Status: active
Source: Lead 2026-11-14 (P3 E1 and E2), SPIKE-114, SPIKE-115, crusader 2026-11-14 (surfaces, canyon, N), TEST-136, lead 2026-11-14 (canonical window names), lead 2026-11-14 (the canyon as valley depth and burn), spikes 2026-11-15 (SPIKE-119: windows M1 and F1), lead 2026-11-23 (the gate's outlets are the connected sea), lead 2026-11-23 (the gate builds its own ocean; the routing's flat grade)
Detail: The gate's outlets, of its floods and its routing: the window's edge cells and the ocean cells still at or under the sea level on the measured surface; the ocean is the cells at or under the sea level on the window's start surface joined through such cells (8-connected) to an edge cell or to an L cell of an H1 sea cell inside the region (the chain's connected sea mask, on a raw grid, the grid itself and its edge), so a coast the variant raises does not cut the sea off, and a start-sea cell the variant raises above the sea is land; an inland cell under the sea level is no outlet: its pit fills, while the land-only measures (cells above the sea level) still leave it out. The gate builds the ocean itself for every candidate; a chain's `ocean.u8` beside the stage fields is compared only, as `g_ocean_extra` (its cells outside the gate's ocean) and `g_ocean_missing` (the reverse), in cells and 8-connected components, both 0 on a chain row. The routing floods canonically with GEN-173's grade of the flats at 1e-5 m a cell, so a flat drains by its distance to its exit and a window-wide flat rises under the 0.25 m uphill step. The valley depth is the p50 of the surface at Chebyshev distance 1 to 2 (102-204 m) above the surface at the river cell; its bar is relative to Earth: at most the share of a GLO-30 tile with 1 to 2 x the window's relief at 10 x 10 cells; the gate prints that relief beside the share, and the GLO-30 share comes from the review table, not from the gate. The burn is the surface at the river cell above the river level, every basin breached and with the lakes of D kept. Ring 2-4 is the p50 of the surface 2 to 4 cells out above the river level (the valley depth there plus the burn). A window is `face,i0,i1,j0,j1,seed` in regional cells or a canonical name: R1h face 1, 8056-8675 x 760-1379, seed 14 (the E1 region stores' window, the default); R3a face 4, 368-987 x 4720-5339, seed 15; R3b face 2, 9968-10587 x 8304-8923, seed 15; M1 (mountain belt) face 4, 6600-7219 x 10200-10819, seed 15; F1 (low-gradient river landscape) face 4, 4800-5419 x 3300-3919, seed 15. The basins are found under a fill of 1e-7 m a cell. A window measures its start surface (the base's C1 read B plus the regional field) over the region's box. A store measures its stored ground B + L (an L tile is K7's ground minus B) over the commit's tiles, with the start surface around them, its tiles draining to their border, at the commit's sea level; a commit of another grid, radius, seed, base or (with the keys) inputs identity is refused; `--no-key-check` reads each tile under the key its commit names and the report says "keys not checked". An H1 crossing counts with Q of at least 1 m3/s. An unknown option, a missing value, a second window and `--no-key-check` without a store are refused.

#### Scenario: An inland pit below the sea level
- **WHEN** a window holds a pit under the sea level that no chain of cells at or under the sea joins to its edge
- **THEN** the gate's flood fills it as a closed basin; a pit joined to the sea through cells at or under it is sea

#### Scenario: A window
- **WHEN** the gate runs on `r1h`, `r3a`, `r3b`, `m1` or `f1`
- **THEN** it measures that canonical window and prints one line each for rivers, basins, valley depth, burn, ring 2-4, erosion depth, lakes, relief and edge, and exits 0

#### Scenario: A spike store
- **WHEN** the gate runs on a spike branch's store with `--no-key-check`
- **THEN** it names the commit and prints "keys not checked: spike store"

### Requirement: BUILD-056 Prepush passes local tier C failures of known classes only, with a summary
When `cargo xtask prepush` runs `test gpu` and it fails, prepush SHALL read that run's log and pass the step only when the run ended, at least one test failed, every failed test matches a line of `xtask/known-tier-c.txt` (a test pattern, an outcome: `abort 0x<code>`, `timeout` or `panic at <file>:<line>`, and the bold title of the `docs/status.md` Tracked entry it cites, which must be present) and no candidate golden was written. It SHALL then print "tier C: N known-class failure(s), listed" and one line per failure, for the PR body. Any other failure, any candidate, or a run without a summary SHALL fail the step. The list is local only: CI runs tier C in full on every push.

Verify: A
Status: active
Source: Lead 2026-11-14 (explorer #149; after #1515 the draw-flag timeout and gpu_preflight's bound split from its text), codifying the architect's rule of 2026-11-09 (known local tier-C classes: WARP aborts 0xc0000005, gpu_skeleton timeouts, gpu_preflight's 8 s bound); BUILD-016, BUILD-026
Detail: It changes no test and no tolerance. A list line whose Tracked entry is gone is ignored with a warning. A failure of a listed test with another outcome (another exit code, another test of the same binary timing out, a panic at another line) is not covered.

#### Scenario: Known classes only
- **WHEN** tier C fails with a WARP abort 0xc0000005, the gpu_skeleton pipelined-loop timeout and gpu_preflight's panic at its 8 s bound (an assertion of its own, apart from the refusal text), and no candidate is written
- **THEN** prepush passes the step and prints "tier C: 3 known-class failure(s), listed" with one line each

#### Scenario: Anything else
- **WHEN** another test fails beside them, or a candidate golden is written, or the run has no summary
- **THEN** prepush fails the step and names the uncovered failure

### Requirement: BUILD-057 The river gate's measurement suite
`cargo xtask river-gate` (BUILD-055) SHALL print, after the gate lines and from the same input (a window's start surface or a store's baked ground), one line per metric, each beside the absolute count it is taken over: the gradient, the valley width, the cross-section asymmetry, the drainage density, the catchment ratio at H1 crossings, the seam at the region's edge, the narrowness ratio, the slope-area scaling and the relief power spectrum. The lines SHALL be the same bits for the same input.

Verify: A
Status: active
Source: Lead 2026-11-15 (P3 E1; the owner's rewrite of P3 after the external review), BUILD-055, TEST-137, lead 2026-11-15 (the store-path test and the table), lead 2026-11-15 (the trunk column for the E4 note), lead 2026-11-15 (the trunk column in corridors, raw and snapped, against a bar per window; lead 2026-11-16: the bar over the region's box; lead 2026-11-15 ground lowered over 50 m; DEC explorer-4: corridors over counted cells only; DEC explorer-5: corridors on the water surface; lead 2026-11-16 land-only snaps and routes, TEST-138); 2026-11-24 (decision 9's measure: #198's width table into main's gate, report only)
Detail: One definition per metric; the bars come from the spikes' Earth benchmark (windows, transects and the GLO-30 reference, #121), not from this requirement.
- conventions: the quantile f of n sorted values is the value at index floor(n f) (the upper median for p50); 0 for none; a log is log10; a least-squares fit with fewer than two points is 0;
- gradient: a river step is a river cell inside to its receiver when that is a river cell; it is uphill when the receiver's surface stands more than 0.25 m above it; printed: uphill steps, steps, the largest rise;
- valley width: across the flow (perpendicular to the step to the receiver) the surface is walked in cell steps, bilinear, up to 64 cells to each side, to where it first stands more than the depth above the river cell's surface, the crossing interpolated; the width is both sides in metres; a side that reaches 64 cells or the grid's edge is capped and counted; the depth is 10 m, and half the cell's ring 2-4 depth, where a ring 2-4 depth under 2 m counts the cell as flat; printed: p50, p90, cells, capped sides, flat cells;
- asymmetry: |L - R| / (L + R), where L and R are the mean of the surface at the points 2, 3 and 4 cells out on each side (bilinear) minus the river cell's surface, a negative mean counting as 0; cells with L + R under 1 m or a point outside the grid are left out; printed: p50, p90, cells;
- drainage density: the river steps' length inside (a diagonal step is the cell width times the square root of 2) over the land area inside (cells above the sea level), km/km2;
- catchment ratio: each H1 crossing of Q at least 1 m3/s gives L's drainage area at the nearest L crossing of its direction within 2 km over its Q; the median of these is the implied runoff (m2 per m3/s); printed: the pairs, the implied runoff, p10 and p90 of each value over the median (1 when L carries H1's catchment at every crossing); no pair without an H1 crossing;
- seam: |dz| of the surface between 4-neighbours with one cell inside the region's box and one outside (count, p50, p99, maximum), against p50 and p99 between 4-neighbours both inside;
- narrowness: the valley depth (BUILD-055) over the ring 2-4 depth above the river cell, over river cells whose ring 2-4 depth is at least 1 m (p50, p90, cells); and the p50 valley depth over the p50 ring 2-4 depth, both over every river cell inside;
- slope-area: land cells inside with a receiver, a positive slope (the surface drop over the step length) and an area of at least 4 cells; bin b holds areas from 4 x 10^(b/2) cells; per bin the p50 slope and the cells; theta is minus the fitted slope of log p50 slope against log (the bin's lower area x 10^0.25), over the bins of at least 30 cells;
- spectrum: the largest power-of-two square of at least 8 and at most 1024 cells in the bounding box of the cells inside, centred in it (none below 8); the mean removed; a Hann window w(i) = 0.5 - 0.5 cos(2 pi (i + 0.5) / n) in each axis; the 2-D FFT; the power |F|^2 over the sum of the window's squares, averaged per octave o of the radial wavenumber k (cycles per square, floor(log2 k) = o, 1 <= k < n); beta is the fitted slope of log power against log (1.5 x 2^o) over the octaves from o = 1; printed for the surface and for the start.
- trunks: each segment of H1's channel graph whose discharge reaches 1 m3/s, its H1 cell centres in L cells upstream first, is sampled every L cell (bilinear surface); its bed is the running minimum downstream; a vertex inside counts when the surface stands more than the valley depth's 50 m (`cut_m`, until G-INCISION has a number) above the bed, the burn KR would need (the E4 note: KR does not cut); printed: reaches and vertices inside and over, with their shares, the deepest vertex's depth, reach and cell; no H1 reach in the input says so;
- trunks in corridors: for k_c 1, 2 (the requirement) and 4, each such reach is routed inside its corridor (the counted cells, a store's tiles or the window's box, within k_c x 814 m of its polyline: outside its tiles a store holds the start surface, not the variant's) on the water surface (a cell under the sea counts at the sea level, as a riverbed at the coast sits there; snapping too) by the 8-neighbour route that minimises the largest rise above the route's running minimum (exact, the maximin form of Dijkstra's search: a higher running minimum leaves every later step at least as much room, so each cell keeps its highest reachable minimum, highest first, ties by cell id); a reach passes when that rise stays within the window's bar `min(50 m, max(10 m, 0.5 x the start's p50 relief over 10 x 10 cells of the region's box))` (set before the results, P3: R1h 27.5, R3a 10, R3b 10, M1 50, F1 10 m; a store's tiles do not move it) over every cell of the route, and otherwise it is over and its minimax rise is bisected to 0.1 m (the vertex counts are of the route's cells inside); two forms: from H1's raw ends, and snapped (each H1 node to the lowest land L cell (above the sea level) within one H1 cell, ties by cell id, shared by every reach at the node; only a mouth, a reach's last node where no reach starts, may snap to the sea; a node with no land cell in its disc takes the lowest cell; a reach crossing the region's edge passes the lowest land edge cell within one H1 cell of H1's crossing step, ties by cell id; a crossing with no land edge cell in its disc adds no waypoint); in both forms routes step over land only and a route that meets the sea before its last node ends there; a reach whose end lies outside the grid or its corridor, whose corridor does not join its waypoints, or that has no vertex inside is not counted, so the counts may differ between forms and k_c; printed per form and k_c: the share of reaches passing, the reaches over, the vertices over along the routes, the deepest reach, and the bar (the E4 note: whether KR can route without a cut; no pass share is set);
- lowered: the land cells inside (the start above the sea) whose surface lies more than 50 m (`cut_m`) below the start, as a count and a share of the land cells inside (ground lowered over 50 m);
- width by order and area class (report only, no bar): the widths at 10 m above split by the river cell's Strahler order on the river cells (each to its receiver when that is a river cell; a cell no river cell drains into is order 1, else the largest order draining into it, plus 1 when two or more carry it; upstream first by drained area, ties by cell id), orders 1, 2, 3 and 4 or more, and by its drained area in km2: under 75, 75-300, 300-1,000, 1,000-3,000, from 3,000; every measured cell lies in one of each; printed as `RIVER-GATE widths: name=value ...` (`all_p50_m`, `all_cells`, then `o1`, `o2`, `o3`, `o4plus`, `a_lt75`, `a75_300`, `a300_1000`, `a1000_3000`, `a_ge3000`, each `_p50_m` and `_cells`), and in `--table` one row per run in `target/review/river-gate-widths.md` and `.csv` beside the table;
- table: every run also prints `RIVER-GATE fields: name=value ...`, every number of the gate and the suite with a count before every percentage; `cargo xtask river-gate --table <window | store dir@window | store dir>... [--windows W,W...] [options]` runs the gate once per row under one heavy lock and writes one row per run to `target/review/river-gate-table.md` and `.csv` (a store row on its own window, a window row on its start surface; the options go to every row); the store path lays the commit's tiles through the reader (`lay_store_ground`), the same function the store mode runs.


#### Scenario: A store
- **WHEN** the gate runs on a store
- **THEN** the suite lines are measured on the stored ground B + L over the commit's tiles, with the same definitions as on a window

#### Scenario: Metrics of a valley
- **WHEN** the surface is a V valley with a river on its floor
- **THEN** the width at 10 m, the asymmetry, the narrowness and the density have the values of its geometry (TEST-137)

#### Scenario: The width by order and area class
- **WHEN** the gate runs on any input
- **THEN** its widths at 10 m are split by Strahler order and by drained-area class, each split covering every measured cell, and the split's p50 over all of them is `width10_p50_m`

### Requirement: BUILD-059 The river gate's channel network at a threshold
`cargo xtask river-gate` (BUILD-055) SHALL print and tabulate, from the same routing as BUILD-057, the channel network at each of two channel thresholds (`--network-km2 A,B`, default 1 and 75 km2): the drainage density and the Strahler stream counts with the bifurcation ratio `R_b`; and it SHALL take a raw height grid as input (`grid=file,w,h,sea_level_m`), so that the GLO-30 benchmark tiles are measured by the same code as the windows.

Verify: A
Status: active
Source: Lead 2026-11-16 (river-graph plan, pre-work: the density bar fixed before any RG result), BUILD-055, BUILD-057, TEST-137
Detail:
- channel cell: a land cell (above the sea level) whose drainage area (the gate's routing, BUILD-055) is at least the threshold;
- density: the length of the steps from a channel cell inside to a channel receiver (diagonal steps sqrt 2 cells) over the land area inside, km/km2; a step into the sea is not counted;
- Strahler order on the whole grid from the channel donors: a cell without a channel donor is order 1; otherwise the donors' highest order `m`, raised to `m + 1` where two or more donors carry `m`; the result depends on the receivers only;
- a stream of order `k` starts at a cell of order `k` that no donor of order `k` enters (a head, or a meeting of streams of order `k - 1`); `N_k` counts the streams starting inside;
- `R_b = 10^-b`, `b` the least-squares slope of `log10 N_k` on `k` over the orders 1 to the highest less one with `N_k > 0` (the highest order is cut by the window); none (0) with fewer than 2 such orders;
- the raw grid: little-endian f64, row-major, cells of 101.8 m (SPIKE-119's preparation); the byte count must be `8 w h`; outlets are BUILD-055's: the grid's edge and the cells at or below the sea level joined to it; counted inside a 10-cell border; routing seed 7; no H1 (the input line says so, and the edge, trunk and corridor lines report none);
- fields per threshold `net1_*` and `net2_*`: the threshold, the channel length, the land area, the density, the stream counts by order (`N_1/N_2/...`), the orders fitted and `R_b`;
- the bar rule: for each window, the range over the GLO-30 tiles whose 1 km relief (10 x 10 cells, p50) lies within 1 to 2 times the window's start relief; the numbers are in the river-graph pre-work results, not in this requirement.

#### Scenario: A hand-built network
- **WHEN** four heads pair into two order-2 streams meeting in an order-3 trunk, and a fifth head joins the trunk
- **THEN** the counts are 5 / 2 / 1, `R_b` is 2.5 over orders 1 and 2, and the density is the step length over the land area (TEST-137)

### Requirement: BUILD-060 The river gate's G2 columns for river graphs
`cargo xtask river-gate` (BUILD-055) SHALL read RG and B through one build: given a canonical river graph (GEN-189), the per-stage budget fields or a partition pair, it SHALL print the G2 columns beside the raster numbers, each with its gate class (a `RIVER-GATE classes:` line, and a `class` row in the table); a row without them (B) prints none of them.

Verify: A
Status: active
Source: Lead 2026-11-17 (the surface input; owner 2026-11-17: the chain sheet as a kept tool; owner decisions 1 and 3 2026-11-17: the bars and the dual reference, 2026-11-18: the chain per row, owner 2026-11-18: the run's own start; 2026-11-18: step C's first measures; 2026-11-19: the base noise, owner 2026-11-18: kept-basin interiors and cells above a graph head counted apart; the river-graph plan's G2, its addendum's section 4 graph reports, the G0 contracts' sections 5, 6, 9 and ), BUILD-046, BUILD-055, BUILD-057, BUILD-059, GEN-189, TEST-137; owner 2026-11-20 (option C, and its three conditions: kept lakes and ponds with their own bars, the raw closed-basin share untouched, a spill mismatch a violation; lead 2026-11-21: test (a) at the rim beside test (b), and the f64 validator's records in bar_violations; lead 2026-11-22: the excused down-node rises in their own columns, lead 2026-11-23: the lake inlets by origin and the line-end ones in no kept basin; 2026-11-23: G3-1's gate integrity and A4-A6 of the review response, Carbon M4.7, Fable F2 and F4; owner 2026-11-23 : bar_relief_lost relaxed for the look, capped at 2 m; the spike's notch columns on main, the gate on main only)
Detail:
- inputs (read by `river_gate_graph::read_graph`, `read_stages` and `read_surface`): `--surface FILE` (a candidate's surface on the window's L grid, little-endian f64, row-major, every height finite: the window's raster columns measure it against the window's start surface; not with `--store` or `--grid`), `--noise axis | turned` (the base's band and crease noise, GEN-140: `turned` with the per-octave lattice turn, the default (main's bakes since GEN-140's default, owner 2026-11-19), `axis` without it; the window's start surface and a store's base follow it, so a run on the turned base is measured on a turned start), `--start FILE` (the run's own start surface on the window's L grid, f64 as `--surface`: a chain that conditions its base has another `S0` than main's; every raster delta, the budget field and the corridor are measured against it, and the corridor's trunks are the H1 reaches of the row's `--graph` at their H1 cell centres, cut at the window's edge; without `--graph` no trunk is measured; the H1 edge-agreement column still reads main's H1; not with `--grid` or `--store`; default: the window's start surface and main's H1 trunks), `--graph FILE` (canonical form), `--stages DIR` (`<stage>-lower.u16` and `<stage>-fill.u16` for kh, ks, kv and kc: little-endian u16 of 0.1 m on the window's L grid, row-major; a missing file is a stage that did not run or does not fill; `residuals.csv` a header and one line per residual record; `budget-violations.csv` the chain's f64 budget validator records, the header `cell,stage,used_m,budget_m` and one line each, a file with another header refused), `--partition ONE,TWO` (the canonical graphs of one and of two regions); `--corridor-bar PCT` (the corridor bar; default 97.5, 95 on M1); in `--table` a row takes them as suffixes `#graph=`, `#stages=`, `#partition=`, `#surface=`, `#start=`, `#noise=`, `#timing=FILE` (a run's timing file, read by the table only: a header, then `stage,seconds,peak_mib` per line, the numbers read from the line's right end, so a stage name may hold commas; the row gets `cost_s`, the seconds summed, and `cost_peak_mib`, the largest peak, `-` when none is given), and `#chain=FILE` (a run's chain file, read by the table only: a start-surface row takes the chain's label, `<label> @ <window>`, and the markdown table ends in a `Chains` section listing each such row's `commit`, `switch` and `stage` lines);
- ids: an L cell of the window `(x, y)` is the global cell `(x0 + x, y0 + y)`, id `(gy << 32) | gx`; 8-neighbours by those coordinates;
- classes:

| Part | Columns | Class |
|---|---|---|
| Graph validity | nodes and reaches per level; GEN-189's validators on an open slice, counted by kind, without the tile-core rule (the gate has no tile map); endpoints owned elsewhere; the L `lake_inlet` nodes inside by origin (`g_lake_inlets`; `g_inlet_line_ends` some reach's down node, `g_inlet_spill_targets` only a basin's spill node, `g_inlet_orphans` neither) and `g_inlet_no_basin` the line-end inlets whose cell lies in no kept-basin interior (the row above), `g_inlet_no_basin_far` those not within one cell of one (lead 2026-11-23; a spill target between reported basins is a hierarchy record and is not counted; an inlet both a line end and a spill target counts as a line end; columns only: they count every window cell, so an inlet beside a kept lake whose floor lies outside the window, or a slice's inlet whose reach another slice owns, is counted too); `g_excused_rises` and `g_excused_rise_m` the bed rises into a reach's down node GEN-189 excuses by its `budget_residual`, count and metres, on the graph line beside the violations | correctness |
| Trunk continuity, outlet connectivity | sources whose chain (followed through a marker along its reach) ends at an outlet, ends open, or neither; trunk reaches (H1, or L with a parent) whose held `down` node is neither an outlet nor a marker nor left by a reach | correctness |
| Junction conservation | the validators' conservation breaks | correctness |
| Bed profile | the validators' rises outside exceptions; reaches whose bed rises above its running minimum anywhere, and the largest such rise (m) | correctness |
| Exceptions | reaches per reason (`rise_over_budget`, `unresolved_endpoint`, `budget_residual`) | correctness |
| Tributary attachment | L reaches without a parent, and those ending at a held `trib_junction`, confluence, outlet or tile crossing | correctness |
| Hierarchy | on GEN-189's rasterisation over land inside: the length from each owned cell to its reach's next vertex (diagonals sqrt 2 cells), the density over the land area, Strahler streams by order (a stream of order k starts at an owned cell of order k that no donor of order k enters: the previous vertex, or at a reach's first vertex the reaches ending at its `up` node), `R_b` by BUILD-059's fit; read against BUILD-059's bar | geomorphology |
| Basins | kept and endorheic counts; a parent or a spill node the graph does not hold (a basin whose floor cell lies outside the counted cells apart, `g_basin_spill_missing_edge`: its spill path may leave the window); a same-level parent whose spill lies below the child's or whose floor lies above it | correctness |
| Graph/raster agreement | at each BUILD-059 threshold: routed river cells (land inside, drained area at least the threshold) with a rasterised graph cell within 1 cell (Chebyshev), and rasterised cells inside whose reach drains at least the threshold there (`a_geo` at the cell's vertex; G3-1) with a routed river cell within 1 cell; both shares; counted apart and outside both shares: routed and rasterised cells in a kept-basin interior (`agree<i>_routed_kept`, `agree<i>_graph_kept`), and routed cells above a graph head (`agree<i>_routed_above_head`: a cell with no graph cell within 1 cell whose downstream path on the gate's routing first comes within 1 cell of a graph cell at a cell within 1 cell of an L `source` node's anchor); each threshold's line names the three counts | correctness |
| Kept basins | a kept-basin interior: a basin `protected_surface`'s lake, the cells of the measured surface under its recorded spill (`spill_m`) joined (8-connected) to its floor cell inside the depression holding the floor (BUILD-057's basin flood: a rim cut under the spill or a drained cell does not let it run on); a floor at or above its spill, or in no depression, holds none; `kept_basins` the kept basins whose floor lies in the window under its spill; `kept_cells` (inside), `kept_steps` (BUILD-057's river steps with either end in an interior) and `kept_uphill_steps` (those rising more than 0.25 m); a `RIVER-GATE kept basins` line with the four counts | correctness |
| Closed basins and kept lakes | on a row with a graph (a kept basin is a lake, not a closed basin): `ponds_cells` the river cells inside in a closed basin (BUILD-055) outside every kept-basin interior (the row above), `ponds_pct` their share of the river cells inside outside the interiors; `kept_river_cells` and `kept_river_pct` the river cells inside in an interior, over all river cells inside; `kept_lake_cells` and `kept_lake_km2` the interiors' cells inside; `g_spill_mismatch` the kept basins whose floor lies in the window and whose depression level there (BUILD-057's basin flood) lies more than 0.5 m under the recorded spill (test b); `g_spill_rim` the kept basins whose floor lies in the window and whose rim (the cells outside the floor's depression that touch it, 8-connected) holds an L vertex whose bed lies more than 0.5 m under the recorded spill (test a: a line leaving the lake below its spill); `g_kept_in_window`, `g_notch_over_0_01`, `g_notch_over_0_1`, `g_notch_over_0_5`, `g_notch_max_m` and `g_kept_dry` (on main by ): the kept basins with a finite spill whose floor lies in the window, those whose recorded spill stands more than 0.01, 0.1 and 0.5 m over the basin flood's level at the floor, the largest such gap (m), and those whose floor the flood stands no more than 1 mm over (dry); columns only; BUILD-055's `basin_cells` and `basin_pct` unchanged; on a row without a graph these columns are `-` | geomorphology |
| Budget use | per stage over land inside (kh: `S0` against KH; ks, kv, kc: against `S_KH` when the fields are): cells lowered, their p95 and largest lowering, cells filled and the largest fill, fills over the budget; cells whose summed lowering of ks, kv and kc exceeds the budget and cells at 95 % of it or more (`b_overrun`, `b_at_95`), and both with kh's lowering added, against `S0` (`b_overrun_s0`, `b_at_95_s0`); the fields floor each stage to 0.1 m: a count is a true overrun, an overrun under the step is not seen; all diagnostic (the G0 contracts' section 6: the fields are never the gate; GEN-189's f64 budget validator is). Residual records and reaches flagged `budget_residual`; `b_validator_overrun` the f64 validator's overruns and raises in a lowering stage as the chain writes them beside the stage fields (`budget-violations.csv`, a header `cell,stage,used_m,budget_m` and one line each; `-` without the file) | preservation; residual counts and `b_validator_overrun` correctness |
| Partition | the changes between the two canonical graphs (GEN-189's diff), the topology changes among them, and the first change (`identical` for none) | partition |
| G2 bars (owner decision 1) | on a row with a graph, each `PASS:<value>` or `FAIL:<value>`: `bar_corridor` the snapped k_c 2 trunk reaches passing (BUILD-057) >= the corridor bar; `bar_uphill` river steps rising over 0.25 m (BUILD-057) <= 1.0 %, over the steps outside kept-basin interiors (uphill steps less `kept_uphill_steps`, over steps less `kept_steps`); `bar_agree_<A>km2` the lower of the routed and the graph share of the graph/raster agreement >= 90 % at each threshold (without the cells counted apart; a side with no cells counts 100 %); `bar_above_head_<A>km2` `agree<i>_routed_above_head` over the routed cells with them <= 10 % (G3-1, proposed for the owner, O10); `bar_trunk_exceptions` the trunk reaches (H1, or L with a parent) flagged `rise_over_budget` or `budget_residual` (`g_trunk_exceptions`) over the trunk reaches <= 1 % (A6; proposed, O10); `bar_violations` = 0, the validators' violations plus `g_spill_mismatch`, `g_spill_rim` and `b_validator_overrun`; `bar_spill_missing` = 0 (basins whose floor cell lies inside); `bar_ponds` `ponds_pct` <= 10 %; `bar_kept_river` `kept_river_pct` <= 5 %, relief-indexed to 10 % (the row below); `bar_uphill_1km2` and `bar_ponds_1km2` the uphill and ponds shares on the 1 km2 network (the row below) <= 1.0 % and <= 10 % (proposed, O10); `bar_hump_cuts` (the row below) per 10,000 km2 of counted land <= 1 (from B's row, 0.04 to 0.24 on the five windows) and `bar_corner_links` <= 10 (both proposed, O10); a `RIVER-GATE bars` line with the count passing | corridor preservation; the rest correctness |
| Gate-integrity measures | on every row: the 1 km2 network (river cells inside on land draining at least 1 km2): `fine_river_cells`, `fine_uphill_steps` and `fine_uphill_pct` (BUILD-057's river steps rising over 0.25 m, both ends outside the kept-basin interiors), `fine_ponds_cells` and `fine_ponds_pct` (river cells in a closed basin, BUILD-055, outside the interiors, over the river cells outside them); hump cuts: an 8-connected group of counted land cells lowered more than 0.25 m against a reference is a path, its two ends the ends of its longest walk (a breadth-first search from its first cell to the farthest cell, a second from there; ties by cell index), a hump cut a path along which the reference rises more than 10 m above its highest end and that is cut more than 10 m somewhere, `hump_cuts` against `S0` and `hump_cuts_s_kh` against `S_KH` (with KH's stage field; printed beside, not barred), `hump_cuts_per_10k_km2`; humps removed (lead review of #1648; reported, not barred): a hump crest is a counted land cell of `S0` at least 10 m above both cells 2 L cells away along a face axis or a diagonal, a hump an 8-connected group of crest cells, removed when the surface lies more than 10 m under `S0` at one of its crest cells, `humps`, `humps_removed` and `humps_removed_per_10k_km2`; corner-only links: 2 x 2 blocks of counted land cells whose one diagonal pair stands more than 5 m above both cells of the other, `corner_links` on the surface and `corner_links_s0` on `S0`, `corner_links_added_per_10k_km2` their difference per 10,000 km2; the margin (Carbon M4.7): the window's cells outside the region's box, `m_cells`, `m_river_cells` (75 km2 network), `m_basin_cells` and `m_basin_pct`, `m_ponds_cells` and `m_ponds_pct`, `m_uphill_steps` and `m_uphill_pct` (outside the interiors) and `m_kept_lake_km2`, reported and not barred; relief-indexed limits (A5, owner 2026-11-21): the corridor, crest and kept-river bars take their lowland value (97.5 %, 15 m, 5 %) up to a relief index of 60 m (`relief_index_m`: the window's own, the p50 of the height range of 10 x 10 land blocks of main's `S0` over the region's box, the same for every candidate on the window; lead review of #1648) and their mountain value (95 %, 35 m, 10 %) from 300 m, linear in the logarithm of the relief between (both endpoints proposed by the lead for the owner, O10); `--corridor-bar` fixes the corridor bar by hand, and no bar is keyed on a window | uphill and trunk exceptions correctness; ponds, hump cuts and corner links geomorphology; margin columns diagnostic |
| Step C, first measures | per stage, cells lowered by more than 50 m (`b_<stage>_over50`); KC's patches, count and volume (`b_kc_patches`, `b_kc_patch_km3`) from `kc-patches.csv` in the stage directory (a header `cells,volume_m3`, one line per patch); `relief2_lost_m` and `relief10_lost_m` (the start's relief minus the surface's, positive is lost); `beta_change` (the spectrum slope minus the start's); the bars `bar_spectrum_steep` (beta >= -4.13) and `bar_spectrum_rough` (beta <= -2.05), the range of SPIKE-119's 25 GLO-30 and 3DEP tiles; in `--table`, each row with a graph against the store row of the same window (B; a row's window is its label's part after ` @ `; a start-surface row gets none): `bar_relief_lost` (`relief10_lost_m` at most `max(B's, min(0.03 x relief index, 2 m))`, the relief index the window's shared S0's (`relief_index_m`, else B's `relief10_start_m`; owner 2026-11-23: relaxed for the look, capped at 2 m); a gain of relief is no loss), `bar_beta_change` (`abs(beta_change)` at most B's) and `bar_cost` (`cost_s` at most B's, when both rows carry a timing) | over-50 and patches preservation; spectrum bars geomorphology; `bar_relief_lost` preservation, `bar_beta_change` geomorphology, `bar_cost` product |
| Step C, ridges | on every row, one ridge mask on the start: a cell inside, on land, at least 2 m above both cells 2 L cells away along a face axis or a diagonal (the G0 contracts' section 9); against the reference `S_KH` (the start less the stage directory's `kh-lower.u16`, plus `kh-fill.u16`, in 0.1 m steps) and against `S0` (the start; `ridge_ref` `s_kh`, or `s0` without a KH field, when both are the start): ridge cells lowered more than 20 m, the p95 and the largest lowering over the ridge cells (a raise counts as 0); per channel threshold 1 and 5 km2, cut-through cells (a lowered ridge cell with a cell of the surface's routing draining at least the threshold in its 3 x 3 and none of the reference's routing there, each reference routed as the surface is) and saddles (their 8-connected groups); columns `ridge_cells`, `ridge_lowered`, `ridge_lowered_pct`, `ridge_p95_m`, `ridge_max_m`, `cut1_cells`, `saddles1`, `cut5_cells`, `saddles5` against the reference, `ridge_s0_lowered_pct`, `ridge_s0_p95_m`, `saddles1_s0`, `saddles5_s0` against `S0`; a `RIVER-GATE ridges` line; on a row with a graph the bar `bar_crest` (`ridge_p95_m` at most 15 m, relief-indexed to 35 m); in `--table`, each row with a graph against B's row of the same window: `bar_saddles1` and `bar_saddles5` (`saddles1`, `saddles5` at most B's) and `bar_saddles_s0` (`saddles1_s0` at most B's `saddles1`, B's reference being `S0`; owner decision 3 revisited, G3-1) | preservation |

- budget field: GEN-189's, from the gate's start surface (the relief that sizes the budget is `S0`'s; the stage fields carry the lowering against `S_KH`), on the window's H1 blocks (`--stages` refuses an origin off an H1 corner); the centre L cell of a block is its index `block / 2`; the p50 follows BUILD-057's quantile convention; the 25 km neighbourhood is cut at the window's edge (a gate approximation of the landmass field);
- chain sheet: `cargo xtask stages --chain DIR [--step S] [--view X,Y,SX,SY] [--out FILE]` renders a chain directory as one PNG (`planet_cache::stage_sheet`, the `stage_sheet` example; no spike code);
 - default: the whole window at 12 L cells per pixel, written to `target/review/stages/chain/<label>.png` (every character of the label but a letter, a digit or `-` written as `-`; `chain.png` without a label);
 - `chain.txt`: `stage-chain 1`, then `window X0 Y0 W H` (global L cells), `coarse C` (L cells per H1 cell), `cell_m M`, and optionally `sea_m S` (default 0), `label TEXT` and the run's chain: `commit C`, `switch NAME VALUE` and `stage STEP NUMBER NAME FORM` lines, kept in order (the commit printed in the title band);
 - L grid, little-endian f32, row-major: `base.f32`, `s0.f32`, `kh.f32`, `ks.f32`, `kv.f32`, `final.f32`;
 - H1 grid: `zs.f32` (f32) and `h1-rivers.u8` (1 on a river cell);
 - `graph.txt` (GEN-189 canonical form) and `stages/` (the stage fields above);
 - every file but `chain.txt` is optional: its panel is grey and its label ends `not dumped`; a file of the wrong size is refused, naming the file and both sizes;
- the sheet: a title band (label, sun, km per pixel, a 50 km bar), then the panels in rows of four, each under its label (cut at the panel's width):

| Panel | Content |
|---|---|
| 1 | the base (the H1 base read at L) |
| 2 | `z_s`, held per H1 cell |
| 3 | H1 rivers on `z_s` |
| 4 | `S0` |
| 5 | KH |
| 6 | KS, its change against KH tinted (lowering red, raise blue) |
| 7 | the trunk graph (H1 reaches and L reaches with a parent) on KS |
| 8 | tributaries whose drained area is at least 25 pixels, and basin floors (kept magenta), on KS |
| 9 | KV, its stage fields tinted (lowering red, fill blue) |
| 10 | KC (the final surface), its stage fields tinted the same way |
| 11 | the final surface with the trunks and the drawn tributaries |
| 12 | final minus `S0` (lowering red, raise blue; the sea in its sea colour) |

- one fixed scale for every panel and run:

| Quantity | Value |
|---|---|
| sun | azimuth 315, altitude 10 degrees |
| grey | 0.5 + 2 (shade - sin 10 degrees) per L cell, averaged over the pixel |
| stage tints (panels 6, 9, 10) | full at 20 m |
| change tint (panel 12) | full at 50 m |

- not here: incremental correctness (fresh against incremental) and the owner's flight; the bar for each geomorphology column is BUILD-059's or the plan's.

#### Scenario: A hand graph
- **WHEN** two sources meet at a confluence that runs to the sea and a tributary ends at a junction marker on it
- **THEN** every source reaches the outlet, Strahler counts 3 / 1, the density is the owned steps over the land area, and the validators report nothing (TEST-137)

#### Scenario: B's row
- **WHEN** the gate runs without a graph, stages or partition pair
- **THEN** it prints no G2 column and no classes line

#### Scenario: The chain sheet
- **WHEN** a 32 x 32 chain directory holds every file, its final surface 50 m below `S0` on one half, a trunk, a tributary and an H1 river cell
- **THEN** the sheet has twelve panels; panel 3 marks the H1 river cell, panel 7 the trunk and not the tributary, panel 8 the tributary, panel 12 the 50 m lowering full red and the unchanged half as shade only; a missing file leaves only its panel empty and a file of the wrong size is refused naming it

#### Scenario: The G2 bars and the dual reference
- **WHEN** a row has a graph, an uphill share of 1.0 %, agreement of 90 % and 89 % at its two thresholds, a corridor share of 97.4 % and three missing spill nodes; and stage fields with kh
- **THEN** uphill, the first agreement and the violations pass, the second agreement, the corridor (97.5 %) and the spill nodes fail, the corridor passes at the mountain value 95 %; the budget use counts ks, kv and kc against the budget and adds kh only in the `_s0` columns (TEST-137)

#### Scenario: The gate-integrity bars
- **WHEN** a row has a graph whose agreement's graph share is lower than its routed share, routed cells above a graph head of 9.9 % and 10.7 % at its two thresholds, 2 and then 3 of 200 trunk reaches flagged, a path lowered 15 m through a 15 m reference hump, a 2 x 2 block whose low pair meets only at a corner, a pit in its margin, and a start relief of 10 m and of 400 m
- **THEN** the agreement bar takes the graph share, the first above-head bar passes and the second fails, 2 trunk exceptions pass and 3 fail, one hump cut and one corner link are counted, the margin's pit is in `m_*` and in no bar, and the corridor, crest and kept-river limits are 97.5 %, 15 m and 5 % at 10 m and 95 %, 35 m and 10 % at 400 m (TEST-137)

#### Scenario: A row's chain and the budget's relief
- **WHEN** a `--table` row ends in `#stages=st#chain=c.txt`, `c.txt` naming the label `r1h-proto`, a commit, a switch and a stage
- **THEN** the gate gets `--stages st` and no chain argument, the row is labelled `r1h-proto @ r1h`, the markdown table ends in a `Chains` section listing the three lines, `#reference=` is refused, and no `b_reference` column is printed (the budget's relief is the start surface's)

#### Scenario: The run's own start
- **WHEN** a row gives `--start` and a graph with two H1 reaches and one L reach on a window starting at L (16, 8), H1 cells of 8 L cells, one H1 reach leaving the window
- **THEN** the trunks are the two H1 reaches at their H1 cell centres (H1 (3, 2) at window (11.5, 11.5)), the leaving one cut at its first vertex outside, the L reach no trunk (TEST-137)

#### Scenario: Kept interiors and cells above a head are counted apart
- **WHEN** a routed channel runs down to within one cell of a source's first vertex, a side channel joins a reach away from every source, a routed cell and a graph cell lie in a kept interior
- **THEN** the cells above the source are `routed_above_head`, the side channel's cells off the graph stay misses, and the kept cells leave both shares (TEST-137)

#### Scenario: A kept basin's interior and its river steps
- **WHEN** a kept basin's floor lies in a depression of nine cells (a 3 x 3 bowl), a drained basin's floor in another depression and a kept basin's floor on dry ground
- **THEN** the interior is the bowl's nine cells and only the first basin counts; the four river steps touching the bowl are `kept_steps`, the two rising over its rim `kept_uphill_steps`, and they leave the uphill bar (TEST-137)

#### Scenario: Missing spill nodes in the margin are counted apart
- **WHEN** two basins name a spill node the graph does not hold, one with its floor inside the counted cells and one outside
- **THEN** `g_basin_spill_missing` is 1 and `g_basin_spill_missing_edge` 1, and only the first fails `bar_spill_missing` (TEST-137)

#### Scenario: Step C's first measures
- **WHEN** a stage lowers one cell by 50.1 m and one by exactly 50 m, the stage directory holds two KC patches of 1,000.5 and 20 m3, and a table holds B and a prototype row on R1h where B gains 8.9 m of 1 km relief, the prototype gains 0.55 m, and the prototype runs 900 s against B's 435 s
- **THEN** the stage counts one cell over 50 m, KC's patches are two of 1,020.5 m3, the prototype passes `bar_relief_lost` (a gain is no loss) and fails `bar_cost`; a spectrum slope of -3.5 passes both spectrum bars and the range's ends pass (TEST-137)

#### Scenario: A kept lake on the river
- **WHEN** a river runs through a closed basin that holds a kept basin's floor, on a row with a graph
- **THEN** its river cells count in `kept_river_cells` and `bar_kept_river`, not in `ponds_cells` or `bar_ponds`, and BUILD-055's `basin_cells` still counts them

#### Scenario: A spill mismatch
- **WHEN** a kept basin's depression level at its floor lies more than 0.5 m under its recorded spill
- **THEN** `g_spill_mismatch` counts it and `bar_violations` fails

#### Scenario: A line leaves a kept lake below its spill
- **WHEN** an L vertex on a kept basin's rim has its bed more than 0.5 m under the basin's recorded spill
- **THEN** `g_spill_rim` counts the basin once and `bar_violations` fails; with the beds at the spill it counts none

#### Scenario: The f64 validator finds an overrun
- **WHEN** the stage directory's `budget-violations.csv` holds a line
- **THEN** `b_validator_overrun` counts it and `bar_violations` fails; a missing file prints `-` and adds nothing; a line not of four fields is refused naming the file

#### Scenario: A line ends at an inlet of no kept lake
- **WHEN** a reach ends at a `lake_inlet` whose cell lies in no kept-basin interior, while a reported basin spills to another inlet outside them
- **THEN** `g_inlet_no_basin` counts the first and not the second, and `g_inlet_spill_targets` counts the second

### Requirement: BUILD-061 The cross-section sheet of a chain directory
`cargo xtask transects --chain DIR [--face F] [--out FILE]` SHALL draw, for every river transect lying inside a chain directory's window (BUILD-060's layout), the start surface `S0`, the chain's output and the matched GLO-30 valley across the river on one vertical scale, and SHALL write each profile's valley depth and its widths at 10 m and 50 m above the river level to a table beside the sheet.

Verify: A
Status: active
Source: Lead 2026-11-19 (step C's cross-section sheet; transects re-picked on today's default start, the stale flag), SPIKE-119 (the transects and the GLO-30 match rule), BUILD-060, TEST-141
Detail:
- inputs: `chain.txt` (its `window`, `cell_m`, `label`, `commit`; the face from its `switch window F,...` line, or `--face F`, 0 to 5), `s0.f32` and `final.f32` (each optional: a missing one is drawn and tabled as missing; a file of the wrong size is refused, naming it);
- data in the crate: `crates/cache/data/transects.csv` (window, id, face, the line's ends and the river point in face L cells, drained area, relief along the line, and `s0_depth_m`: `S0`'s depth on it at the pick, by this requirement's measure) and `crates/cache/data/transects-glo30.csv` (the matched GLO-30 profile per transect, heights relative to its river point, with the licence's notice for derived data and its liability sentence);
- the transects: SPIKE-119's rule, 3 a window on R1h, R3a, R3b, M1 and F1, picked on the prototype's `S0` of each window as of 2026-11-19 (the whole window; drained area from `route_rules` on the plain flood, epsilon 1 mm, seed 7); a transect is drawn when its whole line lies inside the window with one L cell to spare; a window with none is refused, naming its face and box;
- a profile: bilinear samples every 0.5 L cells along the line, `n = floor(length / 0.5) + 1`, at the distance `d` from the line's middle;
- the river level: the lowest sample within 250 m of `d = 0`; the depth: the lower of the two sides' highest samples, less the river level (at least 0);
- the width at `h` (10 m and 50 m): the distance between the first sample above the river level + `h` on each side of the river point, linear between samples; open (`>` on the sheet, `N open` in the table: at least `N` m) when a side never rises that high;
- stale: `S0`'s depth under 10 % of its depth at the pick (`s0_depth_m`; the stored relief when the column is absent); no verdict for a pick depth of 0 or without `S0`; the panel's title ends `STALE` in red and the table's `s0` line ends `stale`;
- the sheet: a title band (label, window, commit, and the GLO-30 licence's notice for derived data with its liability sentence, in full), then one panel per transect: `S0` grey and the final surface orange relative to `S0`'s river level, GLO-30 blue dashed relative to its own river point, one vertical scale per panel with its exaggeration printed, and one text row per profile (depth, widths in km);
- output: `target/review/stages/transects/<label>.png` by default (every character of the label but a letter, a digit or `-` written as `-`; `transects.png` without a label) and the table beside it (`.txt`): `TRANSECT label window id profile river_m depth_m width10_m width50_m`, the label's spaces as `-`;
- the command runs under the heavy lock (BUILD-026), as `stages --chain` does; an unknown option, a missing value, a second `--chain` or no `--chain` is refused.

#### Scenario: A V valley's depth and widths
- **WHEN** a chain directory's `S0` is a V valley of 10 m a cell up each side and 100 m cells, a transect crossing it over 56 L cells
- **THEN** its profile has 113 samples, the river level is the floor at `d = 0`, the depth is 280 m, the widths at 10 m and 50 m are 200 m and 1,000 m, and it is not stale against a stored 300 m (TEST-141)

#### Scenario: Stale, open and refused
- **WHEN** `S0` is flat under the same transect, one side of a profile never rises 10 m, `final.f32` is missing, and `chain.txt` has no face line
- **THEN** the table's `s0` line ends `stale`, the 10 m width is open, the final profile is tabled as missing, and without `--face` the command is refused naming `--face F` (TEST-141)

### Requirement: BUILD-062 Companion agent guardrails and unified ID allocator
`cargo xtask agent-guard` SHALL read a `PreToolUse` JSON payload on stdin and emit a JSON decision (`allow`, `deny`, or `force_ask`) enforcing read-only sibling workspaces, zoned writes under `SeriousPlanetData` (`tmp/heavy/**`, `tmp/ag2/**`, `lanes/ag2-*.md`), protected repository files, zero script files (`BUILD-002`), yielding cargo runs while `quiet` or an exclusive machine lock is active, and blocking direct `git push` to `main`/`master`, bare `git push`, force-pushes, and repository deletion while allowing explicit feature-branch pushes and commit messages. `cargo xtask next-id <PREFIX|ADR|SPIKE>` SHALL scan `openspec/`, `docs/`, and optional lane files to print the next contiguous `max + 1` identifier. `.agents/**` SHALL match `.claude/**` in `DOCS_PATTERNS` across both GitHub workflows and `prbody`.

Verify: A
Status: active
Source: ADR 0128 (proposed), CON-25, BUILD-002, BUILD-026, BUILD-044

#### Scenario: Companion guardrails and sequential ID allocation
- **WHEN** `agent-guard` receives writes targeting a read-only sibling repo, an unapproved `SeriousPlanetData` path, `cargo xtask check` while `quiet` is active, a direct `git push` to `main`/`master`, a bare `git push`, or a force-push, and `next-id` scans specs, lanes, and open PRs
- **THEN** read-only paths, unapproved data paths, direct/bare/force pushes, and `cargo` under `quiet` or `exclusive` are denied, explicit feature-branch pushes, commit messages, and owned paths allow, and `next-id` prints `max + 1`

### Requirement: BUILD-063 The code-generated pipeline catalog
`planet_generators::pipeline_catalog` SHALL publish version pins, K-labelled scoped (`global`, `region`, `tile`) pipeline stages, separated M9 layer and K5-K7 commit erosion stages (including K5 `window_cut`), exhaustive `bake::probe::Stage` bindings (`probe_hooks`), preview command contracts, and runtime switch predictions as pure, non-authoritative metadata (`CON-06`, `CON-07`). All cited `spec_ids` SHALL resolve live in `openspec/specs/`, without committing a derived JSON file or changing `cargo xtask stages`. `planet generate --pipeline-json OUT` SHALL resolve a `--seed` or `--world` definition (with `--erosion`, `--channels`, or `--commit DIR`) into its live `HeightField` stack and emit JSON holding `base_node_hash`, `definition_hash`, `active_stages`, `walked_chain`, and the embedded catalog.

Verify: A
Status: active
Source: owner (2026-11-20, Global Authoring & pipeline debugger MVP), ag2-antigravity, BUILD-047, TEST-142

#### Scenario: CLI resolution across erosion and commit modes
- **WHEN** `planet generate --pipeline-json OUT` runs with `--seed`, `--world`, `--erosion on --channels on`, `--commit DIR`, or clashing flags
- **THEN** `--pipeline-json` emits valid JSON naming `base_node_hash`, `definition_hash`, `erosion_mode`, active `implementation_id`s, and the embedded catalog, and clashing options are refused

### Requirement: BUILD-064 The offline pipeline cockpit and preview batch runner
`cargo xtask cockpit` SHALL print living usage documentation and verified examples via `-h` or `--help` (and inside the HTML report), validate schema-v2 pipeline JSON (`BUILD-063`), parse probe rows from `docs/tools/stages/manifest.txt`, index existing PNG headers and text reports across preview and stage directories, and write a single-file offline `index.html` containing a live switch simulator, side-by-side preview comparator, and searchable version-pin and probe ledger using `std`-only code without external assets or network requests (`CON-02`, `CON-06`). With `--run <world|drivers|stages|all>`, `--compare-seed`, or `--chain`, it SHALL plan and execute matching CPU preview, stage-manifest, and transect commands sequentially under the shared heavy lock (`BUILD-026`) before assembling the report.

Verify: A
Status: active
Source: owner (2026-11-20, Global Authoring & pipeline debugger MVP), ag2-antigravity, BUILD-026, BUILD-063

#### Scenario: Offline cockpit assembly and batch preview planning
- **WHEN** `cargo xtask cockpit` runs with `--help`, reads a valid schema-v2 pipeline JSON, or runs `--run all --compare-seed N --chain DIR`
- **THEN** `-h`/`--help` and `--pipeline-json` without `--run` skip the heavy lock, `-h`/`--help` prints live flags and validated examples, HTML indexes PNG dimensions and probe hooks, `--run` builds CPU preview and sheet commands, and invalid flag combinations or outdated schema versions are refused

### Requirement: BUILD-066 G3-8's out-of-sample windows, a seeded stratified draw
`cargo xtask windows-draw [--out FILE] [--threads N]` SHALL draw 10 windows of the five windows' size over the seed-14 world's coarse field by fixed rules and a recorded seed, none overlapping one of the five windows or another drawn window, and write them as data (`crates/cache/data/g3-windows.toml`), so that G3-8's windows are fixed before anyone looks at their terrain and no RG or B run happens on them before G3-8.

Verify: A
Status: active
Source: Lead 2026-11-23 (the 10 out-of-sample windows as a seeded stratified draw, not hand-picked; the G3 gate's rows G3-1 and G3-8, risk R18), BUILD-055, BUILD-060, TEST-149
Detail:
- the candidates: on each face, boxes of 620 x 620 regional cells (of 12,288 a face edge) at `0, 620, 1240, ...` on each axis while they fit, and one more at `12,288 - 620`;
- the samples: 16 x 16 per box, at regular sub-box centres: the class inputs' base elevation (land above 0 m), the orography's precipitation and temperature (runoff by `runoff_mm`), and the latitude of the box's centre;
- a box's statistics: the land share, the p50 and p90 of the land samples' elevation, their mean runoff (mm a year), and the absolute latitude (degrees);
- a box lies in a face edge's band when it comes within 204 regional cells of that edge (the gate's 200-cell margin and the C1 read's 4);
- the strata, the first rule that holds, with their quota:

| Stratum | Rule | Quota |
|---|---|---|
| cube corner | in two edges' bands, land at least 20 % | 1 |
| cube edge | in one edge's band, land at least 30 % | 2 |
| high mountain | in no band, land at least 80 %, land p50 at least 1,500 m | 2 |
| arid or endorheic | in no band, land at least 80 %, land runoff under 50 mm a year | 2 |
| flat coast | in no band, land 20 to 80 %, land p90 under 300 m | 2 |
| mixed mid-latitude | in no band, land 40 to 90 %, latitude 30 to 60 degrees | 1 |

- the draw: per stratum in the table's order, its candidates in the order of a splitmix64 hash of the draw seed (`0x20261123`), the face and the box's corner (ties by face and corner), each taken unless it overlaps one of the five windows or an earlier drawn window, until the quota is met; a stratum short of candidates is refused, naming it and the count found;
- the data file: `world_seed`, `draw_seed`, `size`, and one `[[window]]` per drawn window with its draw `index`, `class`, `window` (`face,i0,i1,j0,j1,seed`) and statistics;
- the edge and corner windows reach the face's edge: the gate measures them once cube faces are defined (G3-5); until then they are data only.

#### Scenario: The draw
- **WHEN** candidates of every stratum are classed and drawn twice with the same seed and once with another
- **THEN** each stratum's quota is met, no drawn window overlaps one of the five windows or another, the same seed draws the same windows, another seed draws others, and a stratum with no candidate is refused naming it (TEST-149)

### Requirement: BUILD-067 The look sheet: a switch row's crops beside the defaults row
`planet_cache::look_sheet::CropWriter` SHALL write a crop store of a chain window: each pre-registered crop cut into a box aligned to the H1 grid and one H1 cell larger on each side, as a chain directory of BUILD-060's layout, for every field the chain hands it. `cargo xtask look-sheet RUN [--against DEFAULTS] [--az A,B] [--out DIR]` SHALL render a store at one L cell a pixel with the chain sheet's fixed-sun shade and colours: without `--against` one sheet an azimuth with a row a crop; with it one sheet a crop and azimuth with the rows defaults, switch and the switch tinted by its difference to the defaults, and `look.txt`.

Verify: A
Status: active
Source: Lead 2026-11-23 (owner 2026-11-23: every switch run comes with its sheet, combined with the stage manifest, the cockpit only where the link is obvious), SPIKE-143 (the crops and the sheet's layout), BUILD-046, BUILD-048, BUILD-060, BUILD-064, TEST-150
Detail:
- the chain hook: `--probe-crops <crops.json>` of the spike branch's chain runner hands `CropWriter` every field as the chain writes it, so the store outlives a deletion of the full dumps (kilobytes a stage);
- crops: SPIKE-143's `crops.json` form (`size` and named `[x, y]` pairs in window L cells; other keys skipped), in the file's order; a side of 1 to 128 L cells; chosen before any stage of the run is looked at;
- the store: `crops.txt` (`look-crops 1`, then `crop NAME X Y SIZE BX BY` a crop);
- a crop's directory: the chain's `chain.txt` with the box's window line and the crop's name after the label, and the box's cut of every L field, H1 field and `stages/` field;
- columns, in order: B (`base.f32`), `S0` (`s0.f32`), `S_KH` (`kh.f32`), KS0 (`ks0.f32`), KS1 with connect (`ks1c.f32`), band6 with connect (`band6c.f32`), KV (`kv.f32`), the final surface (`final.f32`);
- `ks0`, `ks1c` and `band6c` are chain surfaces beyond BUILD-060's layout, which the chain sheet does not draw;
- a stage the run did not write stays grey, labelled `not dumped`;
- the shade: BUILD-060's (central differences, sun 10 degrees high, grey `0.5 + 2 (shade - sin 10)`) at each azimuth given, by default 315 and 135;
- the difference: red lower, blue higher, full at 10 m; no scale is stretched per sheet;
- `look.txt`: per crop and stage the cells, the RMS and the largest absolute difference (m);
- a pair needs the same crops of the same window (each box's place, size and H1 grid);
- output: `target/review/look/<run>/`, 8-bit RGB PNG;
- a sheet over BUILD-048's per-sheet limit is refused, naming it, its size and the limit, and is not written;
- sheets are committed only at a gate, under `docs/tools/stages/sheets/<gate>/`;
- the cockpit (BUILD-064) indexes `target/review/look/` with the stage sheets, the run as the variant;
- the lanes: every switch row reported carries its look sheet pair beside its gate table.

#### Scenario: A switch row's sheet
- **WHEN** a defaults run and a switch run of one window write crop stores of the same crops and `cargo xtask look-sheet SWITCH --against DEFAULTS` runs
- **THEN** each crop has one sheet an azimuth with the defaults, switch and difference rows at one L cell a pixel, a stage the switch did not change has a difference panel equal to its switch panel, and `look.txt` names each stage's RMS and largest difference

### Requirement: BUILD-068 Prepush passes a change's declared golden candidates
`cargo xtask prepush --declared <proposal.md>` SHALL pass a failed tier C when the run ended, the names of the golden candidates it wrote are exactly the goldens of the proposal's `Expected golden changes:` section, and every failed test either panicked at the testkit's golden check (`GoldenChecks`, "N golden check(s) failed") or matches a known class of BUILD-056 by name; otherwise it SHALL fail the step, naming the extra and the missing goldens or the uncovered test.

Verify: A
Status: active
Source: Lead 2026-11-25 (#204: the gap PR #1662 hit, pushed under a stated exception), BUILD-056, ADR 0017
Detail:
- the declared goldens are the names of the section's `- <name>: <reason>` lines up to the next `## ` heading; a line that is not `- <name>:` (a note) is skipped; a proposal without the section is refused;
- the golden check's panic location is read from `crates/testkit/src/lib.rs`, so it tracks an edit of the testkit;
- the candidates are the local adapter's (`target/review/candidates/`), counted by name once whatever the adapter; CI's adapters are golden-review's concern;
- without `--declared`, BUILD-056 is unchanged: any written candidate fails the step;
- the mode writes, copies and blesses no golden: the candidates go to golden-review (ADR 0017, 0061, 0096), and lit, sky and water views to the owner's `/bless`;
- any other argument to `prepush` is refused.

#### Scenario: Exactly the declared candidates
- **WHEN** tier C fails on a golden test's golden check and a known WARP abort, and the candidates are the proposal's declared goldens
- **THEN** the step passes and prints the declared list, the golden tests and the known classes for the PR body

#### Scenario: An extra or a missing candidate
- **WHEN** a candidate is not declared, or a declared golden wrote no candidate
- **THEN** the step fails, naming both lists

### Requirement: BUILD-069 A geometric gate or look column is registered only with a passing calibration
A geometric gate or look column SHALL be registered (added as a bar or a target) only with a calibration entry in `planet_cache::metric_calibration` that passes TEST-155. Each metric's band SHALL be stated in the Detail of its own requirement. A column registered before this requirement SHALL carry a calibration report: a failing report is an owner item, not a removal. The owner's visual sign-off is not a column and is not bound.

Verify: A
Status: active
Source: Lead 2026-11-25 (the lead's rule after the owner's caution and the external process review; look bars included, the owner's visual sign-off not, lead's review of #1665), TEST-155, CON-31
Detail:
- `metric_calibration::registered()` lists the entries registered under this rule; the tier-A test fails if one of them fails its calibration; an entry without a band (report only) cannot be registered;
- `metric_calibration::reported()` lists the entries that predate it; their calibration runs and writes its table, and its verdict is printed, not asserted;
- the first reported entries and their bands (a band holds a defect-free valley's reading):

| entry | reading | band |
|---|---|---|
| `corner-links-added` | the counted 2 x 2 blocks that are corner links (`river_suite::corner_link`, the predicate `corner_links` counts) in the surface and not in the reference | 0 to 0 |
| `hump-cuts` | `river_suite::hump_cuts` | none (report only: the count depends on the background's humps) |
| `spectrum-slope-change` | `river_suite::spectrum`'s beta, the surface's minus the reference's | none (report only: a valley on a flat background has its own spectrum) |
| `xdiag-in-zone` | SPIKE-143's M1 X_diag over the zone's counted cells (the grid's border cells left out): d1 = (z(x+1, y+1) - 2 z + z(x-1, y-1)) / 2 binned by (x - y) mod 8, d2 = (z(x-1, y+1) - 2 z + z(x+1, y-1)) / 2 binned by (x + y) mod 8; per diagonal the largest share of its squared energy in two adjacent bins (cyclic); X_diag = their mean / 0.25 - 1 | -1 to 0.010 (the chainsaw target) |
| `xdiag-in-zone-weighted` | the same per-diagonal shares, weighted by each diagonal's energy: `(e1 l1 + e2 l2) / (e1 + e2) / 0.25 - 1` | -1 to 0.010 |
| `m4-along-added` | M4 along the channel path (zone cells within 4 cells of its segments; k = z(p + t) - 2 z(p) + z(p - t), bilinear, t the nearest segment's unit direction; RMS), the surface's minus the reference's (m) | -0.5 to 0.5 |
| `d8-sinuosity` | the channel path's length over its chord, at least 1 (a rounding below 1 reads 1) | 1.00 to 1.02 |
| `straight-run-share` | the share of the channel path's length in runs of one repeated step longer than 2 km | 0 to 0.5 |

- a reported entry that fails is not changed or removed by the calibration (CON-31); its report goes to the owner.

#### Scenario: A new metric without a passing calibration
- **WHEN** an entry is added to `registered()` and its calibration fails
- **THEN** the tier-A test fails, naming the metric and the first case that breaks TEST-155's rule

#### Scenario: A column that predates the rule
- **WHEN** a `reported()` entry's calibration fails
- **THEN** the test passes, the table and verdict are written for the owner, and the gate's column is unchanged

### Requirement: BUILD-070 The planet library's bake tests take slots in proportion to their work
The nextest profile SHALL give each of the planet library's bake tests that take 10 s or more in `test fast` (named one by one in one override, filter `package(=planet) and kind(lib)`) `threads-required = 4`. It SHALL NOT give them a test group, change their slow-timeout (the default's 180 s) or change any test body.

Verify: A
Status: active
Source: Lead 2026-11-26 (after four prepushes of #1662 timed them out; tracked in #1666), BUILD-003, BUILD-045
Detail:
| Run (`planet-render`, `planet-editor`, `planet`, `xtask`; 816 tests; idle machine) | Total | Slowest bake test |
|---|---|---|
| before (no override) | 106.5 s | 95.6 s (`generate_bakes_the_class_source...`) |
| after (`threads-required = 4`) | 99.9 s | 68.1 s (`the_base_under_a_commit...`) |
- Before, with other work on the machine, two to four of them ran past 180 s (2026-11-25).
- A test group is not used: BUILD-003's software GPU tier rule keeps every test group to `gpu_all` tests, so tier A and B are not capped by groups.
- The list is the planet tests of 10 s or more in the before run; a new heavy bake test joins it.

#### Scenario: The override
- **WHEN** `.config/nextest.toml` is read
- **THEN** one override names the bake tests, sets `threads-required = 4`, and sets no slow-timeout and no test group

### Requirement: BUILD-071 The look sheet's step rows: each stage beside its step from the previous stage
`cargo xtask look-sheet RUN --steps [--az A,B] [--out DIR]` SHALL render, for each crop of a crop store (BUILD-067) and each azimuth, one sheet `<crop>-step-azNNN.png` with one row per stage of the run in the run's order. Each row SHALL show the stage under the chain sheet's fixed-sun shade at one L cell a pixel, beside that shade tinted by the stage's difference to the previous stage. The command SHALL also write `steps.txt`. The stages SHALL be the store's own list.

Verify: A
Status: active
Source: Lead 2026-11-26 (the river-graph park plan, step 4: the owner's item to keep sheet generation on main), lead #184i (`step_row.py`, spike branch 7d6d11c3), BUILD-067, BUILD-048, TEST-158
Detail:
- the stage list: the store's `stages.txt`:
  - its form: `look-stages 1`, then one `stage NAME LABEL` a stage, in the pipeline's order;
  - it is written by `CropWriter::stage(NAME, LABEL, surface)`, which cuts an L surface of the window as `NAME.f32` into every crop, as the writer gets it;
  - NAME is that file's name: letters, digits, '-' and '_', unique ignoring case; LABEL takes no whitespace;
  - a new writer at a store's root removes the root's `stages.txt`;
- a store without `stages.txt` takes BUILD-067's columns, those its first crop holds, in their order;
- refused, naming the store:
  - a stage listed twice, ignoring case;
  - a listed stage that a crop does not hold, naming both;
  - a store with fewer than two stages;
- `--steps` together with `--against` is refused (a pair's sheets are BUILD-067's);
- the shade and colours are BUILD-067's, at its azimuths;
- the difference: the stage minus the previous stage, red lower, blue higher, full at 2 m (`STEP_M`); no scale is stretched per sheet; the first row's difference panel is grey, labelled `first`;
- `steps.txt`: `look-steps 1` and `run LABEL`, then one line a crop and step: `CROP NAME PREV cells N mean_m M rms_m R max_abs_m X`;
- output: BUILD-067's directory and form; a sheet over BUILD-048's per-sheet limit is refused, naming it, its size and the limit, and is not written.

#### Scenario: A run's stages
- **WHEN** a crop store holds three stages written by `CropWriter::stage` and `cargo xtask look-sheet RUN --steps` runs
- **THEN** each crop has one sheet an azimuth with three rows, a lowered step is tinted red and a raised one blue, an unchanged stage's difference panel equals its stage panel, and `steps.txt` names each step's signed mean, RMS and largest difference

### Requirement: BUILD-072 One command archives a finished milestone's log and spikes
`cargo xtask archive-milestone <M> [--before D] [--dry-run]` SHALL do the housekeeping skill's mechanical archive steps for milestone `M`, compute every new file text before it writes any, and delete only the log fragments it merged.

Verify: A
Status: active
Source: Lead 2026-11-27 (the P3 closure plan's S1 and D3's precondition; both reviewers), the housekeeping skill's step 2, BUILD-010
Detail:
| Step | What it does |
|---|---|
| log | the fragments of `docs/log.d/` (dated before `D`, when given) merged into `docs/archive/<M>/log.md` in `cargo xtask log`'s order (newest first; an existing archive's entries merged in, its header kept), then the merged fragments deleted |
| spikes | the rows of `docs/spikes.md`'s index whose status starts with `closed`, byte for byte, into `docs/archive/<M>/spikes.md` (appended after an existing table's last row, a table header first when it has none; a row it already holds is not added again) |
| pointers | `docs/archive/<M>/log.md` added to the header sentence of `docs/log.md` that lists the archives, and `docs/archive/<M>/spikes.md` to the last header line of `docs/spikes.md` that names an archive, each only when missing |
| `--dry-run` | the plan printed (the files written, the fragments deleted), nothing written |
- Refused before any write: a malformed fragment (BUILD-010), a header without the sentence or line the pointer goes on, a milestone name with other than letters, digits and `-`, an unknown option.
- A second run with the same arguments has nothing to do.
- An existing log archive is rewritten as its header (the lines before its first entry) and its entries; a fragment dated `D` stays; files are written with LF line ends.
- Nothing else is deleted: no change archive, no image, no ADR.

#### Scenario: A milestone archived
- **WHEN** the command runs for `M2` before 2026-02-01 on a docs tree with three fragments (one later) and a register with two closed, one open and one planned spike
- **THEN** the archive holds the two entries newest first, the later fragment stays, the two closed rows move byte for byte, the pointers are added once, and a second run has nothing to do

### Requirement: BUILD-073 Retired requirement IDs live in one committed file, independent of the change archive
`openspec/retired-ids.txt` SHALL list every requirement ID that is in no main spec any more, one a line, sorted by prefix and number (`#` lines are comments). `cargo xtask next-id` SHALL count it and SHALL NOT read `openspec/changes/archive/`, and spec-lint SHALL check it.

Verify: A
Status: active
Source: Lead 2026-11-27 (the P3 closure plan: D3's precondition, the change archive deletable without an ID issued again), BUILD-062
Detail:
| spec-lint fails on | |
|---|---|
| the file's form | a line that is not `PREFIX-NNN`, an ID twice, an ID out of order; a missing file |
| reuse | a requirement of a main spec, or one an open change adds (`## ADDED`), under an ID of the file |
| completeness | a requirement of the change archive (any delta section) in no main spec and not in the file |
- The file was generated from the change archive on 2026-11-27: 55 IDs.
- A change archived with a REMOVED requirement, or a RENAMED one whose ID changes, adds the old ID to the file in the same PR (spec-lint names it otherwise).
- An example ID in spec prose counts for next-id like a real one: examples use placeholders.
- Leaving the archive out changes no requirement prefix's next ID: the archive's only other tokens are prose.

#### Scenario: An ID only in the file
- **WHEN** a main spec's highest ID of a prefix is n, the file holds n + 8 of that prefix, and the change archive n + 18
- **THEN** next-id gives n + 9 (the archive's n + 18 does not count), and without the file it gives n + 1

### Requirement: BUILD-074 The plan command checks a sprint file
A milestone file with an `Opened:` line SHALL be read as a sprint file (ADR 0130 point 1); any other keeps BUILD-053's form.
`cargo xtask plan` and `cargo xtask spec-lint` SHALL fail a sprint file whose header, exit rows, records or decisions break the shape and the limits of the scenarios below: at most 7 days, at most 5 exits, a record for each promoted or parked row, at most 3 open decisions, and only safe defaults taken on silence (A1).
`plan` SHALL print `overdue` past `Closes by`, and the decisions past their `By` date.
`plan --pr` SHALL check one PR against the R/P firewall and the overdue rule.

Verify: A
Status: active
Source: ADR 0130 points 1-6 (proposed, applied while proposed; owner 2026-11-27); amendments A1 and A4 (lead, 2026-11-27); lead 2026-11-27 (the draft state)

#### Scenario: The shape
- **WHEN** a sprint file is read
- **THEN** it holds a `Status:` line starting `active`, `draft` or `closed`; one line `Opened: <date>   Closes by: <date>   Owner look: <date> | none   R&D lane: <lane>` (dates `YYYY-MM-DD`); an `Objective:` line; an `Out of scope:` line with its `- ` lines; under `## Exit items` a table with the columns #, Item, Mode, Done when, Gate, Park, Lane, State, Result; under `## Decisions` a table with the columns #, Decision, Lead's recommendation, Default, By, State

#### Scenario: The header
- **WHEN** a date is missing or not a calendar date, `Closes by` is before `Opened` or more than 7 days after it, `Owner look` is neither a date nor `none`, `R&D lane` or `Objective` is empty, or `Out of scope` names nothing
- **THEN** plan names the line and fails

#### Scenario: The exit rows
- **WHEN** there is no exit row or more than 5, an exit ID twice, an empty Item, Done when, Gate, Park, Lane or State cell, a mode other than R, P, X, M, a Gate part (`;`-separated) not `<class>: ...` with a class of `num`, `func` (or `fun`), `dom` or `look`, a state other than `open`, `running`, `look-pending`, `waiting-owner`, `done <date>`, `promoted <record>`, `parked <record>`, `carried <M>`, or a promoted row that is not P
- **THEN** plan names the exit and the cell and fails

#### Scenario: The records
- **WHEN** a promoted or parked row's record (a bare name in `docs/milestones/<M>/`, a path with `/` from the repository root) is missing or has no table with the columns criterion, class and verdict; a promotion record has a `fail` or empty verdict; or a promotion or park record takes an exception on a `num` criterion
- **THEN** plan names the exit, the record and the criterion and fails

#### Scenario: The decisions
- **WHEN** more than 3 decisions are open (state empty, `open` or `waiting-owner`), a decision has no Default or no `By` date, or a decision in state `default taken ...` has a default other than park, keep-off or keep today's
- **THEN** plan fails, printing `capped` for the first; past `By`, an open decision is printed as firing when its default is park, keep-off or keep today's, and otherwise as `waiting-owner`

#### Scenario: The queue
- **WHEN** an active sprint's exit is open (not done, promoted, parked or carried) and no queue row of `docs/status.md` names it
- **THEN** plan fails as in BUILD-053

#### Scenario: The firewall
- **WHEN** `plan --pr TITLE [--branch B] [--body-file F] [--files F] PATH...` names an R or P exit that is not promoted (in the title or on a body `Exit:` line) and a path neither docs-only nor under `spikes/`, a `change/*` branch names no X or M exit and no promoted P exit, or the sprint is past `Closes by` and the PR is not docs-only and does not change the sprint file (the stand-in for closing, carrying or parking an exit)
- **THEN** it fails with a line per refusal; docs-only paths are `docs/`, `openspec/`, `.claude/`, a root `*.md`, `spikes/**/results/**` and `spikes/README.md`; without an active sprint file it checks nothing

#### Scenario: Not checked
- **WHEN** a sprint relies on the gate-ledger park counters, a criterion's date set against the exit's first ledger row, A2's `decision:`/`look:` rule or the capped-state row limit
- **THEN** plan does not check it (the combined plan's item 3b and the lead's reading)

### Requirement: BUILD-077 The milestone archive command also moves a closed sprint's file, report and records
`cargo xtask archive-milestone <M>` (BUILD-072) SHALL also move the sprint file `docs/milestones/<M>.md`, when its first `Status:` line starts with `closed`, together with its report `docs/milestones/<M>-report.md` and the files of its records folder `docs/milestones/<M>/`, into `docs/archive/<M>/`.

Verify: A
Status: active
Source: ADR 0130 point 7 (proposed, applied on the owner's word of 2026-11-27); the combined sprint process plan's item 7 (lead, 2026-11-27); BUILD-072
Detail:
| From | To |
|---|---|
| `docs/milestones/<M>.md` | `docs/archive/<M>/<M>.md` |
| `docs/milestones/<M>-report.md` (when it exists) | `docs/archive/<M>/<M>-report.md` |
| `docs/milestones/<M>/<path>` (every file, at any depth) | `docs/archive/<M>/<path>` |
- The old milestone form moves the same way (P3: `P3.md` and `P3-report.md`, once `P3.md`'s status says closed).
- A sprint file that is not closed, or has no `Status:` line, stays with its report and records; the run prints why, and its log and spike steps still run.
- A move onto an existing file, onto a file the same run writes (the log or spike archive), or onto another move's destination refuses the run before any write or move.
- The emptied records folder is removed; `docs/milestones/` stays.
- The live files that still name a moved path (`docs/` outside the archive, `openspec/specs/`, `.claude/`, `xtask/src/`, the root `*.md`) are printed as `<file>: <old path>`, not rewritten.
- `--dry-run` prints the moves and writes nothing; a second run has nothing to move.

#### Scenario: A closed sprint archived
- **WHEN** the command runs for a closed sprint with a report, a record and a nested records file, while `docs/status.md` names the sprint file
- **THEN** the four files are under `docs/archive/<M>/` byte for byte, the records folder is gone, `docs/status.md` is printed and unchanged, and a second run moves nothing

### Requirement: BUILD-075 The PR-body check also needs the review lines by path, for records and for the active sprint
`cargo xtask pr-body-check` (BUILD-044, the pr-gate check) SHALL also refuse a PR whose body lacks a review line its changed paths need, a record's research review or owner look, or the active sprint's `Sprint: <M>` line; and it SHALL run the active sprint's R/P firewall (`cargo xtask plan --pr`, BUILD-074) on the PR's title, branch, body and changed paths.

Verify: A
Status: active
Source: ADR 0130 point 6 (proposed, applied on the owner's word of 2026-11-27); the combined sprint process plan's item 6; the lead's ruling of 2026-11-27 on when each line applies; CLAUDE.md "How to work" points 3 and 4; BUILD-044
Detail:
| Line | Needed when | Form |
|---|---|---|
| `Numerics: PASS <sha>` | a PR with a non-docs path changes `crates/core/`, `crates/generators/` or a `*.wgsl` | the sha (7 to 64 hex digits) a commit of the PR, as BUILD-044's verifier line |
| `Spec-Guardian: PASS <sha>` | a PR with a non-docs path changes `openspec/` | as above |
| `Research-Review: READY <sha>` | a PR adds or changes a record: `docs/milestones/<M>/<E>-record.md`, or a `spikes/<name>/results/results.md` whose `Decision:` line says `promote` (the file at the PR's head) | as above |
| `Owner-Look: yes` or `Owner-Look: no` | as `Research-Review` | the verdict as the first word (`yesterday` is none); the date and the sheet follow |
| `Sprint: <M>` | a sprint file is active: `docs/milestones/<M>.md` with `Status: active` in the sprint form (an `Opened:` line); not in the old milestone form, a draft or between sprints | `<M>` the active sprint's name |
- A docs-only PR (BUILD-044's paths) needs no path line, `openspec/` included; it still needs the record lines and the `Sprint:` line.
- A record deleted at the PR's head (moved to the archive) and a file under `docs/archive/` are not records.
- Every problem is one line naming the path and the reviewer to run; the check lists them all (the verifier line, the review lines, the firewall's), then fails.
- The PR's title and head branch come from `PR_TITLE` and `HEAD_REF` in the environment, like the body.

#### Scenario: A generator PR without its numerics line
- **WHEN** a PR changes `crates/generators/src/erosion.rs` and its body has `Verifier: PASS <sha>` but no `Numerics:` line, or one naming no commit of the PR
- **THEN** the check fails naming the path and the numerics-reviewer agent; with `Numerics: PASS <sha>` of a commit of the PR it passes
