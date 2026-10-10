# build-tooling Specification

## Purpose
TBD - created by archiving change m0-foundations. Update Purpose after archive.

## Requirements

### Requirement: BUILD-001 Layered crates
The workspace SHALL contain the crates `core`, `world-def`, `generators`, `cache`, `streaming`, `frame`, `render`, `editor` and `app` such that every crate depends only on crates below it in that order, and `core` through `frame` SHALL have no graphics crate (wgpu, naga, winit, ash, egui) anywhere in their dependency tree.

Verify: A
Status: active
Source: CON-02, ADR 0008

#### Scenario: Upward dependency
- **WHEN** a crate lists a crate at its own or a higher layer under `[dependencies]`
- **THEN** `cargo xtask layering` fails and names both crates and the chain

#### Scenario: Graphics crate in a GPU-free tree
- **WHEN** a GPU-free crate gains a transitive graphics dependency
- **THEN** `cargo xtask layering` fails and names the crate and the dependency

### Requirement: BUILD-002 Single command surface
All build, test and tooling commands SHALL be available through `cargo xtask`, the repository SHALL contain no `.sh` or `.ps1` files, and the performance command SHALL refuse to run unless the owner's machine-ready flag is given.

Verify: A
Status: active
Source: CON-25, CLAUDE.md

#### Scenario: Script file added
- **WHEN** a `.sh` or `.ps1` file exists in the repository outside `target/`
- **THEN** the repository check test fails and lists the file

#### Scenario: Perf without confirmation
- **WHEN** `cargo xtask perf` runs without `--machine-ready`
- **THEN** it exits with an error that names the owner confirmation and runs nothing

### Requirement: BUILD-003 Tiered test commands
`cargo xtask test fast` SHALL run tier A tests for the changed crates and every crate above them, `cargo xtask test` SHALL run tiers A and B, and `cargo xtask test gpu` SHALL run tier C; tiers SHALL be selected by test-binary name (`gpu_*` is tier C, `oracle_*` tier B, all others tier A) on the software adapter unless `--real` is given.

Verify: A
Status: active
Source: ADR 0009, report §16

#### Scenario: Change in a middle crate
- **WHEN** only files of `streaming` changed
- **THEN** `test fast` selects `streaming`, `frame`, `render`, `editor` and `app`

#### Scenario: Shared crate changed
- **WHEN** `testkit`, `perf` or a root manifest changed, or nothing changed
- **THEN** `test fast` selects every crate

### Requirement: BUILD-004 Three run modes
The single binary SHALL provide the modes `editor`, `generate` and `test-render`, each of which starts and exits cleanly with `--smoke`; headless PNG output of `test-render` is covered by TEST-004 and REND-007.

Verify: A
Status: active
Source: CON-01, ADR 0008

#### Scenario: Smoke start
- **WHEN** each mode is started with `--smoke`
- **THEN** it prints a line containing `smoke ok` and exits with status 0

#### Scenario: Unknown mode
- **WHEN** an unknown mode or option is given
- **THEN** the program exits non-zero and prints the usage text

### Requirement: BUILD-005 CI on both operating systems
Continuous integration SHALL run check, spec-lint, licence check, tiers A to C and the run-mode smoke starts on Windows (software adapter WARP) and on Linux (software Vulkan lavapipe) for every pull request and every push to `main`.

Verify: A
Status: active
Source: CON-26, report §9

#### Scenario: Workflow content
- **WHEN** the workflow file is parsed by the repository check test
- **THEN** it names both a Windows and a Linux runner and invokes `cargo xtask check`, `spec-lint`, `test` and `test gpu`

### Requirement: BUILD-006 Licence policy
Dependency licences SHALL be checked by `cargo deny` against an explicit allow-list in `deny.toml`, and CI SHALL run that check.

Verify: A
Status: active
Source: CON-27

#### Scenario: Policy present
- **WHEN** the repository check test runs
- **THEN** `deny.toml` exists with a licence allow-list and the CI workflow invokes `cargo deny`
