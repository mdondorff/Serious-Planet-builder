# SPIKE-114: rivers routed at L inside a region (R3a)

Opened: 2026-11-13
Question: inside a region, do river lines routed at L (102 m) on the drained start surface meet the 1 % bar for line cells cut over 50 m without breaking agreement with H1 (814 m) at the region's edge?
Why it matters: #125 showed that most deep cuts come from H1 lines crossing L ridges (13 % of line cells cut over 50 m with the lines placed, 6.75 % straight; draining alone 10 %). The ADR on rivers at L inside regions (owner, 2026-11-13: "rivers at 100 m inside regions, with crusader's test first") waits on these numbers.
Time box: 1 day of agent work.
Success criteria: per variant (drain D = infinity, all breach; D = 20 m, lakes kept): line cells cut over 50 m at most 1 % inside the commit's tiles; no water-surface rise over 0.25 m; at the region's edge every H1 inflow and outflow matched by an L river within a stated distance, the discharge ratio per outlet named, and water sent to another valley than H1 named per outlet; wall time and peak commit within 12 GB; the size of the L geometry if stored and the time to rebuild it from `bake_base()`.
Fallback if it fails: keep H1's network and route at L only within a corridor (placement) with the lakes rule, or route at L with H1's outlets as fixed exits.
Code location: spike branch `spike/rivers-at-l` (crusader), examples in `crates/cache/examples/`; never merged.

## Method
- `rivers_at_l` (cache example, spike branch): R3a's front (seed 14, face 1, margin 200); K7's start surface `B + regional` sampled at every L cell of the region window (8,160 x 8,160 = 66.6 M cells).
- One priority flood over the window (border and sea as outlets), D8 routing at L with the plain flood rules, and D8 accumulation of area and discharge (the H1 cell's runoff over each L cell; no open-water losses).
- Channel cells: drainage area at least H1's channel threshold (75 km2).
- Network levels as in A'': the running minimum of the drained surface down the channel cells. D = infinity breaches every pit; D = 20 m keeps pits deeper than 20 m as lakes (water at the spill).
- Edge: channel steps crossing the region's border (its cells without the margin), against H1's segment steps crossing it.
- Rebuild: the same routing on `B` alone (what a reader has from `bake_base()`).

## Numbers (R3a, inside the commit's 52 tiles unless named)
| | D = infinity (all breach) | D = 20 m (lakes kept) |
|---|---|---|
| channel cells | 184,118 | 184,118 (99,820 in lakes, 54 %) |
| cut under the start surface p50 / p95 / p99 / max | 2.2 / 88 / 207 / 738 m | -1.9 / 4.8 / 11.0 / 20.0 m |
| line cells cut over 50 m (bar 1 %) | 10.55 % | 0.00 % |
| ground rises over 0.25 m | 0 of 183,891 steps | 43,893 (under lakes) |
| water rises over 0.25 m | 0 | 268 |

| Region edge (H1 crossings with Q >= 1 m3/s: 44, of them 10 out; L crossings 56, 9 out) | |
|---|---|
| H1 crossings with an L crossing of the same kind within 2 km | 36 of 44 |
| discharge ratio L / H1 where matched | 1.20-1.39 (L carries no open-water losses) |
| not matched (water elsewhere) | in (69411, 7739) Q 158 m3/s: nearest L inflow 0.3 m3/s at 3.9 km; out (64675, 11035) Q 20: L 2.8 at 5.2 km; out (64875, 11035) Q 6 at 15.2 km; in (69411, 6987) Q 16: L 4.3 at 2.9 km; out (69403, 6947) Q 15 at 6.9 km; 3 small ones (1-3 m3/s) |

| Cost | |
|---|---|
| wall | 215 s (front 42 s, start surface 5 s, fill 25 s, route 78 s, accumulation 4 s, rebuild test 58 s) |
| peak commit | 5.76 GB (budget 12 GB) |
| partition | whole-region flood and routing (27,636 pits, the widest 1,650 cells): not tile-bounded |

| Store or rebuild | |
|---|---|
| stored L geometry | about 0.9 MB for the commit's tiles (5 bytes a channel cell) |
| rebuild from `B` alone | 58 s for the window, but only 8.0 % of the channel cells agree with the routing on `B + regional` (14,767 of 184,118): `B` alone gives another network |

| GEN-188 (lead 2026-11-14) | |
|---|---|
| L cell pairs across a regional cell-centre line whose second difference exceeds 0.1 m (not a clean step count: it also counts curvature, see steps 1 and 2) | 6,843,572 of 16,640,280 (41 %) |
| channel steps running straight along such a line | 17.8 % with `B + regional`, 11.9 % with `B` alone |

## Verdict
Routing at L does not meet the bar by itself: with every pit breached 10.6 % of the river cells are cut over 50 m, because the start surface's L pits lie on the routed paths too. With lakes kept (D = 20 m) the cut bar holds trivially, but 54 % of the river cells lie in lakes. The start surface `B + regional` is not a drainable terrain at L; the fix sits before the rivers (a region-wide L conditioning of the surface: the carve that K7 runs per tile, run once over the region), then routing at L. At the edge 36 of 44 crossings agree within 2 km; the largest inflow (158 m3/s) enters elsewhere. A reader cannot rebuild the network from `B` alone (8 % agreement): the L geometry is stored (0.9 MB) or the regional change goes into the commit. Rivers follow the 814 m lines more often than on `B` alone (17.8 % against 11.9 %); the 41 % count is not a step count (corrected in steps 1 and 2).

## Visuals
NOT RUN: the before/after stills need a bake whose K7 burns the L-routed lines, which this spike does not build (it measures the network only). The numbers above decide whether that bake is worth making.

## Steps 1 and 2 (lead 2026-11-14)
| R3a, inside the commit's tiles | clamped regional read | GEN-188 limiter | limiter + region-wide K7 carve before routing |
|---|---|---|---|
| pits of the routed surface | 27,636 | 30,555 | 338 |
| channel cells in a pit | 134,564 of 184,118 (73 %) | 145,716 of 193,281 (75 %) | 24,974 of 195,376 (13 %, pits of the start surface) |
| D = inf: cut over 50 m under the start surface | 10.55 % | 10.92 % | 16.38 % (p50 11 m, max 938 m) |
| D = 20 m: cut over 50 m | 0.00 % | 0.00 % | 14.90 % |
| D = 20 m: channel cells in lakes | 99,820 (54 %) | 110,237 (57 %) | 5,692 (2.9 %) |
| D = 20 m: ground / water rises over 0.25 m | 43,893 / 268 | 47,250 / 261 | 800 / 0 |
| channel steps straight along a regional cell-centre line (B alone: 11.9 %) | 17.8 % | 18.8 % | 23.7 % |
| edge: H1 crossings matched within 2 km | 36 of 44 | 38 of 44 | 38 of 44 |
| wall / peak commit | 185 s / - | 233 s / - | 619 s (carve 250 s, fill 291 s) / 6.26 GB |
- The step count of the regional read is not a clean step detector: it counts second differences over 0.1 m and rose with the step-free limiter (6.84 M to 8.99 M), so it also counts the read's curvature. The earlier "41 % step" overstated the clamp.
- The limiter does not make the start surface drainable (pits and cuts unchanged within 1-2 %): the clamp is not the cause.
- The region-wide carve drains the surface (338 pits), but its breach cuts the canyons itself: 16 % of the river cells lie over 50 m under the start surface, deeper than without it.

## Verdict after steps 1 and 2
None of the variants meets the 1 % bar without half the rivers in lakes. The start surface `B + regional` holds L-scale closed basins on most river paths (73-75 % of channel length); a river through them is either cut to their bottoms (breach: 10-16 % of river cells over 50 m) or held as a lake (D = 20 m: 54-57 % of river cells). The clamp of the regional read is not the cause; the relief of `B` at L is. Routing at L does not remove that choice; it only puts the river where the water goes. The decision (canyons, lakes, or a base whose L relief drains) is the ADR's.
