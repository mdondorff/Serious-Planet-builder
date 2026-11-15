# SPIKE-123: basins and spill relations as first-class objects (P3 E5, D-4)

Opened: 2026-11-15 (lead #145)
Question: can basins, their hierarchy, spill relations, classes and water levels be computed on any surface, deterministically, and what do they show on the A stores?
Why it matters: D-4 (lakes and basins) and its gate G-LAKE; E4's design note §5 needs basin id, spill point, active outlet, class, level and `q_out` from D-4.
Time box: 1 day of agent work.
Success criteria: the objects computed on the A stores of all five windows, counts per class, and a G-LAKE fixture designed for the whole chain.
Fallback if it fails: the flat basins of H1 (`Hydro::basins`, GEN-145) stay the only model.
Code location: branch spike/lakes-d4 (local, never merged): `crates/cache/examples/lakes_d4_fsm.rs.inc`, `river_gate --lakes`, `lakes_selftest`, `lcell_latlon`.

## Method
- **Sources:** Barnes, Callaghan and Wickert, "Computing water flow through complex landscapes", Part 2 (depression hierarchies, 2020) and Part 3 (Fill-Spill-Merge, 2021). The steady form is used here. The papers were not downloaded; the algorithm follows their published structure.
- **Hierarchy:** one sweep over the cells in increasing (height, cell id) order, with a union-find of components.
  - A cell with no processed neighbour starts a pit (a leaf).
  - A cell that joins two depressions is their saddle: it creates their parent at its height.
  - A depression that meets an outlet, or land already drained, is linked. Its spill point is that cell; its spill target is the node, or the outlets, on the other side.
  - Every cell is put in the node it floods in.
- **Water:** runoff on the steepest-descent watershed of each pit (the raw surface) feeds the leaves. The climate is H1's: runoff and open-water evaporation `E_open = PET - (P - r)` per regional cell.
- **Steady Fill-Spill-Merge:**
  - a body fills until `E_open` over its area equals its inflow;
  - a full body overflows to its spill target;
  - a full child fills its sibling;
  - two submerged children merge into their parent.
- **Classes**, applied in this order (min depth 0.5 m and min size 16 cells, the gate's basin metric):

  | Class | Rule |
  |---|---|
  | artefact to drain | floor to spill under 0.5 m, or under 16 cells at the spill, whatever water it holds |
  | kept lake | full: its spill point is the active outlet |
  | dry | no water reaches it |
  | endorheic | the rest: closed, level below the spill |

- **Outputs per body:** basin node (`floor_cell` names it, as `basin_id` does), spill cell, spill target, class, water level, `q_in`, `q_out`. These are the fields E4 §5 names.
- **Determinism:** single-threaded, with a fixed visiting order and sums in cell order (CON-14), in f64 arithmetic only. Two runs on the two R1h A stores (crusader's `r1h-round-k8` and explorer's `a-r1h`) gave identical output.
- **Store mode:** the cells outside the commit's tiles drain. They stand below the sea, as in the gate's routing; otherwise the higher start surface around the tiles dams them.
- **Fixtures** (`lakes_selftest`, two pits behind a saddle behind a rim with a gap):

  | Inflow | Result |
  |---|---|
  | none | both pits dry |
  | small | both endorheic |
  | 8 | the left pit fills and passes 0.57 m3/s into the right, which stays endorheic |
  | 20 | both submerge, the parent spills at 6 m with 8.57 m3/s out |

## Counts per class and window
Bodies whose floor lies inside the region's box (620 regional cells); water areas in km2.

| Window | Surface | Nodes (pits / merges) | Kept lakes | Endorheic | Dry | Artefacts | Artefact depth p50 / p90 | Time | Peak |
|---|---|---|---|---|---|---|---|---|---|
| R1h | today's start surface | 87,113 (58,176 / 28,937) | 5,887 (11,911) | 80 (338) | 0 | 5,539 (448) | 0.46 / 3.73 m | 263 s | 6.5 GB |
| R1h | A (explorer `a-r1h`) | 32,423 (29,327 / 3,096) | 35 (11.8) | 0 | 0 | 15,513 (275) | 0.05 / 0.14 m | 27 s | 5.5 GB |
| R3a | A | 39,228 (36,943 / 2,285) | 2 (0.9) | 0 | 0 | 27,926 (747) | 0.04 / 0.11 m | 35 s | 6.5 GB |
| R3b | A | 42,126 (38,812 / 3,314) | 40 (24.8) | 0 | 0 | 25,876 (479) | 0.05 / 0.14 m | 26 s | 6.5 GB |
| M1 | A | 107,360 (82,387 / 24,973) | 1,056 (665) | 0 | 0 | 39,480 (874) | 0.07 / 0.19 m | 31 s | 5.5 GB |
| F1 | A | 66,183 (60,146 / 6,037) | 25 (9.6) | 0 | 0 | 47,279 (896) | 0.05 / 0.14 m | 28 s | 5.5 GB |

- Time is the analysis after the surface is built: the hierarchy takes 14-23 s; peak is the whole gate run. The R1h start-surface row is slower (263 s) because it has 58,176 pits and its steady solve runs over many more merged depressions.
- Kept-lake depths: R1h today p50 10.0 m (up to 287 m); M1 A p50 13.2 m (up to 194 m); the other A stores p50 0.7-1.1 m.

## Findings
1. **The round at L drains the lakes.** On R1h today's surface holds 5,887 kept lakes (11,911 km2, about 4.7 % of the region); the A store keeps 35 (11.8 km2). The round's breach drains every closed basin (SPIKE-120 item 2), real lakes included. Only the mountain window M1 keeps lakes (1,056, 665 km2, up to 194 m deep).
2. **Artefacts outnumber lakes everywhere** (15,513-47,279 per A store). They are a few centimetres deep (p50 0.04-0.07 m), at the scale of the stored change's 0.25 m steps. The classifier drains them by its rule.
3. **No endorheic basin on any window:** with this climate's `E_open`, every body that receives water overflows. An endorheic test needs an arid window, or a fixture with a set `E_open`.
4. **Spill relations:** 3,400-11,000 depression trees per A store spill into another depression rather than the outlets; the spill graph carries their overflow there.
5. **For D-4:** basins have to be identified before the D-1 stage, and a kept lake has to be protected through it, for example as an intentional basin (GEN-146) whose cells the breach may not lower. Otherwise the chain keeps no inland lake.

## G-LAKE fixture (designed now; run when E3 has a winner)
**The lake:** R1h, face 1, seed 14, L cell (65823, 7542), at -37.1001, -164.7368 (degrees).

| Property (today's start surface) | Value |
|---|---|
| Floor | 438.43 m |
| Spill | 535.90 m (depth 97.5 m) |
| Area at the spill | 12,548 L cells (130.0 km2) |
| Inflow / outflow | 3.61 / 3.20 m3/s, to the outlets |
| Class | kept lake (it overflows: a through-flow lake) |

On the A store it is gone.

**Mark:** the lake's cells at today's spill level as an intentional basin (GEN-146, A3 section 5.3), before the D-1 stage.

**Checks along the chain:**

| Stage | Check |
|---|---|
| Base (B + regional) | the basin exists: floor within 1 m, spill within 1 m, area within 5 % of the table |
| Rounds / D-1 stage | no lake cell lowered below its start by more than 1 m; the spill cell within 1 m of 535.90 m |
| H1 | a kept basin with this floor; class Lake, `overflows` true, level within 1 m of the spill; the channel continues below the spill (`q_out` > 0) |
| K7 | no channel cell burnt inside the lake (GEN-163); the burnt bed at the lake entry at or under the level; the rim unlowered |
| Store | the lake record (`Lake`: id, floor, spill, level, class, tiles) in the commit |
| Runtime read | the water mask shows the lake at its level over its area (within 5 %), with the river leaving at the spill |
| This spike's classifier on the stored ground | class kept lake, level within 1 m |

**Pass:** every check holds on E3's winning variant. One failing stage names the stage that drained it.

**Flight brief** (#146, filled in after E3):
- commit folder: the winner's store;
- command: `planet editor --commit <DIR> --site -37.1001,-164.7368 --marker <lat,lon>` with one `--marker` per kept lake of the window (from this classifier's kept-lake list, converted by `lcell_latlon`);
- what to judge: the water lies in a closed hollow at a level, not on a slope; the river enters and leaves at the spill; the shore follows the ground, with no straight cut through the rim.
- A variant that changes the base (E) needs its stated route into the viewer (VIEW-069 opens a commit over the in-memory production base). The command is smoked once by crusader before hand-over.

## Visuals
None: counts only. Pictures come with the G-LAKE flight on E3's winner.
