# Gate ledger (BUILD-041)

One row per measurement of a gate of `docs/design/terrain-target.md` section 12.1. Verdicts: pass, fail, partial, reported, not run. A value without a commit on main is `not run`. The commit is the squash commit of the PR that carried the measurement; the spikes rows (e4c03141, SPIKE-106) measured main at 8b57df69.

| gate | seed | region | resolution | commit | value | verdict | lane |
|---|---|---|---|---|---|---|---|
| G-SURF | 14 | prototype region, one tile | 102 m local cells | a561a419 | at the cell centres 5.7e-14 m before quantisation, half a step after; owner 2026-11-09 sets the gate at the centres (b5a5a6d2) | pass | explorer |
| G-SURF-I | 14 | prototype region, one tile | 102 m local cells | a561a419 | 192 of 28,900 lattice vertices (0.66 %) over 0.25 m, p99 0.22 m, maximum 2.55 m | reported | explorer |
| G-CON | 14 | prototype region | 12,288 cells/face edge | 2e5453ea | 0 disconnected, Q at confluences 4.3e-16, Q at outlets 8.5e-15, A_d at outlets 4.8e-15 | pass | explorer |
| G-CON | 14 and 15 | prototype region and four hills-and-plains windows | 12,288 cells/face edge | e4c03141 | 0 channels end mid-course, own channel set equals the library's on all five windows, Q at confluences 4.3e-16 to 4.4e-16 | pass | spikes |
| G-CON | 14 | prototype landmass, production phase plan, two k=4 runs | 12,288 cells/face edge, 149.3e6 cells | 1094d82a | hydro::verify at k=4: Q at outlets 1.2e-14, Q_w closure 1.5e-14, confluences 5.7e-16 and 4.4e-16, A_d at outlets 5.4e-13, 0 disconnected, broken or miscovered; identical in both runs | pass | explorer |
| G-CON | - | prototype landmass | 3.26 km | 4fa17ae5 | exact on the landmass (hydrology-h1 log) | pass | explorer |
| G-PROFILE | - | - | - | - | no measurement on main | not run | - |
| G-LOCAL | - | - | - | - | no measurement on main | not run | - |
| G-ROUND | 14 | prototype region | 12,288 cells/face edge | 2e5453ea | 0 cells differ on windows of 1e6 to 44e6 cells | pass | explorer |
| G-ROUND | 14 and 15 | prototype region and four hills-and-plains windows | 12,288 cells/face edge | e4c03141 | 0 cells differ, 20 steps of 5 and 12 steps of 4, 1, 2, 5 and 8 threads | pass | spikes |
| G-EQ | 14 | prototype region, E1-E7 | 12,288 cells/face edge | f6adbdf1 | 0 cells differ against the fresh solve, f64 against f64; canonical f32 forms, K7 and the sampled surface not in the comparison | partial | explorer |
| G-INC | 14 | prototype region, E2a and E2b | 12,288 cells/face edge | f6adbdf1 | tile floods 6.5 % and 9.8 % of a fresh build (bound 10 %); the routing work beside them is recomputed whole; no tier-A test of the bound | partial | explorer |
| G-INC | 14 | prototype region, E2 | 12,288 cells/face edge | ef408031 | E2 edits flood 4.6 % of the tile floods (example measurement) | reported | explorer |
| G-BASIN | 14 | prototype region, E3a-E3d | 12,288 cells/face edge | f6adbdf1 | E3 merge, split and trench fixtures run with the continuity map and Q_w closure at most 4.3e-14; the 1e-9 level rows of A3 5.5 and the high and zero E_open fixtures are not listed | partial | explorer |
| G-SED | 14 | prototype landmass, 4 rounds | compact grid, 9.4e6 cells | f38737d9 | deposition balance per catchment and per round 3e-14 to 4e-14; round-internal sediment and the partition test not measured | partial | explorer |
| G-SED | 14 | corner-cap landmass fixture | 64 cells a face edge | 86c8422b | per-catchment terms, heights and report equal under compact against masked cube and 1, 3, 8 threads (bit for bit); groups of whole catchments add up to the whole (1e-12) with the identity under 1e-9 and the sums matching the heights read another way; no spatial cut of the pass exists (open); round-internal sediment not measured | partial | explorer |
| G-LAND | 14 and 15 | four hills-and-plains windows | 12,288 cells/face edge | e4c03141 | unkept pit cells 0.008 % to 0.035 % of land (bound 1 %); relief p50 and hypsometry against the whole-region carve not measured here | partial | spikes |
| G-SCALE | 14 | prototype landmass, complete resident build on the phase plan (complete-build row) | 12,288 cells/face edge, 148.9e6 land cells | 1094d82a | peak commit 11,363 and 11,131 MiB in two runs (worst 11,363 against the 11,444 gate, margin 81 MiB, run spread 232 MiB), working set 10,861 and 10,645 MiB; peak in hydrology; spill 1.19 GB deleted; hashes equal in both runs; plan 5,354 and 5,257 s; the other A3 6.6 scenarios not run | pass | explorer |
| G-STORE | 14 | new chain, real pack records | at Earth scale (extrapolated) | 39aff58b | 17.4 B a land cell, about 3.9 GB at Earth (round records 2.7 GB); field bake and kind 13 on the new chain not measured; legacy sum about 39.6 GB | partial | explorer |
| G-SCALE | 14 | prototype landmass, 4 resident rounds | 12,288 cells/face edge, 149.3e6 cells | 9b5a3e65 | peak 7,858 MiB (55 bytes a cell) in 1,716 s on 8 threads; the H1 pass not run at this size (81 bytes a cell extrapolated to 12.1e9 bytes); the H0-split fallback, H0 at 2x radius and the scaling model not measured | partial | explorer |
| G-SCALE | 14 | prototype landmass, partitioned rounds | 12,288 cells/face edge, 149.3e6 cells | 6807ad71 | peak 8,854 MiB (62 bytes a cell) in 2,974 s; bit-equal to the resident rounds at 3,072 cells | partial | explorer |
| G-RT | - | - | - | - | no measurement on main | not run | - |
| G-DET | 14 | prototype region | 12,288 cells/face edge | 2e5453ea | 1 against 8 threads 0 cells differ; Windows against Linux, resident against partitioned over edits and the CI manifest fixture not measured | partial | explorer |
| G-DET | 14 and 15 | prototype region and four hills-and-plains windows | 12,288 cells/face edge | e4c03141 | 1 against 2, 5 and 8 threads 0 cells differ | partial | spikes |
| G-PART | 14 | prototype region | 12,288 cells/face edge | 2e5453ea | 16 tiles, 6 sweeps, 49 floods, 0 filled, 0 receivers, 0 A_d differ | pass | explorer |
| G-PART | 14 and 15 | prototype region and four hills-and-plains windows | 12,288 cells/face edge | e4c03141 | tiles 128 and 256, z0 and carved, 0 differ; a one-cell-wide spiral corridor 257 x 257 and 401 x 401 bit-equal to the resident route and Dijkstra | pass | spikes |
| G-PART | - | prototype landmass | 9.3e6 land cells | 44980cab | G-PART exact on the landmass routed whole (cube-landmass log) | pass | explorer |
