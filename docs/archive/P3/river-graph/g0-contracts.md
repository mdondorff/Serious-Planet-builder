# G0 contracts: river graph, basins, rasterisation, budget, partition, edits

Status: agreed (owner, 2026-11-16: "let Fable check this, then this is approved"); draft 2 takes in every blocker and improvement of `critique-fable-g0.md`. Implements `g0-contract-addendum.md` §2 for `plan.md` (draft 4). Changes after agreement go through the lead with evidence; no lane redefines a term here (addendum §6).
Builds on: `hydro.rs` (`Segment`, `Basin`; GEN-145), `channel_graph.rs` (GEN-170), `river_suite.rs` and the river gate (BUILD-055, BUILD-057).

## 1. Levels and domains
| Level | Cells | Producer | Content |
|---|---|---|---|
| H1 | 814 m global cells | K6' | trunk reaches (>= 75 km2), H1 basins |
| L | 101.8 m global cells | KG-trunk, KG-trib | L images of trunk nodes and reaches, tributary reaches (>= `A_crit`), L basins |
- A `domain` is `landmass` (H1, every trunk reach at both levels, H1 basins and their L images) or `tile(face, ti, tj)` of K7's canonical tile grid (GEN-182).
- A region, a window, a store and a halo are never domains and never enter an id or a stored value.
- One canonical graph spans both levels. A store holds slices of it; a store lacking the owner of a `down` node holds an open slice, reported as `missing owner(tile)`: a store fact, not a node kind.

## 2. Graph entities
| Entity | Fields | Rules |
|---|---|---|
| Node | `id`, `kind`, `level`, `anchor` (global cell id at its level), `parent` (H1 node id, L trunk images only), `owner` (domain) | kinds: `source`, `confluence`, `trib_junction` (marker), `exception` (marker), `tile_crossing`, `outlet_sea`, `lake_inlet`, `lake_spill` |
| Reach | `id`, `level`, `up`, `down` (node ids), `vertices` (8-connected chain of global cell ids from `up.anchor` to `down.anchor`, both included), `parent` (H1 reach id, L trunk images only), `owner`, attributes (§3), `status` | directed downstream; acyclic |
| Marker node | `on_reach`, `at_vertex` | no reach starts at a marker; only tributaries end at a `trib_junction`; an `exception` never splits a reach |
| Confluence | two or more incoming reaches, one outgoing | connections exist only as nodes; cell overlap never connects reaches |
| Outlet | `outlet_sea` (first sea cell) or `lake_inlet` (enters a basin) | every chain ends in exactly one outlet; there is no external outlet |
| Tile crossing | a level-L chain leaving a tile core is cut at a `tile_crossing` node on its last vertex inside the core it leaves | a tributary reach lies in one tile core plus its first vertex; trunk reaches are never cut at tiles |
| Exceptional reach | `status = exception(reason, value)`, reasons `rise_over_budget`, `unresolved_endpoint`, `budget_residual` | reported, never repaired silently; counted in every gate |

### 2.1 Uniqueness and identity
- At most one node per (level, anchor). Tributaries ending at one trunk vertex end at one `trib_junction`.
- Every node except `outlet_sea`, `lake_inlet` and markers has exactly one outgoing reach (at most one reach per (level, up)).
| Item | Id |
|---|---|
| Node | `hash(body, kind, level, anchor)` |
| Reach | `hash(body, level, up.id, down.id)` (GEN-145's rule on node ids) |
| Basin | `hash(body, level, floor anchor)`, ties by the global cell id (GEN-145) |
- A trunk node at L is the image of one H1 node: `anchor_L` = the lowest L cell of `S0` within one H1 cell of the H1 anchor, ties by the global cell id (BUILD-057's snap, on `S0` only, so KS parameters never move an id); `parent` = the H1 node id. A trunk reach at L has `parent` = its H1 reach id; images are 1:1.
- An H1 basin has at most one L image: the L basin whose cells hold the H1 floor cell, `parent` = the H1 basin id; the image's spill height is the protected surface at L.
- Ownership never enters an id. A raster cell anchors a node; it never identifies a reach.

### 2.2 Topology and attribute changes
- Topology change: a node or reach id added or removed (a kind change on an anchor retires the id; a reattachment changes `up` or `down` and so retires the reach id).
- A trunk geometry change that moves the cell under a `trib_junction` retires the junction and its tributary's reach id (topology).
- A new confluence on a reach retires it and creates two new reaches; a removed confluence merges two into one new id. Trunk reaches split only at H1 events.
- Attribute change: same ids, different `a_*`, `q`, `q_w`, order, bed, geometry or status.
- A diff reports both kinds separately and names (id, field) per difference.

## 3. Reach attributes (per vertex unless stated)
| Field | Meaning | Unit | Rule |
|---|---|---|---|
| `a_geo` | geometric contributing area | m2 | `a_cell(v) + Σ donors + a_ext(v)`; conserved at confluences to 1e-9 relative |
| `a_ext` | area entering at a window-entry vertex from outside the tile window: H1's `A_d` of the H1 cell holding the vertex minus the window's own area draining to it (zero elsewhere) | m2 | `a_run` stored with and without it, so G1d measures the seed |
| `a_run` | runoff-weighted area (GEN-145's `A_d`) | m2 | the channel threshold reads it |
| `q` | potential discharge (GEN-145): local term plus donors | m3/s | conserved at confluences to 1e-9; over the landmass the sum at outlets equals the sum of local terms |
| `q_w` | routed discharge: `q` with the basins' losses, `q_out` below a basin | m3/s | conserved at confluences; the width law (GEN-165) and the store code (GEN-170) read it |
| `strahler` | order per vertex, changing only at nodes and `trib_junction` vertices by the standard rule (inflows: the trunk upstream of the vertex and the tributaries ending there); a reach's order is that of its last vertex; every `source` is 1 | - | |
| `bed` | bed level | 0.5 m codes (GEN-170) | non-increasing on codes downstream except across `lake_spill` and `exception`; a node's bed is one value shared by its reaches; at a `trib_junction` the bed is the trunk's bed at `at_vertex`, and the tributary's last vertex carries it |
- A tributary continuing above a trunk's head ends at a `trib_junction` at vertex 0, never at the trunk's `source`.
- `width` is derived from `q_w` by GEN-165, not stored in the graph.

## 4. Basins (topology, no water balance)
| Field | Meaning |
|---|---|
| `id`, `level`, `parent` | nesting: a basin merged at its spill into a larger one has it as parent; L images carry the H1 basin id (§2.1) |
| `floor` | anchor cell, height |
| `spill` | spill cell, height, receiving node id (a `trib_junction` on a reach or another basin's `lake_inlet`; never a bare reach id) |
| `outlet` | `active` or `endorheic` |
| `represented` | `protected_surface` for every kept basin; `none` only for reported depressions |
- KS, KV and KC never change a cell of a protected basin: its ground stays `S0`. The flat surface at spill height is water (GEN-167's mask), not ground.
- Kept: H1's kept set; L basins from 0.25 km2 and 2 m. Basins are counted per basin (count, area, volume).
- Water balance, evaporation and time-varying levels are deferred.

## 5. Rasterisation (derived, deterministic)
| Question | Rule |
|---|---|
| Rasterised reaches | level L only |
| Overlap | a cell belongs to one reach: the larger `q_w`, ties by the lower reach id |
| Junction cell | belongs to the downstream reach |
| Tile borders | §2 (`tile_crossing`); both tiles compute a crossing from their own window; a mismatch is `unresolved_endpoint`, counted in G1d |
| Cube-face edge | the L window is GEN-182's extended plane; a face-crossing reach is gated at H1 (GEN-170's rule) and reported at L until the face-seam work (REND-063) settles it |
| Raster to graph | per L cell `(reach id, vertex index)` or none; derived, never authority |
| Agreement | a routed river cell of the final surface agrees when a rasterised graph cell lies within 1 cell (Chebyshev); printed: the share of routed river cells agreeing and the share of rasterised cells with a routed river cell within 1 cell; bar set from the pre-phase baseline before G1c; plus BUILD-057's corridor at `k_c` 2 snapped at the owner's share |

## 6. Terrain-change budget
- `relief_1km(c)` per 814 m cell: the range of `S0` over the L cells within 5 cells (Chebyshev) of `c`'s centre L cell.
- `relief10(x)`: the p50 of `relief_1km` over the 814 m cells within `R_relief` = 25 km of `x`'s parent cell; a K6' landmass field read at L through the parent.
- `budget(x) = min(50, max(10, 0.5 × relief10(x)))` m against `S0`. The five windows also print BUILD-057's window bar for B's table; gates use the field.
- Lowering per stage: `used_stage(x) = max(0, in − out)` in f64. Order: KS (`KS_used = S0 − KS1`; KS0 is reported, not a budget stage), then KV, then KC; each lowers at most `budget(x) − used_so_far(x)`.
- The bed's cut at a vertex is at most `budget(x) − KS_used(x)`; a larger rise makes the reach `exception(rise_over_budget)`. The D-2 list on `S0` (G1a, BUILD-057) and this remainder list are two lists.
- Raising per stage: `fill_stage(x) = max(0, out − in)`; KV and KC fills each at most `budget(x)`; an overrun is a residual.
- A clamp writes a residual `(cell, stage, needed_m, available_m, feature id)`; the feature gets `exception(budget_residual)`.
- Validators run on f64. The diagnostic field per stage is u16 `floor(used / 0.1 m)`, never the gate. An injected overrun, and an injected clamp without a residual, both fail.
- Amended (owner O3, 2026-11-23; measured by explorer #199, 2026-11-24): the budget holds **net against `S_KH` at every stage boundary**: `max_k (S_KH(x) - z_k(x)) <= budget(x)`, with `z_k` the surface after stage k. A fill between two lowerings is credited. The sum of `used_stage` over the stages (the gross count) is reported, not the bar. On today's defaults (5ee9987e): net 0 overruns on all five windows; gross R1h 100 (KV 97, KC 3), M1 1,298 (KV 1,292, KC 6), and 0 on R3a, R3b and F1. Every gross record is a cell lowered, filled back by band6, and lowered again.

## 7. Determinism, partition, edits
| Test | Pass |
|---|---|
| Same inputs, 1 and N threads | canonical serialisation and surface bit-identical (CON-14) |
| One region against two regions | shared slices and shared tiles bit-identical (CON-30) |
| Tile crossings | no missing or duplicate id; every endpoint resolves or is `unresolved_endpoint` |
| Fresh against incremental after one representative edit | bit-identical (CON-29); dirty set reported |
| Attribute-only change | same ids, attribute diff only |
| Topology change | reported; an H1 diff dirties KG-trunk and every tile whose core or window a changed reach touches; a KG-trib diff dirties its owning tile and the tiles whose window holds its vertices |
No tolerance beyond the existing numeric contracts.

## 8. Serialisation and validators
- Canonical form: nodes, reaches, basins sorted by id; fixed field order; f64 as IEEE-754 bit patterns in hex; codes as integers; incoming reaches sorted by id. Diffs name (id, field).
- Validators, each with an injected-failure test: cycle; duplicate id; two nodes on one (level, anchor); unresolved endpoint; chain without an outlet; confluence with one inflow; a non-outlet node with zero or two outgoing reaches; invalid edge (`up` of an outlet kind, `down` of kind `source`, ends at different levels, `vertices` not an 8-connected chain from `up.anchor` to `down.anchor`); bed rise on codes outside allowed nodes; conservation break; two reaches owning one cell; budget overrun; clamp without residual.

## 9. Metric definitions (fixed before any RG result; B and RG through the same build)
| Metric | Definition |
|---|---|
| G-DRAIN | BUILD-055's basin rule verbatim (deeper than 0.5 m, at least 16 cells, share of the gate's river cells); protected basins excluded |
| Valley depth, width, asymmetry | BUILD-057's definitions (depth above the river cell; asymmetry `|L − R| / (L + R)` of the rise 2 to 4 cells out; per river cell); width at 50 m depth added to BUILD-057 by a spec delta |
| Ridge cell | a land cell of `S0` inside the core, at least 2 m above both cells 2 L cells away on either side along a face axis or a diagonal (SPIKE-122 §6, restated) |
| Cut-through saddle | a distinct ridge cell lowered more than 20 m against `S0` with a new channel of at least 1 km2 (and 5 km2) within one cell (SPIKE-122 §7, restated); ridge lowering also reported as a share of ridge cells over 20 m and its p95 |
| GLO-30 match | BUILD-055's: a tile with 1-2× the window's relief at 10 × 10 cells; the density and `R_b` baseline is BUILD-059 once its requirement lands (explorer #163) |
| Ground lowered, relief lost | BUILD-057's `lowered`, and `relief10_m` against `relief10_start_m`, with `S0` as the start |
| Graph/raster agreement | §5 |

## 10. Choices made (owner may change)
1. Node ids anchored on global cell ids at the node's level, with `body`.
2. Trunk reaches not split by tributaries.
3. Domains only `landmass` or canonical tiles; no external outlet.
4. `R_relief` = 25 km.
5. Fills capped per stage and reported apart from lowering.

## 11. Fixture (TEST-139, crusader #154)
| # | Feature (addendum §2.7) | Tests that read it |
|---|---|---|
| 1 | one trunk and two tributaries | topology, ids, conservation, Strahler |
| 2 | one confluence | conservation, bed sharing |
| 3 | an antecedent river crossing a ridge | budget, `rise_over_budget`, D-2 list |
| 4 | a closed basin, protected | basin contract, protected surface untouched |
| 5 | a divide close to a region boundary | partition (layout 2 cuts here) |
| 6 | a reach crossing a tile boundary | `tile_crossing`, partition |
| 7 | a reach crossing a cube-face edge | gated at H1, reported at L |
- Body: the cube sphere of GEN-145's tier-A test (24 cells a face edge at H1) with L at 8 cells per H1 cell over the fixture's faces; cells and faces of each feature in TEST-139's expectation file.
- Layouts: one region; two regions cut along a tile line through features 5 and 6.

## 12. Readings (lead, 2026-11-16, DEC crusader-6; easy to reverse)
1. Uniqueness (§2.1) holds among non-marker nodes. A marker may share its anchor only with the `up` node of the reach it sits on (a `trib_junction` at vertex 0 at a trunk head).
2. An L `outlet_sea` image is anchored on the first sea cell of its L chain (the kind rule wins over the snap rule); `parent` = the H1 outlet node.
3. The graph validator's conservation: every reach at a confluence carries one shared value at the node's cell; that value is at least the sum of the inflows one vertex upstream (1e-9 relative); `a_geo`, `a_run`, `q`, `q_w` never fall along a reach. The exact per-cell rule stays GEN-145's raster check.
4. (DEC crusader-7, 2026-11-17) Basin id = `hash(body, level, floor anchor, spill anchor)`: a parent shares its floor with its deepest child. The spill anchor is the topological spill cell, which an endorheic basin also has; 0 only for a basin with no spill cell at all. GEN-189 and its id validator follow in a main PR.
5. (DEC crusader #156, 2026-11-17) The L kept-basin rule reads the conditioned surface KS1, not `S0`: on `S0` the kept set (3-19 % of land, the largest on trunk lines) is mostly the hand-over's dams, which KS removes. Protected: H1's kept basins with their L images, and the L basins of KS1 (>= 0.25 km2, >= 2 m). `S0`'s basins are reported as a diagnostic. If KS1's kept set holds over 10 % of lowland river length (review B Q8's upper bound), the owner decides the share (ADR 0127 point 6).
6. A basin whose spill path reaches the sea before any reach receives an `outlet_sea` node anchored on the first sea cell of that path; `spill.receiving` may be a `trib_junction`, a `lake_inlet` or an `outlet_sea`.
7. (spikes #132 outbox, 2026-11-17) Two H1 nodes snapping to one L cell: images are snapped in order of descending `q_w`, ties by H1 id; a cell already taken is excluded from the next snap. Images stay 1:1.
8. Markers are unique per (reach, anchor): a marker id is `hash(body, kind, level, anchor, on_reach)`; reading 1 still holds for non-markers.
9. (DEC crusader #157, 2026-11-17) Every stage records a wanted fill beside its wanted lowering; a fill held at the budget writes a residual like a lowering clamp. A reach carries a list of exceptions (each `reason, value`), not one status.
10. The divide buffer of KV applies to a divide cell only when it lies farther than `W_floor(q_w)/2 + d_buf` from every graph line; near a confluence the catchment boundary meets the river and is a valley floor, not a divide. G1f reports the share of divide cells exempted.
11. (spikes SPIKE-129, 2026-11-17) A trunk's L centreline is the lowest path in its corridor: Dijkstra with cost length × (1 + height over the corridor minimum / 10 m); BUILD-057's maximin route stays the corridor gate's measure, not the centreline.
12. (DEC spikes-4 a, A) Sea at L: cells at or below the sea level connected to an H1 sea outlet's L cells or to the domain's sea. Other cells at or below the sea level are land (they become basins or repairs); they never end a route.
13. (DEC spikes-4 b, A, prototype rule) A trunk is cut at a kept H1 basin (lake inlet and spill nodes) only where the basin's depth at its floor is within the budget there. A deeper basin is crossed along H1's chain; its spill becomes `exception(rise_over_budget)` and is counted. Which deep basins are real lakes is decision 5 (the kept-lake share), measured before G1f.
14. (DEC crusader-8, 2026-11-17) KG-trib channels start on the D8 accumulated area (`a_run` on D8, so the stored `a_geo` matches the initiating area); clipped MFD initiation gave valley-floor stubs (187 against 55 confluences, order-1 reaches ending at 0.07 km2). MFD stays for diffuse hillslope area only.
15. Marker id formula (reading 8 made exact): `id_hash(node_id(body, kind, level, anchor), on_reach, 0x8A2E_0008)` with GEN-145's mixer, as spikes' G1c (`spike/g1c-trunk-k6p-2`, `kg_trunk.rs` about line 1537).
16. (SPIKE-134, 2026-11-17) A kept H1 basin crossed under reading 13 is not a lake: through it `q_w = q` (no basin loss), so conservation holds; only a cut basin (lake inlet and spill) applies its losses.
17. (explorer SPIKE-128, 2026-11-17) No stage lowers a land cell to the sea level or below: KS, KV and KC clamp at the sea level plus 0.5 m (one bed step) and record a residual; the sea line is the connected sea of reading 12.
18. (crusader SPIKE-133, 2026-11-17) Areas and discharge on L trunks are accumulated at L on the combined graph: a trunk vertex carries its upstream L trunk value plus every tributary ending at it plus its own local term; H1's values seed only H1 heads and halo inlets. A landmass pass over the combined graph (sparse) does this before KV.
19. (SPIKE-136, 2026-11-17; plan amendment) The hand-over stage KH reads `z_s` at L as two triangles per 814 m quad: a quad holding a D8 diagonal step of `z_s` is split along that step; every other quad along the diagonal that keeps the higher pair connected. No 814 m height changes. KS starts from `KH(z_s) + r` (the band floored at the sea level plus 0.5 m near the connected sea, reading 17) in place of `C1(z_s) + r`. The facet look is the owner's call in the G2 flight.
20. (SPIKE-136 addenda 2-3, 2026-11-17; replaces reading 19's form; clamp confirmed #1591) KH (h4c) is Clough-Tocher C1 patches on reading 19's split, with every D8 step edge monotone, clamped per quad to the range of its four corners (no overshoot, no undershoot below the lowest corner). The plain triangulation (h2) failed the look (facets); the skeleton (h3) grooves the ground. The clamp is confirmed by #139 before G1f.
21. (owner, 2026-11-17, decision 3: dual reference) The per-cell stage budget (`KS + KV + KC <= budget`) and every L-stage preservation delta (saddles, lowered over 50 m, crest rounding p95) are measured against `S_KH = KH(h4c)(z_s) + r'` (KS's base). KH's own change is reported against `S0` in a hand-over column. G2 prints both `vs S_KH` and `vs S0`; B on `S_KH` too if cheap.
22. (owner, 2026-11-17, decision 6) Replaces reading 5's protection: an L depression (>= 0.25 km2, >= 2 m) on KS0/KS1 is protected only when both its breach and its fill exceed the remaining budget; every other one drains (KC fills or breaches it, least-cost breach added to KC). H1's kept basins: prototype rule (c), every basin drains; intended basins (tectonic plus authored) before a production bake.
23. (owner, 2026-11-17, decision 5) KS never lowers a cell below the fill level the band's own drainage (`drain_band`) set; or `drain_band`, capped by the budget, runs after or in place of KS and KC.
24. (explorer question, 2026-11-17; follows decision 6 (c)) In the prototype no H1 basin is a lake: reading 13's cut is off, every trunk crosses its basins (reading 16: `q_w = q`), and a crossing whose rise exceeds the budget is `exception(rise_over_budget)`, with the budget read on `S_KH` at KG-trunk (which runs after KH). KH need not move before H1.
25. (SPIKE-136 addendum 4, #1596; owner's look request, 2026-11-17) KH = h5: the smooth C1 read everywhere, and h4c only on 814 m cells of H1 channels (>= 75 km2) and their 8 neighbours, blended into C1 over one 814 m cell. Trunk drainage as h4c (within 0.4 points); tributaries +2.3-3.9 points of river length in closed depressions, left to KS and KC. The owner judges the sheets.
26. (spikes, from explorer's g1f2 sheets, 2026-11-18; refines reading 24) H1 marks a filled basin's whole flat floor as channel. Inside a crossed basin, only the segments fed from outside it are trunk reaches: a segment that starts outside the basin, or has a kept upstream segment, down to the spill; segments wholly inside and fed by none are the floor's own drainage and are dropped. One converging path per inflow remains.
27. (technical review of the runner, 2026-11-18) The h5 zone is H1's channel cells (GEN-145, runoff-weighted area >= 75 km2) and their 8 neighbours, not KH's own D8 area on `z_s`.
28. The budget's size field `relief10` is computed on `S0` (contract §6; a property of the terrain before any conditioning, the same for every variant); the lowering it limits is measured against `S_KH` (reading 21).
29. (crusader DEC crusader-15, 2026-11-18; lead) KV cuts the floor and a wall decaying at `s_wall` from the line's own height above its bed; it does not cut valley sides steeper than `s_wall` down to the budget. A bounded side band is a later switch if the owner's flight asks for it.
30. (crusader SPIKE-140 items 1-2, 2026-11-18; lead) The prototype runs with GEN-140's per-octave lattice turn on (`rg_chain --turned-noise`): the runner never set the process flag, so every chain run so far had the axis-aligned base lumps. Default on for G2's prototype runs; B is rerun with it on too, so both share the base.
31. (same) K5 and H1 use the cube-face cell metric; the L stages (KS, KV, KC, KG-trib, the gate) use uniform 101.8 m cells, while the real cell edge varies 0.72-1.38 times across the five windows (6-13 % inside one). Open: whether the L stages take the metric (slopes, the budget's relief, `s_wall`); measured before G2, not yet a rule.
32. (crusader DEC crusader-16, 2026-11-19; lead) No stage cuts past the budget, also for one-cell links (contract §6: a need beyond the budget is recorded, never done). On a two-vertex trunk reach, which has no inner vertex for a marker, the rise_over_budget exception excuses the rise at the reach's down end; the validator (GEN-189) accepts a rise there only when the reach carries that exception with the rise's value.
33. (spikes SPIKE-139, 2026-11-19; lead) K5 without a hillslope term builds 814 m checkerboard relief, which C1 shows as the comb on diagonal valley sides. A linear hillslope term in K5 (`--k5-diffuse 0.02`) is the source-stage candidate; its relief cost (14-22 %) and a k_dt compensation are shown to the owner before any default change. D8-LTD and D-infinity are not taken further.
34. (crusader #168, owner 2026-11-19) KG-trib runs a second pass on band6's output, before basins-2 and KV (`--trib-on post`, the default): the tributaries are extracted on the floors the band stages leave, not on KS0.
- Reading 33 amended (owner, 2026-11-19): `--k5-diffuse` stays off for G2 unless g1f8's unscaled form beats the default on all five windows; k_dt is not scaled.
35. (crusader #171, 2026-11-20; lead) A depression whose rim an L line crosses with a bed below its spill - 0.5 m is drained by that line and is not a kept basin (basins-2). The gate's `spill_mismatch` (a rim vertex bed below spill - 0.5 m, or the final depression level 0.5 m under the recorded spill) is a graph violation.
36. (spikes SPIKE #154, 2026-11-20; lead) K5's window edge is a free outflow boundary (`--k5-edge free`, the default): a window-cut catchment's edge outlet is breached like any receiver; sea outlets keep their height. The dam form made edge lakes on every window and flooded M1's box (basin_pct 10.5 -> 4.1 %). Main's region bake has the dam form: a G3 seam item.
37. (owner, 2026-11-20, option C) A basin is drained only by a line leaving it below its spill (`--kept-drained exit`, the default). The closed-basin bar (river length in closed basins <= 10 %) is measured outside kept-lake interiors, as the uphill and agreement bars are; the gate prints the kept-lake share (river length and area inside kept lakes) in its own column.
- Reading 37 amended (owner, 2026-11-20, three conditions): (1) KV protects kept-basin rims, so spill_mismatch = 0 holds on all five windows, and spill_mismatch counts in bar_violations = 0. (2) BUILD-055's raw basin_pct stays unchanged; new bar_ponds <= 10 % over (river cells - kept river cells), and kept_river_pct <= 5 % (lowland) / <= 10 % (mountain); the G2 report states R1h's and F1's unintended ponds (9.1-9.6 %, from the agree2_routed_kept split). (3) The exit-minus-drop lake sheets are regenerated inside the box on `--k5-edge free` (the crusader172 `*-exit-lakes.png` crops are exception-lake plots, mislabelled).
38. (spikes #158, 2026-11-21; lead) Carved lines are 4-connected (`--ks-connect 4`, the default): on each diagonal step KH's h5, KS and the post-band6 pass lower one shared edge cell to the step's mean bed, within the budget, so a line never meets only at cell corners (corner-only pits on R1h's final 25,506 -> 3,556; gate bars unchanged). KV's own vertex lines get the same rule (crusader).
39. (spikes #160, 2026-11-21; lead; default after F1's and R3a's rows) Every stage uses the one connected-sea mask of reading 12 (`--sea-mask connected`): inland cells below the sea level are land, not outlets, for KS, KG-trib, the basin passes, band6, KV and KC. R1h: uphill 1.27 -> 0.45 %, basin share 11.18 -> 3.46 %, all bars pass.
40. (spikes #161, 2026-11-21; lead) KH's h5 never raises the ground (`--kh-lower-only`, the default): KH = C1 + w·min(0, h4c - C1). The raised side lobes (R1h: 1.07 M cells over 0.5 m, up to 155 m) closed depressions that band6 then breached. Bars a little better (R1h uphill 0.45 -> 0.32 %). band6's remaining hump breaches (about 900 per window over 10 m) stay open for the owner: fill (`--band6-hump`, costly), or a kept lake behind the hump (not built).
41. (crusader DEC crusader-29, 2026-11-22; lead) Reading 32's form extends to `budget_residual`: a rise into a reach's own down node, where no marker may stand, is accepted by the validator (GEN-189) only when the reach carries `exception(budget_residual)` with a value at least that rise. A larger rise, a rise at any other vertex, or a rise without the record stays a violation. The gate prints the excused rises (count, metres) beside `bar_violations`.
42. (crusader DEC crusader-31, 2026-11-22; lead; owner's ablation rule, review-response §3) The prototype's defaults add `--band6-floors on` (band6's breach floored per cell at the budget KS left), `--inlet-floor on` (graph beds on and inside a kept rim floored at the spill, stopped carries recorded as `budget_residual`, reading 41) and `--trib-once on` (no basins pass 1, no KG-trib pass 1). The combination passed the rule on R3a, R1h and F1; `--sea17` stays off (it broke R1h's bars); reading 17 is fixed upstream in the bed coding at G3.
43. (explorer #191, 2026-11-23; lead; ablation rule) band6's breach keeps the per-cell floor at its end cell too, including outlets (`--band6-outlet-floor on`, the spike default): an outlet loses its exemption from the floor in the breach search and in the floor check. R1h and M1: band6's cuts past the budget 11 and 17 -> 0; every gate share and bar unchanged; pits beside the window border stay filled.
44. (crusader #182b, 2026-11-23; lead; ablation rule) After basins-2, KG-trib post runs once more with basins-2's kept set, and basins-2's producer then runs once more on the new lines (`--trib-rerun on`, the spike default): a line no longer ends at an inlet of a basin the drain rules dropped. R1h: orphan inlets 146 -> 0, line-end inlets without a kept basin 992 -> 0 (chain), agreement 92.49 / 98.12 -> 92.73 / 98.26; F1 5 -> 0. A spike stopgap: the port makes the kept set final before line extraction (seam brief).
45. (spikes #182, 2026-11-23; lead; ablation rule) Trunk accumulation counts an H1 headwater's 75 km2 once (`--trunk-head-once on`, the spike default): `accumulate_trunks` no longer adds the head's area again at the first L vertex. R1h, R3a, M1 pass: trunk areas at reach ends p50 x0.87; violations 102 -> 101 / 0 / 1,306 -> 1,300; agreement at 75 km2 +0.13 / +0.11 / +0.04; saddles -15 / 0 / -24. `catchment_p90` reads the routed network and does not show it.
46. (spikes #182, 2026-11-23; lead; ablation rule) Trunk Strahler orders are updated at every `trib_junction` (`--strahler-junction on`, the spike default): surfaces bit-identical. `g_rb` (GLO-30's R_b at 1 km2: 4.14-4.42 on R1h's match) R1h 7.66 -> 5.24 (still above), R3a 5.36 -> 4.78, M1 7.01 -> 4.34.
47. (owner O3, 2026-11-23; explorer #199, 2026-11-24; lead) The chain's budget validator is net (`validate_budget_net`, the spike default) and feeds `bar_violations`; the gross validator stays as a reported column. Section 6's amended line is the rule.
48. (owner decision 6(a); lead #203-#203i, 2026-11-25/26; explorer) Intended basins (R23; `--intended-basins on`, off by default until G3-8).
    - Selection, on S0 with the sea and the window edge as outlets: an L depression is intended when
      - the basin province term is >= 0.50 at its floor and over more than half its cells;
      - its area is >= 10 km2;
      - its spill is under floor + 10 budgets;
      - P/PET at its floor is < 0.20.
      The largest such node per floor is chosen.
    - Type `intentional`, with an active outlet: spill path and spill node as for any kept lake (readings 4, 6 and 16 unchanged). `endorheic_candidate` (P/PET < 0.2) is recorded per basin for O9; it decides nothing.
    - The set is fixed on S0, and every lowering stage keeps it as its own bound (KH, KS0, KS1, the KS and band6 connects, band6, KV, KC): no stage lowers its rim ring or outlet path below min(spill, S0). band6 does not decide it, and KG-trib and KG-trunk end at its inlet and leave at its spill. basins-2 keeps it ahead of A5 and reading 22.
    - Reading 17's exception: the sea + 0.5 m floor does not apply inside an intended basin. A closed basin below the sea level that is not connected to the sea is land with a lake.
    - Calibration (crates/cache/data/g3-calibration-windows.toml):
      - c0 keeps 55 of 55 with no new violation (0d4a1689b); c1 kept 19 of 19 on f574668fd (not rerun after #203g).
      - R1h, R3a, R3b, M1 and F1 mark nothing and are bit for bit equal to off.
    - Known case: c0's trunk exceptions are 1.05 against <= 1 (21 reaches against off's 13). The 8 extra are trunk routes crossing an intended rim away from its spill, 3.6 to 24.8 m over the budget (lanes/digest/river-graph/r23-c0-trunk-cols-2026-11-26.csv). They are recorded for row 3's worklist and not excused.
