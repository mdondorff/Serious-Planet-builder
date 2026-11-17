# SPIKE-130: basin and lake topology on the river graph (G1e)

Opened: 2026-11-17 (lead #156)
Question: can a basin producer on the river graph's contract types (GEN-189, G0 contracts section 4) give basins whose ids and spill relations are stable under partition and thread count, on the G1e cases and the fixture, and what do the five windows hold on `S0`?
Why it matters: G1e of the river-graph plan; KS, KV and KC must leave protected basins at `S0`, and KG needs the receiving nodes of their spills.
Time box: about 2 days (used: about 1 day).
Success criteria (addendum section 3, G1e): ids and spill relations stable under both partition layouts and 1 and N threads; validators pass; the five windows counted.
Fallback if it fails: name the case that breaks and the contract rule it needs.
Code location: branch `spike/basins-g1e` (local, never merged):
- `crates/generators/src/bake/basins_rg.rs` (the producer);
- `crates/generators/tests/basins_g1e.rs` (the cases and the fixture);
- `crates/cache/examples/basins_g1e.rs` (the windows).

## 1. Producer
| Step | Rule |
|---|---|
| Hierarchy | SPIKE-123's one sweep on any grid. Cells go in (height, id) order through a union-find. A saddle joining two depressions starts their parent. A depression meeting the outlet set is linked there (spill cell, height, target). A tree never linked is closed. |
| Kept | a node from 0.25 km2 (its subtree's true cell area) and 2 m (spill minus floor), `protected_surface`; at H1, GEN-145's rule (2 m at any size) |
| Reported | under the kept bar but from 0.5 m and 16 cells (BUILD-055's basin rule), `represented: none` |
| Parent | the nearest basin ancestor in the hierarchy; for an L image, the H1 basin (section 2.1) |
| Receiving node | The overflow leaves over the spill cell to its lowest neighbour outside the basin (ties by id). It then follows the routed path (round rules) until it meets either another basin's cell (that basin's `lake_inlet`, anchored there) or a given reach's vertex (a `trib_junction` on it). A path that reaches an outlet first is reported unresolved. |
| Inlets | a given reach entering a kept basin gets a `lake_inlet` on its first vertex inside it |
| Endorheic | a closed tree (no outlet in the domain); spill anchor none |
| Id | GEN-189's `basin_id(body, level, floor, spill)` |
| L image | the L basin holding the lowest L cell of `S0` (ties by id) among the L cells of the H1 floor cell |

The ids exposed one contract change. A parent's floor is its deepest child's floor, so `hash(body, level, floor)` gave nested basins one id. DEC crusader-7 resolved it: the lead's reading 4 adds the spill anchor, and #1568 implements it in GEN-189.

## 2. G1e cases (tier A, spike branch)
Cube sphere of 24 cells a face edge, 100 m cells, face 0 land falling 0.5 m a cell to the east, the other faces sea. Synthetic thresholds: kept from 4 cells and 2 m, reported from 2 cells and 0.5 m.

| Case | Holds |
|---|---|
| Nested basins | two pits inside a container: both have it as an ancestor |
| Two depressions sharing a spill | one saddle cell; each spills into the other's `lake_inlet`; siblings |
| Closed endorheic basin | a world without an outlet holds an endorheic basin; with the sea the same bowl spills (`active`) |
| Single-outlet lake, several inlets | three reaches entering give three `lake_inlet`s; one spill |
| Basin spilling into another | the upper basin's spill node is a `lake_inlet` inside the lower one; two trees, not nested |
| Basin across a tile line | it crosses tile columns 0 and 1. Both layouts (one region; two regions of whole tiles, each with a one-tile halo, the domain's edge an outlet) give the same basins as the whole run: ids, parents, spills and receiving nodes. |
| All cases | the graph of basins, nodes and a reach validates (GEN-189), the same bytes on a second run |

## 3. The G0 fixture (TEST-139)
| Measure | Value |
|---|---|
| H1 basins on `z_s` | 1 (the bowl) |
| L basins on `S0` | 4 (3 kept), among them the bowl and a basin in the C1 trough at the antecedent ridge |
| L image of the H1 bowl | linked: `parent` = the H1 basin id, owner landmass; spill 431.845 m as TEST-139 records; floor 360.357 m (TEST-139's floor key is `S0` at the bowl's centre cell) |
| 1 and 4 threads | same bytes |
| Both layouts | all 4 L basins resolved inside a region, equal to the whole run |
| Unresolved spills | 2 (this test gives no reaches; with the trunk graph they reach a `trib_junction`) |

## 4. Five windows on `S0` (the start surface `B + C1(z_s - z_b)`, L, floor in the region's box)
| Window | Kept (nested, outermost) | Reported | Outermost area, km2 (% of land) | Area p50 / p90 / max, km2 | Depth p50 / p90 / max, m | Outermost on an H1 trunk line (area km2) | Closed |
|---|---|---|---|---|---|---|---|
| R1h | 11,388 (7,080; 4,308) | 3,283 | 11,981 (6.8 %) | 0.67 / 3.9 / 634 | 15.1 / 74.8 / 287 | 1,740 (9,263) | 0 |
| R3a | 3,489 (1,867; 1,622) | 4,773 | 6,147 (3.0 %) | 1.32 / 7.0 / 331 | 3.7 / 9.1 / 80 | 1,341 (5,871) | 0 |
| R3b | 10,394 (6,034; 4,360) | 5,107 | 12,678 (5.6 %) | 0.81 / 3.9 / 523 | 6.9 / 31.1 / 107 | 2,913 (11,398) | 0 |
| M1 | 32,846 (28,395; 4,451) | 5,135 | 49,093 (19.3 %) | 0.57 / 3.9 / 2,518 | 73.3 / 282 / 928 | 1,611 (47,037) | 0 |
| F1 | 19,716 (12,312; 7,404) | 13,582 | 22,188 (8.7 %) | 1.00 / 4.9 / 269 | 4.8 / 15.9 / 65 | 3,410 (17,580) | 0 |

- "On an H1 trunk line": the basin's cells hold a cell of an H1 trunk polyline (sampled every L cell). The five largest basins of every window are on trunk lines.
- `S0` is the unconditioned start: these are the depressions KS, KV and KC meet, before any conditioning.
  - On the flat and coastal windows (R3a, F1, R3b) the largest basins are lowlands floored near sea level (0-4 m), 6-67 m deep.
  - On R1h and M1 they are valley stretches behind the band `r`. M1 has 2,518 km2 at 761 m, a mountain window's sub-km relief.
- Cost: 73-155 s a window, 1.8-2.0 GB at most; the hierarchy takes 28-117 s.

## 5. Findings
1. The producer meets G1e's exit on the cases and the fixture:
   - ids and spill relations are the same under both layouts and on 1 and 4 threads;
   - the graph of basins and nodes validates.
   One contract change was needed: the spill anchor in the basin id (reading 4, #1568).
2. Partition: a region with a one-tile halo resolves every basin of the fixture and of the tile case. The windows' basins reach 2,518 km2 (M1). Region-level basins of that size need a landmass pass (H1 basins and their L images), not a tile halo. The contract already puts H1 basins and their images on the landmass.
3. `S0` holds far more closed area than Earth: 3-19 % of land in kept basins, most of it on trunk lines. KS, KV and KC would have to leave every kept basin at `S0` (section 4). As kept today (0.25 km2 and 2 m on `S0`), that freezes 3-19 % of the land, including whole valley stretches on trunk lines. The kept rule should read the conditioned surface, or the L-scale floor needs a decision before G1f.
4. Unresolved spills: without reaches, a spill path that reaches the sea has no receiving node. The contract has no external outlet, so a coastal basin needs a reach (KG-trib) or an `outlet_sea` reading.

## 6. Recommendation
- Keep the producer's structure for G1e's implementation: SPIKE-123's sweep, nearest-basin parents, spill paths from the lowest outside neighbour, and GEN-189's ids.
- Decide before G1f which surface the L kept rule reads. Options: `S0`, or KS0/KS1 after conditioning, or a trunk-line exemption. As it stands on `S0`, the protected set would block KV and KC along most trunks.
- Add an `outlet_sea` receiving reading for basins whose spill reaches the sea without a reach.

## Visuals
None: the results are counts and topology (the cases are asserted in tests).
