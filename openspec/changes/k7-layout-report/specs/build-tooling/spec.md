## ADDED Requirements

### Requirement: BUILD-052 K7 changes report their difference from an alternate tile layout
`cargo xtask k7-layout` SHALL run K7 on the region bake fixture under the canonical layout and the alternate layout (half the tile side, same halo) from one regional solve, unquantised, and SHALL print: the cells differing in bits and their share, the p50, p99 and maximum height difference in metres, and the cells over 0.25 m and over 1 m, for all cells and for the cells off the channel lines, in total and by distance to the nearest alternate seam (0 to 7, 8 to 31, 32 to 127, 128 or more cells). `cargo xtask prepush` SHALL run it when the diff touches K7, print the committed baseline beside it, and print `REVIEW` when the p99 or maximum exceeds twice the baseline. The exit code SHALL be 0.

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
