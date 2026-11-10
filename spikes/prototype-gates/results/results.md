Opened: 2026-11-08
Question: Do the prototype's gates (G-ROUND, G-PART, G-CON, conservation, G-DET) hold when measured by a lane that did not build the code, with its own probe and independent reference kernels, and do the structural gates also hold on a second region of hills and plains and on a second seed?
Why it matters: `docs/briefings/terrain-prototype.md` items 12 and 13; the A/B gate needs gates measured by another lane (A3 section 12.1; SPIKE-105 section 17).
Time box: 3 h
Success criteria: agreement or a stated difference with each reported number of `openspec/changes/region-12288/design.md`; the structural gates at 0 on four further windows (two regions, two seeds).
Fallback if it fails: a disagreement is a finding for the planner; no code is fixed here (inbox #103).
Code location: `spike/prototype-gates` (`crates/generators/examples/gate_probe.rs`, commit f1f0e9c8; never merged). Raw output beside this file.

# SPIKE-106: independent measurement of the prototype gates

Status: closed 2026-11-08 (inbox #103). Main at 8b57df69; release build, Windows, 8 threads, BelowNormal.

## Result

**Every gate agrees with explorer's reported numbers, and every gate is 0 on four further windows (two hills-and-plains regions, two seeds). No disagreement with a reported number was found.** Findings that are not disagreements are in section 4.

## 1. Method: what is independent

| Part | Probe | Shared with the code under test |
|---|---|---|
| Window and outlets | own (rows dealt round-robin to the threads; the mask is own; 200 spot cells re-read on a single thread; compared with `Grid::outlets`) | the base (`BakeInputs::base`) and `FaceSpec::halo_direction` |
| Filled surface | a Dijkstra with the **opposite** tie order (highest id first), and a sweep relaxation of the definition `F = max(z, min F(q) + eps)` with no queue, alternating directions (windows up to 1.3e6 cells) | - |
| Receivers, catchment roots, clipped MFD area | rewritten from the documented rules (steepest jittered slope, strictly lower neighbours of the own catchment, `slope^1.1`, donors by level descending then id), over the probe's own order | `jitter`, `Grid::neighbour`, `distance_at`, `cell_area` |
| Cell area, distance | 3,000 cells: Gauss-Legendre (4 x 4) of the gnomonic solid angle in own tangent coordinates; great circle between the cell directions | - |
| `Q`, channel cells, heads, confluences, mouths | own accumulation in descending filled level; own initiation (`A_d >= T`) and downstream propagation; own conservation sums | `hydrology` (compared), `runoff_mm`, `discharge_weight`, `pet_mm`, orography |
| Basins | own components of the cells with `F - z > 0.5 m` | `hydrology` (compared) |
| Code under test | `carve`, `solve_rounds`, `partitioned_route`, `route_rules`, `hydrology` | - |

Carves: 20 steps of 5 (explorer's) and 12 steps of 4 (3 rounds), on 1, 2, 5 and 8 threads (explorer: 1 and 8). Tiles 128 and 256. Windows are the region plus 200 cells (1.04e6 cells); one repeat at 600 cells (3.3e6).

## 2. Re-measurement on the prototype region (face 1, i 8056-8675, j 760-1379 at 12,288 cells, seed 14)

| Gate | Explorer (`region-12288`) | This probe, margin 200 | Margin 600 |
|---|---|---|---|
| Land cells | 724,570 | 724,570 | 2,158,143 (explorer 2,158,143) |
| G-ROUND, 20/5 | 0 cells differ | 0 on 1, 2, 5, 8 threads | 0 |
| G-ROUND, 12/4 (not run by explorer) | - | 0 on 1, 2, 5, 8 threads | 0 |
| G-DET (1 against N threads) | 0 | 0 | 0 |
| G-PART, tile 256 (z0) | 16 tiles, 6 sweeps, 49 floods, 0, 0 | 16 tiles, 6 sweeps, 49 floods; 0 filled, 0 receivers, 0 `A_d` | 64 tiles, 7 sweeps, 156 floods, 0 |
| G-PART, tile 128 (z0) | - | 64 tiles, 7 sweeps, 191 floods; 0, 0, 0 | 225 tiles, 10 sweeps; 0 |
| G-PART on the carved surface, tiles 128 and 256 | - | 7 to 10 sweeps; 0, 0, 0 | 9 and 13 sweeps; 0 |
| Resident route against the probe's own receivers and `A_d` | - | 0 receivers, 0 `A_d` bits; z0, carved, weighted | 0 |
| Flood: Dijkstra with opposite ties, and sweeps, against the canonical flood | - | 0 cells differ (7 sweeps to converge) | 0 (Dijkstra) |
| G-CON | 0, 0, 0 | 0 channel cells end mid-course, 0 heads under `T`; own channel set equals the lib's (48,833 cells, 0 differ) | own 166,551 = lib 166,551 |
| `Q` at confluences | 4.3e-16 | 4.4e-16 | 4.4e-16 |
| `Q` at the outlets against `sum a r` | 8.5e-15 | 8.6e-15 | 3.2e-14 |
| `A_d` at the outlets against `sum a w` | 4.8e-15 | 4.8e-15 | 6.0e-15 |
| Own `Q` against the lib's | - | max relative 1.9e-15 | 2.3e-15 |
| Channel density, heads and segments per 1,000 km^2 | 6.7 %, 18.6, 35.3 | 6.7 %, 18.6, 35.3 | 7.7 %, 21.0, 40.1 (explorer 7.7 %, 21.1, 40.1) |
| Basins (artefact rule 4 cells, 2 m) | 96 found, 18 artefacts | 96 found = 78 kept (72 lakes, 6 endorheic) + 18 artefacts + 0 ramps; own components 96, own pit cells 56,872 = lib | 159 = 114 + 44 + 1 |
| Cell area against the integral; distance against the great circle | F5: 3e-14 face sum; about 3e-8 at 12,288 cells | 1.1e-8; 4.4e-13 | same |

The region's land change printed here (p5 -458 m, p50 -65 m) is over the whole window; explorer's (p50 -45.5 m, p5 -399 m) is over the region only. Different sets, not a difference.

## 3. Item 13: a second region of hills and plains, a second seed

Selection (probe `survey`, coarse stride 32 cells, 19,000 windows a seed): windows of 620 cells, 200 cells inside the face, land 70-92 %, land p95 elevation 200-800 m, nothing above 1,500 m, median precipitation 500-2,500 mm. Land p95 closest to 450 m wins; two per seed. Candidate lists: `survey14.txt`, `survey15.txt`.

| Window | Face, i, j (start, 620 cells) | Seed | Land | z p50 / p95 / max (land) | Character |
|---|---|---|---|---|---|
| R2a | 1, 7536, 8432 | 14 | 84 % | 57 / 515 / 706 m | plains with hills, wet (P p50 1,097 mm) |
| R2b | 3, 6512, 11248 | 14 | 73 % | 287 / 451 / 516 m | upland hills |
| R3a | 4, 368, 4720 | 15 | 70 % | 35 / 684 / 744 m | plains and hills at a coast |
| R3b | 2, 9968, 8304 | 15 | 85 % | 345 / 468 / 535 m | upland hills |

The prototype region on seed 15 is ocean (0 land cells): its fixtures are specific to seed 14, so a second seed needs its own region (`r1-seed15-ocean.txt`; the gates run and agree on a window with no land, which proves nothing).

| Gate | R2a | R2b | R3a | R3b |
|---|---|---|---|---|
| G-ROUND (both carves, 1/2/5/8 threads) | 0 | 0 | 0 | 0 |
| G-DET | 0 | 0 | 0 | 0 |
| G-PART, tiles 128 and 256, z0 and carved | 0, 0, 0 (4 to 8 sweeps) | 0 (4 to 8) | 0 (5 to 11) | 0 (5 to 9) |
| Dijkstra and sweep flood against canonical | 0 | 0 | 0 | 0 |
| Own receivers and `A_d` against the lib's | 0 | 0 | 0 | 0 |
| Channel cells, own against lib; heads, confluences, mouths | 59,452 = 59,452; equal | 53,569 = 53,569; equal | 76,098 = 76,098; equal | 78,279 = 78,279; equal |
| Channels end mid-course; heads under `T` | 0; 0 | 0; 0 | 0; 0 | 0; 0 |
| `Q` at confluences / outlets, `A_d` at outlets | 4.4e-16 / 2.4e-14 / 1.4e-14 | 4.3e-16 / 4.1e-14 / 1.7e-14 | 4.3e-16 / 3.0e-14 / 1.6e-14 | 4.3e-16 / 2.6e-15 / 5.9e-15 |
| Own `Q` against the lib's (max relative) | 2.3e-15 | 3.5e-15 | 3.0e-15 | 1.7e-15 |
| Pit components, own against lib basins + artefacts + ramps | 282 = 152 + 130 | 108 = 73 + 35 | 343 = 195 + 147 + 1 | 91 = 68 + 21 + 2 |
| Unkept pit cells, share of land (G-LAND: at most 1 %) | 0.026 % | 0.008 % | 0.035 % | 0.010 % |
| Channel density; heads, segments a 1,000 km^2 | 6.8 %; 15.7, 29.4 | 7.1 %; 19.9, 37.7 | 10.5 %; 36.8, 69.0 | 8.9 %; 24.4, 46.0 |
| Basins kept (lakes, endorheic) | 152 (149, 3) | 73 (73, 0) | 195 (195, 0) | 68 (68, 0) |
| Land change p5 / p50 / p95 (m) | -42 / -1 / 15 | -103 / 3 / 16 | -84 / -3 / 13 | -112 / 6 / 17 |
| Cell area, distance checks | 4.6e-9, 4.3e-13 | 4.7e-9, 4.3e-13 | 7.0e-9, 1.2e-12 | 7.4e-9, 4.7e-13 |

## 4. Findings (none a disagreement with a reported number)

1. **Adversarial partitioned route (A3 section 6.8).** A spiral corridor (one cell wide, one outlet at its entrance, walls at 500 m) forces the flood path across tile borders: 257 x 257 cells, tile 32: the corridor crosses a border 1,040 times and the partitioned route needs 515 sweeps and 4,915 floods (0.7 s); 401 x 401, tile 48: 835 sweeps; tile 16: 5,044 crossings, 2,500 sweeps, 55,291 floods (5.1 s). **The result is bit-equal to the resident route and to the Dijkstra reference in all three** (filled, receivers, `A_d`). The sweeps grow with the border crossings (about half), as section 6.8's bound allows; real terrain needs 4 to 13. Consequence: correctness holds on the worst case; the cost of a winding drainage path through small tiles is linear in the crossings, which G-INC and G-SCALE should include (a lake-rich or channelised fixture). A first version of the fixture (rings not joined) was a weak test and was fixed.
2. **The fixed point is order-independent in practice.** Two further algorithms (opposite tie order; queue-free sweeps) give the canonical flood's bits on all windows, which is the claim of A3 section 6.8 checked from outside the kernel.
3. **G-PART with runoff weights is not testable through the API.** `partitioned_route` takes no weights (A3 SPIKE-105 section 19 N3). The probe confirms the weighted resident route equals the probe's own weighted definition (0 `A_d` bits), so the missing piece is the partitioned route, not the weighted kernel.
4. **The class check of the basin balance is not exercised.** Kept basins are lakes except where PET exceeds precipitation: 0 endorheic basins in R2b, R3a and R3b, 3 in R2a, 6 in the prototype region. With the caller's `E_net = max(PET - P, 0)` at 0 over humid land, every basin overflows, so SPIKE-105 section 19 N1 (inflow reads the potential `Q`) and N2 (rain counted twice) cannot show in these windows. They need an arid fixture (the synthetic closed basin of the prototype-region fixtures), as the prototype region's results already note.
5. **Channel density varies with the region.** 6.7-10.5 % of land cells, 15.7-36.8 heads a 1,000 km^2 (R3a twice the prototype region). Explorer's "7 % at the erosion grid" is one region's value; `T` (75 km^2 of effective area) will need calibration against a reference, not against this region.
6. **Cube edges and the whole landmass are not covered.** All windows are single-face planes with an artificial border (outlets); GEN-144's cube grid at 12,288 cells does not fit in memory (region-12288 design), so this probe does not test cross-face routing at the erosion grid.
7. **Unkept pits** (kept basins aside) are 0.004-0.035 % of land, well under G-LAND's 1 %.

## 5. What this does not show

- Only the round solver's gates were re-measured; G-EQ, G-INC, G-BASIN (E1-E7), G-SURF, G-STORE and G-SCALE do not exist yet or belong to other items.
- The independent kernels were written from the documented rules and `route.rs`'s doc comments; they share `jitter` and the grid's neighbour and metric functions with the code under test. The metric was checked separately (area and distance sections above); the jitter hash was not.
- Four windows of 1.04e6 cells on two seeds are a sample, not a proof; one window repeats at 3.3e6.

## Visuals

Nothing to look at: the question is numeric (bit equality and conservation); no images.

## 6. Item 13 re-run on main 563c0e60 (2026-11-10)

The re-run of the structural gates of section 3 on R3a and R3b with the library's example `region_scale` (not the probe), stage probe ON, on the code of main 563c0e60 (the runs were made from the branch commit 29ea4426, which main holds; the relief line printed below was added after the runs and read with a second `mem` run of the same windows); section 3's rows above are unchanged. Release build, Windows, 8 threads, BelowNormal, margin 200 (1,020 x 1,020 cells, 1.04e6), tile 256. Raw output: `item13-r3a.txt`, `item13-r3b.txt`; the probe's numbers: `item13-r3a-stages.txt`, `item13-r3b-stages.txt`. Sheets (not committed): `target/review/stages/region-s15-f4-i368-j4720/` and `region-s15-f2-i9968-j8304/` in the crusader worktree (`C:\DEV\Serious-Planet-Builder\.claude\worktrees\sonnet-crusader-c1a4f2`): `round-00..03` `z` and `area`, `h1` `area`, `z`, `filled`.

| Gate | R3a (seed 15, face 4) | R3b (seed 15, face 2) | Section 3 (R3a / R3b) |
|---|---|---|---|
| G-ROUND (round solver against the whole-region carve) | 0 cells | 0 cells | 0 / 0 |
| G-DET (1 against 8 threads) | 0 cells | 0 cells | 0 / 0 |
| G-PART, tile 256 (receivers; `A_d` bits) | 0; 0 (5 sweeps, 43 floods) | 0; 0 (5 sweeps, 49 floods) | 0 (5 to 11) / 0 (5 to 9) |
| G-CON (disconnected, broken channel cells, miscovered) | 0, 0, 0 | 0, 0, 0 | 0 |
| `Q` at confluences / outlets; `A_d` at outlets | 4.4e-16 / 3.0e-14; 1.6e-14 | 4.4e-16 / 2.8e-15; 5.9e-15 | 4.3e-16 / 3.0e-14 / 1.6e-14; 4.3e-16 / 2.6e-15 / 5.9e-15 |
| Channel cells, heads, segments, mouths | 76,098; 13,794; 25,835; 491 | 78,279; 12,196; 23,031; 184 | channel cells 76,098 / 78,279 |
| Channel density; heads, segments a 1,000 km^2 | 10.5 %; 36.85, 69.01 | 8.9 %; 24.35, 45.99 | 10.5 %; 36.8, 69.0 / 8.9 %; 24.4, 46.0 |
| Basins kept; artefacts; ramps | 195 lakes, 0 endorheic; 147; 1 | 68 lakes, 0 endorheic; 21; 2 | 195, 147, 1 / 68, 21, 2 |
| Pits (land cells with no receiver that are not outlets) | 0 | 0 | not measured |

Every number of section 3 is reproduced (same hydrology, same counts); G-SED, G-BASIN, G-LAND and H1's other gates are not in `region_scale` (it runs the round solver and H1 only), so they were not run.

Relief and precipitation of the land of the region proper (620 x 620 cells) from the current base, next to section 3 (whose z is the land of the whole 1.04e6-cell window with its margin: the probe's round-00 `z` max is 747 m for R3a, so the 684 m p95 of section 3 belongs to that larger set):

| Window | Land cells | z p5 / p50 / p95 / max (m) | Precipitation p5 / p50 / p95 (mm) | Section 3 z p50 / p95 / max |
|---|---|---|---|---|
| R3a | 321,971 | 1 / 25 / 478 / 687 | 853 / 946 / 1,316 | 35 / 684 / 744 |
| R3b | 343,014 | 9 / 282 / 447 / 494 | 733 / 1,387 / 3,313 | 345 / 468 / 535 |

Time a phase (seconds, probe ON, so the probe's cost is inside them; `probe_ms` of each field is in the stages files):

| Phase | R3a | R3b |
|---|---|---|
| Base sampled (1.04e6 cells, 8 threads) | 14.7 | 14.8 |
| Round solver, 4 rounds, 1 thread / 8 threads | 5.8 / 5.2 | 6.8 / 6.2 |
| Whole-region carve (same rules) | 6.3 | 7.7 |
| G-PART, tile 256 (resident route) | 3.6 (1.2) | 4.1 (1.4) |
| Weighted route; canonical flood | 1.2; 0.2 | 1.5; 0.2 |
| H1 | 0.3 | 0.2 |
| Total | 39.2 | 43.5 |

The canonical flood is 12 % (R3a) and 9 % (R3b) of the route-and-flood step, and 0.5 % of the run.

Plains block. A false flat is a land cell of the region with a receiver whose drop on the canonical filled surface to the receiver is exactly 0 or below one f32 step at that height (the surface a tile stores in f32 cannot hold such a drop; a negative drop would count too). A sink is a land cell the fill raised (filled above the carved surface); a pit has no receiver and is not an outlet.

| | R3a | R3b |
|---|---|---|
| False flats | 0 (0 at a drop of exactly 0, 0 below one f32 step) | 0 (0, 0) |
| Sinks (share of region land; mean fill) | 30,856 (9.58 %; 0.39 m) | 4,732 (1.38 %; 0.19 m) |
| Pits | 0 | 0 |

The canonical flood's epsilon is 1e-3 m, which is above one f32 step below about 8 km of height, so the filled surface has no flat by construction at these heights; the fill raises land cells by a mean of 0.2 to 0.4 m (the plains' depressions). The sinks share is not section 3's "unkept pit cells" (0.035 % and 0.010 %: only pits deeper than a limit that H1 does not keep as basins).

Verdict: no gate fails; the verdicts are the architect's.
