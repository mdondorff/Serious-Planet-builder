## MODIFIED Requirements

### Requirement: BUILD-004 Three run modes
The single binary SHALL provide the modes `editor`, `generate` and `test-render`, each of which starts and exits cleanly with `--smoke`, and the walking skeleton (generate a tile, plan a frame, execute it, produce pixels) SHALL run in every mode: `test-render` and `editor --offscreen` (the window arrives in M2) produce the same frame, and `generate` produces the tile hashes and CPU dumps.

Verify: A
Status: active
Source: CON-01, ADR 0008

#### Scenario: Smoke start
- **WHEN** each mode is started with `--smoke`
- **THEN** it prints a line containing `smoke ok` and exits with status 0

#### Scenario: Skeleton in all modes
- **WHEN** the tiles scene is rendered by `test-render` and by `editor --offscreen` with the same options
- **THEN** the two PNGs are identical

#### Scenario: Unknown mode
- **WHEN** an unknown mode or option is given
- **THEN** the program exits non-zero and prints the usage text

