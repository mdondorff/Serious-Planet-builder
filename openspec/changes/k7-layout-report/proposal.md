# k7-layout-report

Why: ADR 0119 point 5 (owner, 2026-11-12): K7's result is defined over its canonical tile layout, and an alternate layout need not be bit-identical. On every K7 change its difference from the canonical layout is reported (cells differing, p99, maximum, distance to the seam); a material rise is a regression to review. Today the figure exists only as the spikes lane's local probe (`spike/k7-two-tiling`: tile (65, 8) of face 1, one tile of 1,024 cells against four of 512, halo 128; 0.57 % of cells differ, p99 1.1 mm, maximum 0.31 m).

What: `cargo xtask k7-layout` runs the K7 pass of the region bake fixture under the canonical layout and under the alternate (half the tile side, same halo) from one regional solve, in memory and without the store's 0.25 m quantisation, and prints the difference. `cargo xtask prepush` runs it when the diff touches K7 (`crates/generators/src/bake/k7*.rs`, `carve.rs`, `round.rs`, `region_bake.rs`) and prints the lines; the PR body carries them. A committed baseline line (`docs/gates/k7-layout-baseline.txt`) is printed beside the run, and a p99 or a maximum above twice the baseline prints `REVIEW` (a report, never a failure: ADR 0119 point 5 asks for review, not a gate).

Out of scope: the G-SEAM gate on the four terrain types (A3 gate row, work list item 4), the face-edge rule (REND-063), the tier B thread test (BUILD-051 covers the bake at 1 and 8 threads).

Requirement ID: BUILD-052 (bulldozer, 2026-11-12). Source: ADR 0119 point 5, architect 2026-11-12 (work list item 3), due 2026-11-19.
