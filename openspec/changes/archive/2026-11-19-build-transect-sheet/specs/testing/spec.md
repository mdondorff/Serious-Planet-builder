## ADDED Requirements
### Requirement: TEST-141 The cross-section sheet on fixtures
Tier-A fixtures SHALL check the cross-section sheet's measures (BUILD-061) against a valley whose depth and widths follow from its geometry, and its table, stale flag and refusals.

Verify: A
Status: active
Source: Lead 2026-11-19 (spikes #150, #150a), BUILD-061
Detail: a V valley of 10 m a cell on 100 m cells, crossed over 56 L cells: 113 samples from -2,800 to 2,800 m, river level 0 at `d = 0`, depth 280 m, widths 200 m and 1,000 m, not stale at a stored relief of 300 m; a profile flat on one side has an open 10 m width and depth 0, stale at 300 m, and no verdict without `S0`; with a stored pick depth of 20 m a depth of 3 m is not stale and 0 m is, and a pick depth of 0 m gives no verdict; a GLO-30 profile of five samples (40, 20, 0, 5, 30 m) has depth 30 m and a 10 m width of 170 m; the sheet and table are written for the one transect inside face 3's window, the final surface's river 5 m lower, and the label's space written as `-`; a flat `S0` makes the transect stale in the table and a missing `final.f32` is tabled as missing; no face line without `--face` is refused, a face holding no transect is refused naming it, an `s0.f32` of 12 bytes is refused naming it, and a csv without a column or with a non-number names it; the command passes `--chain`, `--face` and `--out` to the example and refuses an unknown option, a missing value, a second `--chain` and no `--chain`.
