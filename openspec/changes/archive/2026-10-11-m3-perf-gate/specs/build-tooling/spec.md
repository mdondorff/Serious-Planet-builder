## MODIFIED Requirements

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
