# SPIKE-117: the combination of ADR 0123 (4-neighbour drain at 814 m, one round at L, the L route)

Opened: 2026-11-14
Question: does SPIKE-116's 4-neighbour hand-over at 814 m (with B's residual damped near channels) followed by one round of the regional kernel at L meet the river bars on R1h, R3a and R3b, and what does the edit path's L round cost over the dirty catchments?
Why it matters: draft ADR 0123's acceptance requires the combination to meet the bars (lead, 2026-11-14, crusader.md #135).
Time box: half a day of agent work.
Success criteria: per window, with the basin metric agreed with spikes (a basin is at least 0.5 m deep and 16 L cells, under a fill with epsilon 1e-7 m a cell; the 5 cm count beside it): basins on at most 10 % of the river cells, canyons over 50 m at most 1 %, lakes at D = 20 m named, relief at 200 m and 1 km kept, edge agreement with H1 named, the share of channel steps along a grid axis against B alone, cost within 12 GB; the L round over the dirty catchments of a ridge edit timed against the full round.
Fallback if it fails: Schott 2024 with per-scale retargeting (SPIKE-118, spikes).
Code location: spike branch `spike/combo-117` (crusader), example `crates/cache/examples/erosion_at_l.rs` (`PLANET_TRANSFER=d4`); never merged.
Windows (agreed with spikes): R1h face 1, 8056-8675 x 760-1379, seed 14; R3a face 4, 368-987 x 4720-5339, seed 15; R3b face 2, 9968-10587 x 8304-8923, seed 15; 620-cell cores, margin 200. My earlier "flat windows" (face 4 and face 2, seed 15, 1,020-cell boxes) are the larger boxes around R3a and R3b, so the five windows of the order are these three.

## Method
- `erosion_at_l` (cache example, spike branch). Region front as `bake_region`. Start surface at L:
  - **D4 (`PLANET_TRANSFER=d4`):** SPIKE-116's hand-over. The solved 814 m surface is breached over 4 neighbours (0.01 m a step, floor 0.005 m) and read bilinearly at L. B's sub-814 m residual is added, damped to 0 toward the channels of that surface over 400 m. A land cell stays at or above 0.005 m.
  - **Plain (`PLANET_TRANSFER=none`):** `B + regional`, as in SPIKE-115.
- Then one round of the regional kernel at L over the window, D8 routing at L (plain flood rules), channels at 75 km2, and SPIKE-114's network levels.
- **Basin metric (agreed with spikes for SPIKE-117 and SPIKE-118):** a basin is a connected depression at least 0.5 m deep and 16 L cells in area, under a priority flood with epsilon 1e-7 m a cell; reported as the share of river cells inside the tiles that lie in one. The SPIKE-114 count (any pit over 5 cm) is beside it.
- **Canyon:** the network level under the eroded surface, over 50 m.
- **Axis grooves:** the share of channel steps along a grid axis (4-neighbour steps), against a routing of B alone on the same window.
- **Edit path:** the L round re-run over the bounding box of the dirty catchments of a ridge edit (+60 m, 16 km, south of lake 30; R1h only), with the box's border and the sea as outlets.

## Numbers (inside the region's tiles; 620-cell cores, margin 200, 8 threads)
| Window | variant | basins >= 0.5 m, >= 16 cells (bar 10 %) | 5 cm count | canyon > 50 m (bar 1 %) | lakes at D = 20 m | relief 200 m / 1 km | edge: H1 crossings within 2 km | steps along a grid axis (B alone) |
|---|---|---|---|---|---|---|---|---|
| R1h (face 1, seed 14) | D4 + round | 2.06 % | 22.3 % | 0 % | 0 % | 115 / 110 % | 42 of 44 | 66.5 % (47.5 %) |
| R1h | round only | 1.13 % | 6.5 % | 0 % | 0 % | 113 / 111 % | 36 of 44 | 47.4 % |
| R3a (face 4, seed 15) | D4 + round | 0.00 % | 71.3 % | 0 % | 0 % | 121 / 117 % | 43 of 84 | 50.2 % (44.4 %) |
| R3a | round only | 1.78 % | 33.4 % | 0 % | 0 % | 120 / 117 % | 34 of 84 | 42.6 % |
| R3b (face 2, seed 15) | D4 + round | 0.20 % | 21.3 % | 0 % | 0 % | 110 / 110 % | 73 of 93 | 68.6 % (47.3 %) |
| R3b | round only | 4.57 % | 15.0 % | 0 % | 0 % | 110 / 111 % | 74 of 93 | 45.5 % |
- Canyon max under the eroded surface: 4.6-9.7 m on every window and variant.
- Wall per window: D4 hand-over 71-83 s, the L round 63-77 s, flood and route 81-107 s. Peak commit (R1h, D4, the whole run): 6.06 GB of the 12 GB budget.

| Edit path (R1h, ridge +60 m) | D4 + round | round only |
|---|---|---|
| dirty catchments | 1 (171,452 cells; box 664 x 606) | 8 (15,788 cells; box 204 x 157) |
| L round over the dirty box | 0.31 s | 0.02 s |
| full L round of the window | 67 s | 63 s |
| Barnes 2016 local part (one tile) | 0.09 s | 0.09 s |
| box re-route equal to the full route | 54 % of the dirty cells | 53 % |
- The dirty-box round and re-route are not exact against the full region (the box's border is an outlet); the exact path is Barnes 2016 over the round's result. The cost of the round over the dirty catchments is under a second here.

## Verdict
Both variants meet the bars on all three windows with the agreed basin metric:
- basins 0.0-4.6 % of the river cells (bar 10 %);
- no canyon over 50 m;
- no lakes at D = 20 m;
- relief 110-121 % of the start.

The SPIKE-114 5 cm count, which made the flat windows look failed, counts shallow pits; the agreed metric does not.

The 4-neighbour hand-over (D4) lowers basins on R3a and R3b (1.78 % to 0.00 %, 4.57 % to 0.20 %) and raises edge agreement on R1h and R3a (36 to 42, 34 to 43). But it aligns the rivers to the grid axes: 66.5 % and 68.6 % of channel steps on R1h and R3b, against 47 % for B alone and for the round only. That is the axis-aligned groove risk of ADR 0123, now measured.

Recommendation for ADR 0123: one round at L alone meets the bars with no axis bias. Adopt D4 only if the axis bias can be removed (for example an 8-neighbour breach that keeps the bilinear read drainable). The edit path's first part, the L round over the dirty catchments, costs under a second.

## Visuals
None: a pure measurement on three windows.
