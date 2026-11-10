# region-sed-land (briefing item 13, step 4), example only

Architect 2026-11-10 (#1295 verdict point 2): G-SED and G-LAND on R3a and R3b; G-BASIN stated.

## What
- `region_scale` prints a G-LAND line (pit cells of the window = cells in kept basins + unclassified; the unclassified are artefacts and ramps; their share of the region's land against the 1 % rule) and a G-SED line (the sediment pass on a copy of the carved surface: eroded = deposited + exported, the heights' volume against deposited, thermal and pit spreading volume change, pits after each step).
- The probe takes one bin a cell (the window's cell count) so the sheets are 1 cell a pixel (architect, #1295 point 3); the library bound is untouched, the example passes it. The Pass2 seed is the world seed, as production.
- No requirement text, no library code, no test: an example is a measurement tool.

## Not in this change
- G-BASIN's E3 fixtures (merge, split, spill, continuity maps) need two surfaces and a basin history; a region window has one H1 call. Stated in the results, not measured.
- The per-catchment residual and the partition test: GEN-152 and GEN-155 test them on fixtures through their own entry points.
