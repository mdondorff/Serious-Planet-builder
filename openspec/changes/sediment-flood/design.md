## Finding (inbox #111, #113; 2026-11-09)
- The sediment pass's pit statistics, its route and its deposition sweep flooded with the plain `priority_flood`; the rounds, routing and H1 flood with the canonical one (GEN-141).
- The plain flood drains a flat chain (a cell within an epsilon of its parent's level) in a FIFO before it pops the heap: a cell beside a lower outlet is lifted to the end of the chain's epsilon ramp. Fixture (tier A): a chain of 7 cells between two sea cells; the cell beside the second sea cell is 8 epsilons above it (0.16 m) on the plain flood and 1 epsilon on the canonical one; the sediment pass reported 55 pit cells on the fixture before the change and 0 after.
- The measured gap grew with scale: the pit count of the pass against H1's basin cells on the surface after the rounds was 1.00 times at 3,072 cells, 2.20 times at 6,144 (853,418 against 387,448; the filled surfaces differ up to 11.8 m inside one depression of 381,313 cells) and 18.5 times at 12,288 (27,567,979 against 1,491,672). The outlet sets and the epsilon were identical; only the flood's tie rule differed.
- The pit shares reported earlier for the complete build (18.5 % before, 17.6 % after the pass, 13.5 % unclassified) are void.

## Decisions
1. `sediment::canonical_surface` floods with `priority_flood_with(..., true)` and routes with `route_rules` under the canonical flood and no MFD clip; `pit_cells`, `sediment_pass` and `catchment_balance` use it. `deposit_on_own_route` (called by pass 2 and K7) keeps the plain flood: the default bake is bit for bit as before, no kernel version moves.
2. The cost: every flat cell enters the heap (the canonical flood has no FIFO queue); the time of the sediment pass at 6,144 cells is in the table below.

## Measurements (`landmass_scale <k> 8 full diag`; threshold 0.5 m; after the change)
| Item | k = 1 (3,072 cells, 9.31M land) | k = 2 (6,144 cells, 37.2M land) |
|---|---|---|
| pit cells after the rounds, share of land | 95,318 (1.024 %) | 387,448 (1.041 %) |
| H1 basin cells on that surface | 95,318 | 387,448 |
| pits after thermal (maximum) | 96,656 (609.7 m) | 488,891 (920.0 m) |
| pits after deposition | 24,230 | 130,862 |
| pits after the pit spreading (maximum) | 24,258 = 0.261 % (5.0 m) | 130,957 = 0.352 % (344.5 m) |
| depressions after the pass; p50 / p99 depth | 516; 4.49 / 4.87 m | 1,079; 4.59 / 109.6 m |
| H1 on that surface: basin cells = pit cells; lake / endorheic / in none | 24,258; 90.5 / 9.1 / 0.4 % | 130,957; 89.9 / 10.0 / 0.07 % |
| not a pit before the pass (created by it) | 52 cells (0.21 %) | 8,538 cells (6.52 %) |
| deposition: eroded / deposited / exported (m3) | 1.5686e16 / 4.3553e15 / 1.1331e16 | 1.1865e16 / 3.0566e15 / 8.8083e15 |
| hash of z after the pass, before and after the change | 2c5d03cbb983063b, e0a14921fa9a77e0 | not run before; 73eee20e40602255 |
| sediment pass wall time | 6 minutes with the machine shared | 462 s with the machine shared |
| peak working set / commit, whole build | 793 / 822 MiB | 2,766 / 2,886 MiB |
- At k = 1 the totals and the pit counts equal the plain flood's to the digits printed; the surface hash moves (the deposition's route is canonical now).
- G-SED at k = 1 (`sedround`): the identity holds in 4 rounds; largest error of a catchment 4.25e-14, of the region 1.21e-14.
- Thermal's deepest pits are unchanged by the flood: at k = 1 the deepest (609.7 m) is a one-cell canyon floor of about 100 m between walls of 1.5 to 2 km; thermal raises the floor cells by unequal amounts and leaves a dam. At k = 2 thermal's maximum is 920 m and 344.5 m remain after the deposition. Not fixed here.
- The k = 4 reading comes with the phase-plan runs (#109).

## Reach of the plain flood, not changed here
- `fill_depth` (`flood.rs`): no caller outside tests.
- `k7.rs` `pit_stats` (the report) and `deposit_on_own_route` (the sweep): called by K7, pass 2 (`tile.rs`) and the k7_region example; pass 2 is the default bake. Moving them changes stored L and G-SURF's reading; the architect decides after l-store (merged).
- `route` (route.rs) without rules: pass 1 and 2 of the default bake.
