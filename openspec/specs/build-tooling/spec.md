# build-tooling Specification

## Purpose
The workspace and its tooling: the layered crates with downward-only dependencies and the graphics API confined to `render` (CON-02, ADRs 0004, 0008, 0015), the single `cargo xtask` command surface and test tiers (CON-25, ADR 0009), the three run modes (CON-01), CI on Windows and Linux (CON-26) and the licence policy (CON-27).

## Requirements

### Requirement: BUILD-001 Layered crates
The workspace SHALL contain the crates `core`, `world-def`, `generators`, `cache`, `streaming`, `frame`, `render`, `editor` and `app` such that every crate depends only on crates below it in that order, and `core` through `frame` SHALL have no graphics crate (wgpu, naga, winit, ash, egui) anywhere in their dependency tree. No crate other than `render` SHALL list a graphics-API crate (wgpu, wgpu-core, wgpu-hal, naga, ash) in any dependency section, dev and build dependencies included, and `render`'s public API SHALL NOT expose a wgpu type; `render`'s public API is the seam to the graphics API (ADR 0004).

Verify: A
Status: active
Source: CON-02, ADR 0004, ADR 0008, ADR 0015

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

#### Scenario: Stale local main
- **WHEN** a branch is checked out in a worktree whose local `main` is behind `origin/main`
- **THEN** the changed files are those of `origin/main...HEAD` (and the working tree), not of `main...HEAD`; `main` is used only when there is no `origin/main`

#### Scenario: Shared crate changed
- **WHEN** `testkit`, `perf` or a root manifest changed, or nothing changed
- **THEN** `test fast` selects every crate

#### Scenario: Slowest tests named
- **WHEN** a nextest JUnit report of a `test fast` run is read
- **THEN** the five tests with the greatest times are listed, slowest first, each as its binary and test name

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
