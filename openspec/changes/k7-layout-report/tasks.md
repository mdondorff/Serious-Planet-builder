# Tasks: k7-layout-report

## 1. The function (tier A)
- [ ] 1.1 `region_bake`: split the front half of `bake_region` (solve, hydrology, graph, channels) into a function both the bake and the report call; the bake's bytes are unchanged (BUILD-051's manifest lines equal before and after).
- [ ] 1.2 `region_bake::layout_difference(spec, base, climate, alt_tile) -> LayoutDiff`: K7 per land tile of each layout (`k7_tile`, unquantised `z`), the common cells, the line cells of the channels; the rows of BUILD-052.
- [ ] 1.3 Tests (spec: BUILD-052): the same layout twice gives 0 differing cells; a halo that is too small gives differences concentrated at the seam; 1 thread equals 8 threads; the bands partition the cells.

## 2. The command
- [ ] 2.1 `cargo xtask k7-layout` on the determinism fixture (a few seconds); `--spec` is not an option in this PR.
- [ ] 2.2 `docs/gates/k7-layout-baseline.txt`: the baseline line; `REVIEW` above twice its p99 or maximum; exit 0.
- [ ] 2.3 `cargo xtask prepush` runs it when the diff touches K7 files and prints the lines (the PR body carries them).

## 3. Close
- [ ] 3.1 The real-region figure (tile (65, 8), halo 128) is the spikes' probe: its numbers are quoted in the baseline file's comment as the reference this fixture is not.
- [ ] 3.2 Verifier, spec-guardian, numerics-reviewer (generators), archive, log fragment.
