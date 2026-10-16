## MODIFIED Requirements

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
