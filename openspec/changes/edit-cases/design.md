## Context
- A3 section 6.5: a re-solve of an edit is a full region re-route executed with memoised work: tile floods (GEN-143) and catchment rounds keyed by their inputs. Section 12.1: G-EQ (the re-solve is the fresh build of the edited definition) and G-INC (the work of what changed), after each edit E1-E7, with the structural gates.
- Inbox #102 (the ADR review pass, SPIKE-105 section 21): three defects of merged prototype code are fixed here: the basin inflow ignored upstream losses, the lake balance counted rain on flooded cells twice, and the partitioned route and the rounds had no runoff weight.

## Decisions
1. The memo of a catchment round is keyed by the hash of its member cell ids, its key inputs (`CatchmentInputs`: heights, receivers, `A_d`, distances, taper, class factor, uplift, outlet flags) and the parameters of the carve that the round reads. The routing before it is the partitioned route per round with memoised tile floods; the change counts of a round read against the same round of the previous solve (`PartCache::use_slot`).
2. Drivers follow `carve_rock`: the class factor multiplies `K dt`, an uplift per cell is `k_dt U` before the taper, the runoff weights start the area. Tier A: bit equality with `carve_rock` for each driver alone and together.
3. `Q_w` (A3 section 5.5): the cascade over the cells in descending filled level (ties by cell id). A cell passes its `Q_w` to its receiver, except an exit cell of a kept basin: its flow is the basin inflow `Q_in` and stops there. The basin `q_out` enters at its spill cell, reached after all the cells of the basin (they stand above the spill level). A chain of basins resolves in one pass, and the window that guarded against flow re-entering a basin is not needed.
4. `E_open = PET - (P - r)` (m/s, per cell, non-negative) is the evaporation input; the example computes it from the orographic climate and the runoff of GEN-129 (`max(0, .)` only for rounding).
5. Intentional basins: a basin that holds a flagged cell is class `Intentional`, kept whatever its size, with `overflows` giving its balance. The continuity map (A3 section 5.4) lists, for each old basin, the new basins by shared cells; `Hydro.label` gives the cell to basin map.
6. G-EQ here is bit equality of f64 with f64 on the complete result: the rounds, and the channels, `Q`, `Q_w`, segments, basins, levels and labels of H1 from the partitioned route against the resident one. The canonical forms (f32 rounds, 0.25 m levels) are not built (A3 section 9, SPIKE-105 section 21 item 6).
7. Edit fixtures at 12,288 cells are the 3,072-grid cells of results.md times 4. The E3 basins are found on the unsolved surface; E3d is the basin of the lowest cell of the fixture (floor 440 m, spill 1,185 m here; 457 m and 1,187 m in the fixture), its floor cell flagged. E3c splits basin A by a ridge across it above its spill level (the reverse of E3b). E7 is the shortest crossing (a bridge of 20 sea cells, 3 wide, raised to 25 m) from the large landmass to the second one in the window (a 90-cell island), then taken away.

## Measurements (`examples/edit_cases`, release, 8 threads, window margin 400 = 1420 x 1420 = 2.02e6 cells at 12,288 cells, tile 256 (36 tiles); base: 4 rounds, 430 tile floods, 14,800 catchment rounds, 5.49e6 cells carved)
Every case: **G-EQ 0 cells differ** against the fresh solve, the H1 result from the partitioned route against the resident route (the whole result of decision 6) **0 of 6 parts differ**, **G-PART 0**, G-CON 0 disconnected / 0 broken / 0 miscovered, `Q` at confluences at most 4.4e-16, `Q_w` closure at most 4.3e-14, `Q_w` at confluences at most 4.4e-16, `A_d` at the outlets at most 2.9e-14.
| Case | Cells changed (base result) | Tile floods of 430 | Catchments carved of 14,800 | Cells carved of 5.49e6 |
|---|---|---|---|---|
| E1 raise a range by 1,000 m | 132,745 | 85 (19.8 %) | 48 (0.32 %) | 36.2 % |
| E2a lower the pass by 100 m | 20,038 | 28 (6.5 %) | 21 (0.14 %) | 3.1 % |
| E2b raise a ridge line by 200 m | 35,804 | 42 (9.8 %) | 25 (0.17 %) | 5.6 % |
| E3a cut 25 m at the spill of A | 128,575 | 95 (22.1 %) | 7 (0.05 %) | 23.4 % |
| E3b a trench joins A and B | 153,945 | 102 (23.7 %) | 8 (0.05 %) | 27.0 % |
| E3c a ridge splits A | 73,357 | 61 (14.2 %) | 11 (0.07 %) | 24.0 % |
| E4a raise a 50 km block upstream by 500 m | 283,488 | 187 (43.5 %) | 45 (0.30 %) | 41.7 % |
| E4b +50 % rainfall over the block | 67,340 | 52 (12.1 %) | 14 (0.09 %) | 23.6 % |
| E5 double the rainfall of a window | 83,458 | 84 (19.5 %); round 1: 0 | 18 (0.12 %) | 22.2 % |
| E6a softer lithology (0.29 to 0.1) | 93,533 | 57 (13.3 %) | 35 (0.24 %) | 29.4 % |
| E6b uplift x 2 | 86,062 | 57 (13.3 %) | 27 (0.18 %) | 29.3 % |
| E7a join two landmasses (90 cells, 20-cell bridge) | 156 | 12 (2.8 %) | 170 (1.15 %) | 0.02 % |
| E7b take the bridge away | 0 (the base result) | 0 | 0 | 0 |
- The gate of A3 (E2 at most 10 % of a full routing) is met by the tile floods only: E2a 6.5 %, E2b 9.8 % (the routing work beside them is recomputed whole). The tile floods count every flood of every sweep: a tile floods several times in a round, so a round can show more floods than tiles. In the first round alone E2a floods 3 tiles and changes 1.
- One catchment holds most of the land (largest 1.4e6 cells), so the carved cells are 3 to 42 % of the cells while few catchments change.
- E5 floods no tile in round 1 (no height change); the later rounds flood where the carve then differs. Initiation moves with no height change: channel cells 100,956 to 101,469, heads 15,249 to 15,339, segments 28,971 to 29,136.
- Time: the memoised solve of an edit with the base pass through the cache before it takes 29 to 40 s; the fresh solve 6.4 to 9.8 s on 8 threads. The edit alone is not timed. The offers, receivers, roots, order and area of each round are recomputed whole (A3 section 6.5: not yet an incremental routing), so the memo saves the carve and the tile floods only.
- E3 (unsolved surfaces): basin A floor 1 m at (8319, 849), spill 727 m, 18,179 cells, a lake; B floor 1 m at (8379, 896), spill 580 m, 1,004 cells, a lake.
  - E3a: the spill falls 25 m to 702 m, the id stays, the class stays Lake (its inflow of 791 m^3/s exceeds its evaporation: the endorheic start of the fixture does not hold at this grid and climate).
  - E3b: one basin with the id of A (both floors are 1 m, the tie goes to the lower cell id), spill 580 m; the continuity map sends A (13,676 cells shared) and B (1,004) to it.
  - E3c: the part with the floor of A keeps its id (11,957 cells); the other part is new (5,854 cells).
  - E3d: the named basin is kept as `Intentional` through every edit (E1 raises it to a floor of 1,440 m: the same lowest cell, the same id).
- E4: the trunk (8390, 958) with `A_d` 87,261 km^2 drains to the outlet (8370, 1110) (`A_d` 170,342 km^2, `Q` 1,619 m^3/s). A 500 m block 120 cells upstream on the main stem dams it: `Q` at the trunk falls from 829 to 3 m^3/s, at the outlet from 1,619 to 786 m^3/s (the same outlet, `A_d` 82,654 km^2); a 50 % rise of the rainfall of the block gives 1,619 to 1,634 m^3/s. 187 tile floods and 45 catchments for the dam, 52 and 14 for the rainfall.
- E7: the second landmass is a 90-cell island near (8450, 1108): the bridge changes 156 cells; the channel and basin counts do not change.

## Open
- The caches grow without bound: the memo of catchment rounds keeps the carved heights of every definition it saw (about 8 bytes a cell a round), and `PartCache` keeps one `(filled, receiver, area)` copy per round slot (about 20 bytes a cell each). At the prototype size this is small; at a face of 1.6e8 cells it is not. An eviction rule (keep what the last solve hit or inserted) is needed before production.
- A memo hit is checked against the member count only; a 128-bit collision of equal size would return the heights of another catchment.

## Earth-specific values (ADR 0108 point 4)
| Value | Earth (example) | Unit, source | Parameter |
|---|---|---|---|
| Planet radius, sea level, seed | 6,371,000 m, 0 m, 14 | m, Earth state; world definition | the example `R`, `FaceSpec` |
| Flood epsilon | 1 mm | m, calibration | `CarveParams::epsilon` |
| H1 presets | initiation 75 km^2, pit limit 0.5 m, 4 cells, 2 m, 16 curve steps | calibration (GEN-145) | `HydroParams` |
| Carve | `k_dt` 0.05, taper 60 m, breach 1e-3 and 500 m | calibration (GEN-100) | `CarveParams::default()` |
| Grid, region, window | 12,288 cells; face 1, i 8056 to 8675, j 760 to 1379; margin 400 cells | calibration or the fixture | the example `CELLS`, `REGION`, `margin` |
| Tile | 256 cells | calibration or the fixture | the example `tile` |
| Edit sizes | +1,000 m over 64 x 64 cells; -100 m over 8 x 8; +200 m along a 100-cell line; -25 m over 5 x 5; +500 m over 61 x 61; x2 rainfall; hardness 0.29 to 0.1 (K x 2.9); uplift x 2; a 3-wide bridge at 25 m | calibration or the fixture | results.md E1-E6 scaled by 4; E7 the example |
| Base rock | K 1, uplift 20 (`u_over_k`) | calibration or the fixture | `CarveParams::default()` |
| Runoff | `runoff_mm`, `discharge_weight` (reference 300 mm a year), `E_open = PET - (P - r)` | calibration or the fixture | GEN-129, GEN-117 |
| Seconds a year | 31,557,600 | calibration or the fixture | the example |
