Opened: 2026-10-23
Question: Can the open items of A-net (SPIKE-33) be closed: a breach rule with no uphill channel cells and no straight groove, the smallest exact halo of a chunk below 5 km, drainage that is not aligned to the grid axes, and bit-exact determinism across threads?
Why it matters: ADR 0077 (chunks of 5 km along a frozen river network) lists these four items as the first tasks of M8 (opus-planner, owner's go 2026-10-23).
Time box: about 3 h of agent work, CPU prototype only (lid closed), no `cargo xtask perf`, runs kept small.
Success criteria: uphill channel cells 0 % with a hillshade pair; the seam step and channel continuity of two chunks at halos of 0.5, 1, 2 and 5 km (N = 30) and the smallest exact halo; the axis share, look and cost of D-infinity, MFD or a jittered receiver; a bit-exact hash at 1, 4 and 16 threads across chunks.
Fallback if it fails: the list of what blocks each item.
Code location: `spikes/landscape-evolution-2/proto/` (throwaway; `landscape-evolution-2-prototype.patch` in this folder, not merged); builds on `spikes/landscape-evolution/`.

# SPIKE-35 results: A-net open items (landscape-evolution-2)

Machine: Rasierklinge, release build of the CPU prototype, **laptop lid closed, no display; other sessions running** (all timings indicative, single thread unless stated). Site, model and parameters as in SPIKE-33 (`spikes/landscape-evolution/results/results.md`): coarse 2048 grid (cached after one 5-minute run), lat 22.536 lon -8.943, 20 m cells over 20 km for items 1, 2, 4 and 4 m cells over 7 km for item 3, N = 30 steps, chunks of 5 km (2.5 km in item 4). "Chunk" is the term of `docs/domain.md` (SPIKE-33 said block).

## Verdict
- **(1) Breach: 0 % uphill channel cells and no visible groove are reached together by three changes** (variant c4): channel cells take no diffusion and no slumping (the implicit step keeps a bed above its receiver, the hillslope processes broke it), a cell whose receiver is a higher ring cell becomes an outlet (no cascade up the channel), and inside the corridor a cell drains to the lowest forward neighbour instead of the best aligned one, on a meandered path. Uphill falls from 15.5 % to 0.00 %; the breach carves 8,000 instead of 20,000 cells and 10 m on average instead of 46 m; the bed lies 4.0 m (p50) below its surroundings against 41.5 m with the straight blended corridor and 14.7 m for natural channels.
- **(2) The smallest halo with bit-exact cores is 3 km** (N = 30, one site, two neighbouring chunks of 5 km): 0 of 125,000 core cells differ from the whole-lattice run, seam step and channel continuity equal the truth's. At 1.5 and 2 km 19 and 3 cells differ by under 0.5 mm; at 1 km p99 0.60 m and max 23 m; at 0.5 km p99 15 m and max 150 m.
- **(3) A jittered D8 removes most of the axis alignment at no cost**: the share of axis directions among the receivers of cells over 2e4 m2 falls from 0.59 (D8) to 0.55 (jitter 0.4) and 0.52 (jitter 0.8), 34 ns per cell-step for all three; D-infinity (41 ns) does not: 0.64, with 1,757 cycle cells and 512 uphill channel cells because the breach follows the primary receiver only.
- **(4) A-net is bit-exact across threads**: 16 chunks (2.5 km cores, 0.5 km halo) give one hash at 1, 4 and 16 threads and in two runs each.

## 1. Breach rule (`raw/breach.txt`, `images/breach_*.png`)
Whole 20 km lattice, N = 30; a channel cell has area over 50,000 m2; uphill means the receiver is higher by more than 1 micrometre (the implicit step makes a bed converge onto its receiver within rounding, so ties are not uphill; the SPIKE-33 figure of 19.7 % was taken on a different surface and may include such ties, not separated). "Bed below surroundings" is the mean of eight points at 160 m minus the bed, over cells within 30 m of the coarse path.
| Variant | Corridor | Protect channel | Meander | Uphill channel cells | Breach carved (cells, mean, max) | Bed below surroundings p50 / p90 |
|---|---|---|---|---|---|---|
| c0 SPIKE-33 rule, ring outlets | straight, blended to coarse height - 15 m | no | 0 | 7,780 of 50,113 (15.5 %) | 20,213, 46 m, 240 m | 26.8 / 51.7 m |
| c1 | straight, blended | yes | 0 | 0 of 50,113 | 20,213, 46 m, 240 m | 41.5 / 87.9 m |
| c2 | blended, best aligned | yes | 700 m | 0 of 42,136 | 22,803, 46 m, 248 m | 18.4 / 46.9 m |
| c3 | not blended, best aligned | yes | 700 m | 0 of 42,125 | 22,697, 49 m, 250 m | 1.8 / 19.1 m |
| **c4** | not blended, lowest forward neighbour | yes | 700 m | **0 of 33,617** | **7,928, 10 m, 180 m** | **4.0 / 41.8 m** |
| natural channels (over 300 m from the path) | | | | | | 14.7 / 82.7 m |
Cycle cells cut: 8 (c0, c1), 93 (c2, c3), 142 (c4); ring outlets 28 to 61. The meander is a noise displacement of the coarse path (700 m, wavelength 0.9 km, zero at the coarse cell centres), points every 40 m, so every chunk finds the same path.

## 2. Halo of a chunk (`raw/halo-c4.txt`)
Two neighbouring 5 km chunks (cores x 250..500 and 500..750 cells, rows 250..500, ring of fixed heights at the halo border), each against the same options run over the whole 20 km lattice (the truth); N = 30; c4. Channel continuity is measured in a 1 km band round the seam (cells that are a channel in one chunk and under a fifth of that area in the other).
| Halo | Core cells differing in bits (of 125,000) | `|dz|` p99 / max | Seam step minus the truth's, p99 / max | Channel cells disagreeing in the band | Hard join: uphill channel cells within 3 cells | Chunk time (20 m cells) |
|---|---|---|---|---|---|---|
| 0.5 km | 35,073 | 15.1 / 150 m | 94 / 98 m | 434 of 1,046 | 1 of 49 | 0.1 s |
| 1 km | 15,326 | 0.60 / 22.7 m | 21 / 22 m | 0 of 343 | 1 of 43 | 0.1 s |
| 1.5 km | 19 | under 0.5 mm | under 0.5 mm | 0 of 343 | 1 of 43 | 0.1 s |
| 2 km | 3 | under 0.5 mm | under 0.5 mm | 0 of 343 | 1 of 43 | 0.2 s |
| **3 km** | **0** | 0 | 0 | 0 of 343 | 1 of 43 | 0.3 s |
| 5 km | 0 | 0 | 0 | 0 of 343 | 1 of 43 | 0.5 s |
The remaining difference is the dependence of a channel bed on its downstream cells: the ring heights are the initial heights, the truth's cells at the same place have evolved, and the error decays along the channel within about 1.5 km at this site. The halo that is exact depends on the channels that cross the chunk border and on N; one site and one seam were measured. A chunk's halo cost grows with its area: a 5 km core with a 3 km halo is 11 km on a side, 4.8 times the core's cells.

## 3. Receivers for the hillslopes (`raw/routes.txt`, `images/routes_*.png`)
4 m cells, 49 km2, N = 30, variant c4 with the receiver rule changed (the corridor rule is the same).
| Rule | Time (49 km2) | ns per cell-step | Axis share, receivers of cells over 2e4 m2 (n) | Over 2e5 m2 (n) | Uphill channel cells | Cycle cells |
|---|---|---|---|---|---|---|
| D8 | 3.11 s | 34 | 0.591 (31,364) | 0.666 (3,532) | 0 of 14,117 | 25 |
| D8, jitter 0.4 | 3.11 s | 34 | 0.552 (32,880) | 0.572 (3,568) | 0 of 15,466 | 34 |
| D8, jitter 0.8 | 3.10 s | 34 | 0.517 (35,869) | 0.527 (3,732) | 0 of 16,425 | 31 |
| D-infinity (area split over two receivers, incision along the larger) | 3.77 s | 41 | 0.638 (26,357) | 0.768 (2,047) | 512 of 12,280 | 1,757 |
Jitter: the slope of each of the eight directions is scaled by `1 + j (2 h - 1)`, `h` a hash of the lattice cell and the direction (`mix64`), a function of position only, so every chunk makes the same choice; 0.5 is no preferred direction. The look (`routes_sunset.png`, left to right D8, jitter 0.4, D-infinity): D8 shows the parallel diagonal hatching; jitter 0.4 breaks it into curved, branching rills with the same network; D-infinity gives fewer, wider rills and flat areas (it spreads the area but the incision still follows one receiver). **Recommendation: jitter 0.4 to 0.8.** MFD for the incision was not built (the implicit step needs one receiver).

## 4. Determinism (`raw/det.txt`)
16 chunks of 2.5 km cores with 0.5 km halos on the 20 km lattice (20 m cells), N = 30, variant c4; threads across chunks; hash of the cores' heights and areas.
| Threads | Run 0 | Run 1 | Time |
|---|---|---|---|
| 1 | 4b5bc84fb64df93d | 4b5bc84fb64df93d | 0.3 s |
| 4 | 4b5bc84fb64df93d | 4b5bc84fb64df93d | 0.1 s |
| 16 | 4b5bc84fb64df93d | 4b5bc84fb64df93d | 0.1 s |
`libm` only (pow, exp, sin, cos, atan2, floor), no unordered containers (`BTreeSet`, vectors), Kahn's queue in cell-index order, accumulation along it; a chunk runs sequentially, threads are across chunks, so no reduction depends on the thread count. The coarse run is as in SPIKE-33 (bit-exact at 1, 4 and 16 threads).

## Open items and what was not done
- The halo was measured at one site, one seam and N = 30; the exact halo (3 km) and the 1 mm halo (1.5 km) should be rechecked at a site with a large river crossing the chunk border and at the step cap chosen for M8.
- The straight groove is gone in the hillshade, but the bed is 4 m (p50) and 42 m (p90) below its surroundings; 142 cells per 20 km lattice are cut from receiver cycles and become outlets.
- No MFD incision; D-infinity breaks the breach (second receiver not carved). Jitter amplitude 0.4 to 0.8 was not tuned against the look beyond the pair of hillshades.
- No engine render: hillshades only (CPU, lid closed). Threads inside one chunk (level-parallel solve) were not tried.

## Visuals
- `images/breach_sunset.png`: hillshade of the central 5 km at 20 m (sun azimuth 262, elevation 25 degrees), left to right: c0 (SPIKE-33 rule: straight canal with a flat floor), c3 (meandered, not blended: still a canal), c4 (lowest forward neighbour: no canal). `images/breach_midday.png`: the same at 55 degrees.
- `images/routes_sunset.png`: hillshade of the central 5 km at 4 m (azimuth 262, elevation 6 degrees), left to right: D8, jitter 0.4, D-infinity. `images/routes_midday.png`: the same at 55 degrees.
