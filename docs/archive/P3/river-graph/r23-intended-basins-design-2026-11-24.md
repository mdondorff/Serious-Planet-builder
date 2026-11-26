# R23: intended basins from the tectonic field (design note, lead #203)

Status: draft for the lead, 2026-11-24 (explorer). No build before the lead's reply.
Sources: owner decision 6(a) (open-decisions-v2.md:80); roadmap item 8; G3-8 and R23 (g3-gate-2026-11-23.md:47, :76); A4 and A5 (o8-synthesis-response-2026-11-23.md:15-16); decision 11; contract g0-contracts.md §4, §6, §10, readings 4, 6, 13, 16, 22, 24, 35, 37, 44; ADR 0111 point 5.

## 1. What the tectonic field offers today

| Field | Where | Resolution | Meaning |
|---|---|---|---|
| `Provinces.basin` | lithology.rs:89-99 (`of_source`) | 256 per face edge (about 39 km), u8, bilinear | lowland inland sediment basin: `(1 - smoothstep(200, 700, e0)) * smoothstep(0.3, 0.7, inland)` |
| `Provinces.rift` | same | same | divergent margin within about 200 km (`convergence < -0.15`) |
| `Provinces.foreland` | same | same | the outer belt flank |
| `MaskTerms.back_arc`, `plateau_field` | classes.rs:505-520 | analytic per direction; `ClassLattice` 256 per face | raised plateau behind a convergent margin (Altiplano type): +2,000 m and +600 m on `e0` |
| aridity `P / PET` | arid.rs:69 (`AridInputs`, GEN-117) | per base cell; orography 512 per face edge | the arid rules' input |

- No field lowers the ground: the uplift is never negative (carve.rs:213), and the rift term lifts (`U_RIFT` 28 > `U_FLAT` 24). The field marks where a basin belongs. It does not make one.
- K5, H1 and the river chain read none of these fields: they see tectonics only through the heights.
- Every "endorheic" in code today is found from depressions after the fact (`BasinClass::Endorheic` hydro.rs:66; basins_rg.rs's never-linked tree).
- Proposed mark `I(c)` per L cell: `max(basin, rift, back_arc * plateau_field)` from the province lattice, gated by aridity: `P / PET < 0.2` (the arid window class, runoff under 50 mm a year, build-tooling spec :1190). A depression is intended when its floor cell has `I >= 0.5` and the gate holds. Thresholds are design values; they are frozen before G3-8.

## 2. How basins-2 takes them

- Mechanism: the forced-lake path that exists (`produce_drain_avoiding_forced`, basins_rg.rs:291; crusader #172). An intended depression joins the forced list. For each marked floor, the node kept is the largest hierarchy node whose floor is marked, more than half of whose cells are marked, and whose spill stays below `floor + 10 x budget`.
- Kept: `represented: protected_surface`, whatever reading 22's budget or A5's 10 m cap would do. KS, KV and KC never change its cells (§4).
- Terminal sink: `outlet: endorheic`. The spill cell is recorded as the id anchor (reading 4), with no spill path, no receiving node and no `outlet_sea` (reading 6). Lines entering it get a `lake_inlet` and end there (no `trib_junction` downstream).
- Ranking in one basin pass, first match wins:
  1. intended: kept, endorheic;
  2. A5: a breach cutting more than 10 m: kept, spills;
  3. reading 22: breach and fill both over the remaining budget: kept, spills;
  4. otherwise drained.
- Decision 11 order unchanged: the intended set is fixed in the early reading-22 pass (before lines, band6 and KV), so reading 44's rerun never flips it.
- H1: an intended basin's H1 nodes are kept too; an H1 trunk entering one ends at its inlet (not crossed with `exception(rise_over_budget)`, reading 13).
- Water: the level is deferred to O9 (store contract at G3). Until then an intended basin holds no lake water (`q_w = 0` out; ADR 0111 point 4).
- Open (needs the lead or owner): ADR 0111 point 5 decides endorheic "by water balance, not by aridity". The aridity gate above is a stand-in until O9's balance exists. Alternative: type `intentional`, protected, but `outlet: active` (it spills) until the balance decides. The order names a terminal sink, so the note builds that and records the stand-in.

## 3. Switch and rows

- `--intended-basins off|on` on rg_chain (spike), default off; off is bit-identical to today (the forced list unchanged).
- The five windows: none holds an endorheic basin (`g_basins_endorheic` 0 on all five). With the switch on they are regression rows: marked cells per window are printed, and a window with none must match off bit for bit.
- The 10 unseen windows (crates/cache/data/g3-windows.toml): 5 and 6 are arid-endorheic (runoff 44 and 15.6 mm), and 1 and 2 are dry cube edges (20.3 and 19.0 mm). They are the cases G3-8 needs.
- Conflict: G3-8 runs the 10 unseen windows on frozen parameters. Tuning the mark on windows 5 and 6 would spend them.
- Proposal: calibrate on two new arid draws outside the 10, with the same draw procedure (#186a) and a new draw seed, set by the lead. The 10 are run only at G3-8 with the switch on.
- Measures per row:
  - intended basins (count, km2) and the cells marked;
  - `g_basins_endorheic`, `kept_lake_km2`, bar_violations, the trunk exceptions, the closed-basin bar and agree_*, all measured outside intended basins.

## 4. Planet form

- The mark is a pure point function. The province nodes are computable per tile plus a one-node margin from the per-seed `ClassSource` (64 plate draws, no grid). Aridity reads the per-face orography that `BakeInputs` already keeps.
- So it adds no landmass-resident state (A4, R13); the only face-resident input is the orography, which is already resident for the base.
- Per tile: the depression hierarchy with the forced list from the tile's marks. The seam engine (`SpillGraph::connect_seams`, GEN-191) joins the tiles' spill graphs across intra-face, cube-edge and corner seams.
- Partition-independent (CON-30): whether a basin is intended is read at its floor cell. The floor is the subtree's lowest cell, ties broken by `GlobalCell` id, so the same cell is chosen however the tiles split.
  - Rule: a joined basin is intended when its global floor is marked.
  - A cube-corner patch holds one `GlobalCell` twice and treats it as one node.
- A basin spanning tiles is decided after the join, not per tile. The tile pass only carries the mark.

## 5. Contract readings it changes

| Reading | Today | With R23 |
|---|---|---|
| 22 | intended basins before a production bake; every H1 basin drains | intended basins on (tectonic mark); first in the ranking of §2 |
| 24 | no H1 basin is a lake in the prototype | an intended H1 basin is kept, endorheic; the rest as today |
| 13 | a trunk crosses a deep kept H1 basin with `exception(rise_over_budget)` | a trunk ends at an intended basin's inlet; no exception |
| 16 | a crossed basin is not a lake (`q_w = q`) | an intended basin is a sink: `q_w = 0` out |
| 6 | a spill path to the sea gets `outlet_sea` | not for an intended basin (no spill path) |
| 4 | an endorheic basin has a spill anchor | unchanged; the spill anchor is the id, no path |
| 37 | closed-basin bar 10 %, measured outside kept-lake interiors | also outside intended basins |
| A5 cap | a breach over 10 m keeps the depression | intended basins rank above it |
| §10 choice 3 | no external outlet | unchanged: a sink is internal |

- Main's gate (BUILD-055/057/060) then needs to read the graph's endorheic basins: routed-graph agreement, uphill and the closed-basin bar must treat them as sinks. That is one main change before G3-8 (a small OpenSpec change on build-tooling).

## Build plan after the lead's reply (spike)

1. The mark (`I` and the gate) per L cell on the region, printed counts. One commit.
2. The forced list from the mark in basins-2 (and its early pass under decision 11), and the H1 trunk end. One commit.
3. Rows: the five (regression) and two new arid draws; the 10 only at G3-8.
4. Main: the gate's sink handling as an OpenSpec change.

## Revisions (lead, 2026-11-24, after the note)
| Item | Note above | Now |
|---|---|---|
| Type and outlet | `endorheic`, a terminal sink | `intentional`, an active outlet: spill path recorded as for any kept lake (readings 4, 6, 16 unchanged); readings 13 and 16 and main's sink handling dropped |
| Aridity | a gate (P/PET < 0.2) | a record only: `endorheic_candidate`, an input for O9's water balance (ADR 0111 point 5) |
| Mark | max(basin, rift, back_arc x plateau_field) | `basin >= 0.5` alone |
| Rift, foreland | in the mark (first row) | dropped: uplifted provinces about 200 km wide; R1h's first row marked 24 % of cells and kept 5,023 humid hollows |
| Back-arc | back_arc x plateau_field | unreachable at 0.5: plateau_field carries (1 - smoothstep(0.2, 0.6, max(flank, back_arc))) (classes.rs:520), so the product peaks near 0.25; no back_arc-only term (it marks the whole raised plateau) |
| Size | basins-2's kept size (0.25 km2, 2 m) | area >= 10 km2 (a 39 km lattice does not decide small hollows) |
| Calibration | the 10 unseen windows 5 and 6 | two new arid draws (draw seed 2026112403; crates/cache/data/g3-calibration-windows.toml, spike); province fields read only there and on the five, never on G3-8's windows |
| Missed basins | - | on the calibration draws: closed arid depressions >= 10 km2 that the mark misses, with their province values at the floor, for the lead's decision on back-arc |
| Freeze | before G3-8 | after the calibration rows: `basin >= 0.5`, more than half marked, spill < floor + 10 budgets, area >= 10 km2, as one contract reading |

## Rulings of 2026-11-25 (lead #203b and #203c)
| Item | Now |
|---|---|
| Set | fixed on S0 (outlets: the sea and the window edge); basins-2 forces exactly this set (it ranks first, so they agree by construction) |
| Protection | a constraint in every stage (KH, KS0, KS1, band6 and its connect, KV, KC): no stage lowers a rim ring or outlet-path cell below min(spill, S0); interiors may change; band6 protects rims only |
| Lake water | KC fills intended basins to the spill like any kept lake (active outlet); the note's "no lake water until O9" is superseded |
| Selection | basin term >= 0.50 at the floor, more than half marked, area >= 10 km2, spill < floor + 10 budgets, P/PET at the floor < 0.20 |
| Aridity | the selection's ceiling (0.20) and `endorheic_candidate` (P/PET < 0.2, O9's input) are equal in value and kept apart in name |
| Frozen | basin >= 0.50, area >= 10 km2, P/PET < 0.20 (sweeps: threshold 0.50 / 0.45 / 0.40, ceiling 0.20 / 0.35 / 0.50; tmp/g203c, g203d) |

### Arid closed depressions outside the basin term
- c0: 13 (473 km2); c1: 1 (10 km2). P/PET < 0.2, area >= 10 km2, depth >= 2 m, on S0.
- Their basin term peaks at 0.476 (c0 p50 0.167); back_arc, rift 0; foreland < 0.06.
- They miss the basin term at every threshold and ceiling swept, so they are a base or tectonic-field item, not R23's.

## Frozen (2026-11-26, contract reading 48)
- Built on spike/rg-prototype up to 0d4a1689b (#203e rim floor in every stage, #203f KG-trunk split, #203g band6 keeps them and reading 17's exception).
- Inertness on 0d4a1689b: R1h, R3a, R3b, M1 and F1 on equal off by SHA-256 (graph, final and every stage file), and equal the earlier builds' off hashes.
- Row 3's worklist: c0's 8 trunk cols at intended rims (lanes/digest/river-graph/r23-c0-trunk-cols-2026-11-26.csv; the basin ids are the nearest intended floor or spill to each inlet). Their rises over the budget are 24.76, 22.42, 17.50, 3.60, 23.38, 14.32, 12.54 and 4.66 m.
