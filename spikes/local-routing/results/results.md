Opened: 2026-10-25
Question: Does routing the main stem locally inside a chunk (inside a band round the coarse path, fixed at the chunk borders) remove the climbing links and the deep cuts of the corridor better than keeping the coarse path with a capped running-minimum bed?
Why it matters: the corridor follows the coupled run's 4.9 km network exactly, the base has km-scale detail at 8 m, so the corridor climbs (design.md section 20: 178 of 680 links at the lowland site, 88 of 169 at site 2); ADR 0077 point 3 freezes the network, so the question is how far the fine path may leave it.
Time box: about 4 h (used about 2.5 h); prototype only
Success criteria: per variant: climbing links, cut depth and length, sinuosity at 1 and 4 km, tributaries per km2, same bits on 1 and 8 threads, border agreement, cost per chunk
Fallback if it fails: report what the data could not settle
Code location: `spikes/local-routing/proto/` (the dumper `routespike.rs` and `route_dump.rs` were built inside `crates/app` on the spike branch and are not committed there; `router/` is a standalone crate; `render.py`)
Closed: 2026-10-25

## Data and its limit
- Shared store `C:/DEV/SeriousPlanetData/global16384` read **before the rebuild**: its coupled run is the compressed one (the strongest rivers of the run climb 70 to 300 m). The lowland chunk reproduces explorer's count (681 corridor cells, 680 links). Explorer's site 2 (88 of 169) was not given; the hilly chunks below are the rivers of 1.3e10 to 1.6e10 m2 on the steepest ground of this run (candidate list from `route_dump --candidates`), not site 2.
- Three chunks of 608 x 608 cells of 8.04 m: `low` (-11.166, 23.049), `h1` (40.681, 135.358; the river only crosses a corner, a stem of 0.8 km), `h2` (40.320, 137.590; stem 10.4 km on the coarse path).
- A re-run on the rebuilt store takes about 5 minutes (two dumps of about 1 min each, the router seconds).

## What was built
- B: the coarse path of the corridor from the entry (or the head) of the largest river to the exit, with the bed as the running minimum of the ground from upstream, uncapped, and capped at 10 m and 30 m under the ground (`bed = max(runmin, ground - cap)`: where the cap bites the bed climbs, a dam).
- A: a least-cost path from the same entry to the same exit over the cells within W of the coarse path (W = 0.5, 1, 2 km; octile distance by Dijkstra), step cost = rise (only positive) + lambda x step length, lambda = 0.005, 0.02, 0.1 m per m; then the same running-minimum bed. Deterministic ties by cell index.
- Variants beyond the brief: a gate (the exit may slide along its border edge to the lowest ground within 0.5 or 1 km; it needs a symmetric rule from the halo rows, not built, so its border agreement is unverified) and a free head (a river that begins inside the chunk, with no entry, starts at the highest ground within R of the coarse head).
- Tributaries: not routed per coarse segment (the chunks hold one stem of the corridor that matters; the second coarse river was not routed). The fine tributary measure is a priority flood from the stem (at its bed) and the core border, counting side entries into the stem with at least 1e5 and 1e6 m2; it is the same measure for A and B.

## Results (full tables: `table.md`, raw: `table_*.txt`)
| Measure | B coarse path | A, W 1 to 2 km, lambda 0.02 |
|---|---|---|
| h2, cut under the ground, max / length over 5 m (uncapped running-minimum bed) | 16.4 m / 2389 m | 0.3 to 0.5 m / 0 m |
| h2, bed climbs at a 10 m cap | 41 links, 428 m at the cap | 0 |
| h2, ground rising downstream (links over 5 cm) | 356 of 1225 (sum 52 m) | 7 to 26 of 586 (sum 1 to 2 m) |
| h2, sinuosity at 1 km / 4 km, median | 1.42 / 1.76 | 1.02 to 1.09 / 1.03 to 1.13 |
| low, cut max (the exit lies 10 m above the entry, the head sits in a pit) | 13.1 m | 12.8 m |
| low, cut length over 5 m | 5421 m | 2992 m |
| low, ground rising downstream | 258 of 681 | 109 of 311 |
| low, free head R 1 km: cut max / length over 5 m | n/a | 4.3 m / 0 m |
| h1 (a corner of 0.8 km) | 40 climbs, cut 2.0 m | 26 climbs, cut 1.7 m |
| Cost of the route (Dijkstra in the band and the path) | 0 | 4 to 50 ms (W 0.5 km: 18 ms, 2 km: 40 ms) |
| Today's chunk | about 4 s | the route is under 1 % of it |

- **Climbing links.** On the hilly chunk the coarse path rises downstream at 356 of 1225 links (all of them small, the largest 0.4 m: the noise of the 8 m base); the local route has 7 to 26 of 586. H1's bed on all corridor links (explorer's measure, the lower of anchor and ground): 545 of 1450 and 604 of 1736 climb at the hilly chunks, 0 of 680 at the lowland.
- **Cuts through highs.** On the hilly chunk the coarse path needs a cut of up to 16 m and 2.4 km of it over 5 m (the coarse polyline meanders with a 1 km wavelength across the slope and crosses spurs); the local route needs 0.3 m at W 1 km: the water gap is avoided, not cut. At a 10 m cap the coarse path climbs back at 41 links (up to 0.4 m each, 428 m at the cap); the local route never reaches a cap.
- **The lowland chunk is bound by its end points.** The river starts at the coarse head in a pit (279.3 m) and leaves 10 m higher (289.4 m); every path needs a cut of 12.7 to 13.1 m. Sliding the exit along the border (gate) gains 5.6 m of ground but not the cut (the pit is the head). Starting at the highest ground within 1 km of the head (free head) lowers the cut to 4.3 m with none over 5 m. The coarse network is wrong here relative to the base; routing can only hide it.
- **Sinuosity.** The coarse path has 1.4 to 1.5 (1 km) and 1.8 (4 km); the local route is 1.0 to 1.2 depending on W and lambda: W 0.5 km keeps 1.27 to 1.3 at 4 km, W 2 km with lambda 0.1 is a straight line (1.00). The target of SPIKE-40 for lowland rivers is 1.3 to 2, for mountain rivers 1.05 to 1.3: a least-cost route alone is too straight for a lowland and fits a mountain river; a narrow band (W 0.5 km) or a sinuosity term keeps the guide's bends.
- **Fine tributaries.** 0.25 to 0.46 per km2 at 1e5 m2 and 0 to 0.04 at 1e6 m2 for both; the numbers fall as W grows (2 km: 0.04 to 0) because a straighter stem has fewer side entries. The measure is a property of the stem's position more than of the routing; neither variant creates tributaries (SPIKE-40 finding unchanged: the stage has no network between the hillslope and the corridor).
- **Determinism.** The hash of all variant paths is identical at 1 and 8 worker threads (`dc4b2f3fe31e70f9` at the lowland chunk). The route uses integer costs (1/1000 m), a heap ordered by (cost, cell index) and no shared state, so this is by construction, not a finding.
- **Border agreement.** A and B keep the coarse entry and exit cells, which are functions of position (the polyline and the lattice only). Checked on one pair: the lowland chunk's exit core cell 369610 (face 0, i 941738, j 455391) is the entry cell of the next chunk (`lowdown`, origin j 455392) at core index 554, upstream cell (941738, 455391): the same pair from both sides. The bed at the border is not compared (it is the ground there plus the depth law, unchanged). The gate and the free head are not border-checked: the head is free by definition (no upstream chunk), the gate needs a symmetric rule.

## What the data does not settle
- Explorer's site 2 (steep ground, anchors 266 m over the ground) is not in this store; the rebuilt store will show whether the hilly gain (cut 16 m to 0.3 m) holds on steeper chunks with deeper gorges.
- One stem per chunk: the second coarse river and the junctions (a tributary must meet the new stem path, not the old one) are not routed; at `h2` there are two entries and one exit.
- The band is measured from the coarse polyline of this run; when the run changes (rho 1 rebuild) the route changes with it, and the polyline's own 1 km meander (a function of position) is part of what A removes.
- Timing is Dijkstra on a band in one thread without the base read (about 2 s of the chunk's 4 s) and without the solve.

## Visuals
- `images/h2.png`: hillshade of the hilly chunk with the coarse path (red) and two local routes (cyan, blue), and the ground and running-minimum bed along each. The coarse path wanders and holds a dam 3 km long near 3524 m; the local route descends monotonically.
- `images/low.png`: the lowland chunk; the ground between the head and the exit rises 13 m whichever path is taken.
- `images/h1.png`: the corner chunk.
