## Context
- GEN-142 to GEN-145 were measured on windows and a landmass at 3,072 cells a face edge. The erosion grid is 12,288 cells (814 m, cell area about 0.5 to 0.66 km^2, 16 times the cells of the 3,072 grid). The region is face 1, i 8056 to 8675, j 760 to 1379 (results.md region 1 times 4).

## Method
- `examples/region_scale`, release, 8 threads, round rules, true cell metric, the carve parameters of pass 1 (`CarveParams::default()`), 4 rounds of 5 steps, seed 14. The window is the region plus `margin` cells on every side; its border cells are outlets (an artificial cut). H1 runs on the carved surface with the runoff weights and the runoff of the orographic climate, default `HydroParams`.
- Memory is the process working set read by PowerShell (Windows): the peak so far after each phase. The process holds `z0` and `z` (16 bytes a cell) beside the solver's arrays.

## Gates (every window: 0)
| Margin (cells) | Window | G-ROUND | G-DET (1 and 8 threads) | G-PART, tile 256 | G-CON | Q at confluences, at the outlets | `A_d` at the outlets |
|---|---|---|---|---|---|---|---|
| 200 | 1,020 x 1,020 = 1.04e6 | 0 cells differ | 0 | 16 tiles, 6 sweeps, 49 floods: 0 receivers, 0 `A_d` differ | 0 disconnected, 0 broken, 0 miscovered | 4.3e-16, 8.5e-15 | 4.8e-15 |
| 1,200 | 3,020 x 2,580 = 7.79e6 | 0 | 0 | 132 tiles, 8 sweeps, 333 floods: 0, 0 | 0, 0, 0 | 4.4e-16, 1.4e-14 | 5.8e-14 |
- The G-CON and conservation rows are repeated at margins 600, 2,400 and 4,000 (`mem` mode): 0 disconnected, 0 broken, 0 miscovered; errors at most 5.6e-16 (confluences), 8.0e-14 (outlets), 5.8e-14 (`A_d`).
- Times at margin 1,200: rounds 21.9 s on 1 thread and 21.0 s on 8, whole-region carve 24.7 s, resident route 4.5 s, partitioned route 12.8 s.
- The region's change after 4 rounds is stable across windows: p50 -45.5 m (margin 200) and -45.8 m (1,200); p5 -399.2 and -409.3 m; p95 8.7 m; max 18.8 m.

## Channel density and basins (T = 75 km^2 = about 150 to 160 cells, H1 on the carved surface)
| Margin | Land cells | Channel cells | Heads a 1,000 km^2 of land | Segments a 1,000 km^2 | Basins kept (lakes, endorheic) | Artefacts | Ramps |
|---|---|---|---|---|---|---|---|
| 200 | 724,570 | 48,833 (6.7 %) | 18.6 | 35.3 | 86 (80, 6) | 10 | 0 |
| 600 | 2,158,143 | 166,551 (7.7 %) | 21.1 | 40.1 | 135 (131, 4) | 23 | 1 |
| 1,200 | 4,260,254 | 334,945 (7.9 %) | 21.6 | 41.4 | 241 (228, 13) | 38 | 2 |
| 2,400 | 7,592,546 | 534,879 (7.0 %) | 18.7 | 35.8 | 485 (463, 22) | 123 | 8 |
| 4,000 | 14,482,501 | 990,077 (6.8 %) | 17.5 | 33.4 | 989 (936, 53) | 284 | 16 |
- At the erosion grid 7 % of land cells are channel cells (39.5 % at 3,072 cells, where T was 7 cells): the 3,072-cell share was a grid effect.
- The artefact criterion of these runs (below 1 km^2 and shallower than 2 m) is live at this grid (22 to 29 % of the basins found), unlike at 3,072 cells. The numbers above are under that area limit; the h1-corrections change replaced it with 4 cells and 2 m (margin 200: 18 artefacts of 96 basins found).
- The largest mouth in the 4,000-margin window carries 9,376 m^3/s.

## Peak memory (`mem` mode; 4 rounds then the H1 pass; the H1 pass stays under the rounds' peak)
| Margin | Cells | Peak after the 4 rounds | Bytes a cell | Time, 4 rounds on 8 threads |
|---|---|---|---|---|
| 200 | 1.04e6 | 168 MiB (the baseline) | - | 3.2 s |
| 600 | 3.31e6 | 294 MiB | 93 | 10.2 s |
| 1,200 | 7.79e6 | 645 MiB | 87 | 21.1 s |
| 2,400 | 20.5e6 | 1,732 MiB | 89 | 43.1 s |
| 4,000 | 44.3e6 | 3,784 MiB | 90 | 90.6 s |
- About 90 bytes a cell (74 without the example's two f64 height copies): the resident solver's arrays, as A3 section 6.6 estimated (64 bytes a cell for the carve, plus the weights).
- Time is about 2 microseconds a cell on 8 threads.

## Bound for the whole landmass at 12,288 cells
- The landmass of GEN-144 has 9,308,154 land cells at 3,072; at 12,288 it has about 149e6 land cells and 1.6e6 coast-ring cells (16 times).
- Resident rounds: 150e6 cells at 74 to 90 bytes = 11.1 to 13.5 GB. At the upper figure it exceeds the proposed 12 GB (ADR 0115). Time at 2 microseconds a cell: about 5 minutes.
- Not measured: the cube grid with a mask holds arrays over all 906e6 cells of the sphere (about 80 GB at these sizes) and does not fit; only a compact landmass layout would run it. The bound extrapolates the window measurements, which have a plane grid and no cube edges.

## Earth-specific values (ADR 0108 point 4)
| Value | Earth (example) | Parameter |
|---|---|---|
| Cells a face edge | 12,288 (814 m) | the example's `CELLS`: the erosion grid |
| Region | face 1, i 8056 to 8675, j 760 to 1379 | `REGION`: results.md region 1 times 4 |
| Tile | 256 cells | the example's `tile` argument |
| Margins | 200 to 4,000 cells | the example's `margin` argument |
| Radius, seed | 6,371,000 m, 14 | the planet's, the world's |
| Initiation, pit limit, artefact limits | 75 km^2, 0.5 m, 1 km^2 and 2 m (these runs; now 4 cells and 2 m) | `HydroParams` defaults (GEN-145) |
