# SPIKE-115: erosion at L inside a region, and the cost of re-routing after an edit (R3a)

Opened: 2026-11-14
Question: do a few erosion rounds at L (101.8 m) over the region make the start surface drain, so that rivers routed at L meet 1 % of river cells cut over 50 m without lakes taking the rivers, while keeping the relief; and what does re-routing the dirty catchments after an edit cost against a full L route?
Why it matters: SPIKE-114 found closed basins on 73-75 % of the river length of R3a at L: the 101.8 m surface is never eroded (B's lattice relief, the regional change, single burned lines). The owner (2026-11-14, "double down on rivers") asks to fix it at the earlier, more global stage and to answer re-routing after edits.
Time box: 1 day of agent work.
Success criteria: per N (1, 3, 10 rounds) and D (20 m, infinity): line cells cut over 50 m at most 1 %, lakes' share named, no water rise, 36 or more of 44 H1 edge crossings matched within 2 km; the relief at 200 m and 1 km kept (named against the start surface); wall time and peak commit within 12 GB; partition independence stated; the incremental re-route of a ridge edit timed against a full L route, with its receivers equal to the full re-route.
Fallback if it fails: the owner's choice from SPIKE-114 (canyons, lakes, or a base whose L relief drains).
Code location: spike branch `spike/erosion-at-l` (crusader), example `crates/cache/examples/erosion_at_l.rs`; never merged.

## Method
- `erosion_at_l` (cache example, spike branch). R3a's front (seed 14, face 1, margin 200); K7's start surface `B + regional` at every L cell of the region window (8,160 x 8,160 = 66.6 M cells).
- **Rounds at L:** the regional solve's kernel (`solve_rounds`: stream power with the breach and deposition, 5 carve steps a round, the world's precision) on the L window; N = 1, 3, 10 cumulative.
- **Multi-scale (Schott et al. 2024, from the literature review):** 3 rounds at 407 m, then 3 at 204 m, then 3 at 101.8 m, each scale seeded with the coarser change upsampled bilinearly; then the multi-scale breach `T - blur_r(T - breach(T))` for r = 8, 4, 2, 1 cells.
- After each: D8 routing at L (plain flood rules), channels at 75 km2, SPIKE-114's network levels with D = infinity and 20 m.
- **Cut:** the canyon is the network level under the eroded surface (the burn); the cut under the start surface is reported too, but after erosion it mostly measures the valleys the rounds carved.
- **Relief kept:** the p50 of the height range in 2 x 2 (200 m) and 10 x 10 (1 km) windows on land, against the start surface.
- **Edit:** a ridge +60 m, 16 km E-W, 1.2 km wide, south of lake 30. The full L route is timed against (a) a re-route of the dirty catchments' box and (b) Barnes 2016's local part (each touched tile flooded alone).

## Numbers (R3a, inside the commit's 52 tiles)
| | start (SPIKE-114) | N = 1 | N = 3 | N = 10 | multi-scale |
|---|---|---|---|---|---|
| river length in a closed basin (bar 10 %) | 73-75 % | 8.6 % | 12.4 % | 20.9 % | 24.7 % |
| pits of the routed surface | 27,636 | 975 | 280 | 383 | 4,668 |
| D = inf: canyon over 50 m (bar 1 %) | 10.6 % | 0.72 % | 0.59 % | 0.37 % | 1.25 % |
| D = 20 m: canyon over 50 m | 0 % | 0 % | 0 % | 0 % | 0 % |
| D = 20 m: river cells in lakes | 54 % | 3.2 % | 3.3 % | 3.6 % | 18.7 % |
| D = 20 m: ground / water rises | 43,893 / 268 | 1,067 / 0 | 900 / 0 | 653 / 0 | 171 / 0 |
| relief 200 m / 1 km against the start | 100 / 100 % | 113 / 111 % | 156 / 143 % | 521 / 358 % | 333 / 297 % |
| H1 edge crossings matched within 2 km | 36 of 44 | 36 of 44 | 36 of 44 | 38 of 44 | 33 of 44 |
| rounds wall (8 threads) | - | 73 s | +156 s | +587 s | 10 + 49 + 253 s, breach 328 s |
| peak commit | 5.8 GB | 6.2 GB (whole run) | | | 6.3 GB |

| Edit (ridge +60 m) | rounds run | multi-scale run |
|---|---|---|
| full L route (66.6 M cells) | 87 s | 78 s |
| dirty catchments | 7 (15,865 cells) | 8 (17,398 cells) |
| box re-route: receivers equal / different to the full re-route | 9,721 / 6,144 (not exact) | 9,384 / 8,014 (not exact) |
| Barnes 2016 local part (touched tiles flooded alone) | - | 1 tile of 768 cells in 0.11 s, plus the perimeter graph |

## Two more regions, N = 1 (lead 2026-11-14)
Windows in the spikes lane's notation (face, i0-i1, j0-j1, seed; the r3a stores' window is face 1, 8056-8675, 760-1379, seed 14, which `spikes/band-drain` calls R1h). Margin 100 (not 200), to stay within the 12 GB budget.
| N = 1 | face 1 seed 14 (r3a stores) | face 4 seed 15 (168-1187, 4520-5539) | face 2 seed 15 (9768-10787, 8104-9123) |
|---|---|---|---|
| start relief p50 at 200 m / 1 km | 7.4 / 67.6 m | 1.26 / 11.6 m | 1.10 / 9.9 m |
| river length in a closed basin (bar 10 %) | 8.6 % | 29.1 % | 13.7 % |
| D = inf: canyon over 50 m (bar 1 %) | 0.72 % | 0.05 % | 0.00 % |
| D = 20 m: river cells in lakes | 3.2 % | 0.8 % | 0.0 % |
| relief kept 200 m / 1 km | 113 / 111 % | 121 / 117 % | 112 / 111 % |
| H1 edge crossings matched within 2 km | 36 of 44 | 71 of 122 | 135 of 200 |
| rounds wall / peak commit | 73 s / 6.2 GB | 119 s / 6.7 GB | 125 s / 6.9 GB |
- The two flat windows (relief about 1 m at 200 m) miss the basin bar: the metric counts every pit over 5 cm, and on a flat land surface one round leaves many shallow pits on the river paths. The canyons and the lakes are within the bars there.
- Edge agreement is lower on the flat windows (58 % and 68 % of H1 crossings matched within 2 km): rivers on a flat surface choose other crossings than H1's 814 m routes.

## Partition independence
The rounds and the routing ran on the whole window. The rounds' catchments span the region: the largest is 5.07 M cells, against 0.59 M in a tile. So the rounds are not tile-bounded; they are a region stage like the 814 m rounds. Barnes 2016 gives the exact flood over any tiling (literature review).

## Verdict
One round of the regional kernel at L over the region meets the bars on the r3a window (face 1, seed 14); on the two flat windows (face 4 and face 2, seed 15) it meets the canyon, lake and relief bars but not the basin bar (29.1 % and 13.7 % of the river length in shallow closed basins) and agrees less with H1 at the edge (58 % and 68 %). On the r3a window:
- closed basins on 8.6 % of the river length (bar 10 %);
- 0.72 % of river cells cut over 50 m with every pit breached, 0 % with pits over 20 m as lakes (bar 1 %);
- lakes 3.2 % of the river cells; no water rise;
- relief kept (113 % and 111 %);
- edge agreement unchanged (36 of 44);
- 73 s and 6.2 GB for the region.

More rounds raise the relief past the start's (N = 3: +56 % at 200 m; N = 10: x5), and the closed-basin share rises again (they are deep basins kept). The multi-scale variant with the 814 m kernel's parameters unchanged at each scale over-erodes (relief x3, 24.7 % in basins, 18.7 % lakes). It needs per-scale parameters (Schott 2024 retargets the elevations), which this time box did not tune. Rejected as run, not as a method.

Edits: an exact incremental re-route needs Barnes 2016's scheme. My box re-route differs from the full route on 39-46 % of the dirty cells. Its local part costs 0.11 s for a touched tile against 78-87 s for a full L route, so the edit cost is the touched tiles plus a small graph.

Methods from the review: Schott 2024 tested (as run, over-eroding); Barnes 2016 tested (local part timed, the global spillover graph not built); Lindsay 2016's selective breach/fill is the D threshold (20 m tested); Fill-Spill-Merge, Génevaux 2013 and the GPU methods not tested (time box; FastFlow and Schott 2023 also need a separate proof of bit-exactness).

## The spike bake
- `bake_eroded` (cache example, spike branch) writes `target/stores/r3a-p2e` (crusader worktree, commit 5c9b47bc8f045ebb, 52 tiles, 403 s). It reruns R3a's front, applies one round at L, routes rivers at L on the eroded surface (3,029 segments, 311,734 channel cells, levels = network levels), and runs K7 with the eroded surface as its start and these rivers burnt.
- No channel graph is stored: the stored format holds H1 lines only, so no river water is drawn.
- Finding: K7's canonical flood (GEN-173 grade) refuses a flat of the eroded surface ("too flat for the epsilon", route.rs:236); the spike bake runs K7 with the plain flood. The implementation needs the grade fixed for such flats or the plain flood in K7.

## Edits (lead 2026-11-14)
The exact incremental path is Barnes 2016's tile scheme (the implementation, not this spike): its local part, a touched 768-cell tile flooded alone, measured 0.11 s, against 78-87 s for a full L route of the region. The perimeter graph is not built here.

## Visuals
![Low-sun oblique at the frame-43680 pose (f43680-lowsun), RTX 3080 Ti, production path (capture-stills --commit). Left: before, r3a-e1 as stored (the H1 line burnt as a box trench up the valley side, drawn as water). Right: after, the spike bake r3a-p2e (one round at L, rivers routed at L burnt; no river water drawn, no graph stored).](images/s115-f43680-lowsun-before-after.jpg)
- After: the box trench is gone; the valley floor is deeper and smooth, in shadow at this sun; no long one-cell groove follows the river. A few short bright slivers on the far valley wall may be walls of short one-cell steps (not checked numerically).
