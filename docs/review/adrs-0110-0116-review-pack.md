# Review pack: ADRs 0110-0115 and constitution v1.7 (ADR 0116)

Status: for the owner's review, then an external review (architect, 2026-11-11). Main at the time of writing: the ADR text below is the text after the alignment pass of 2026-11-11 (branch `docs/adr-0110-0115-simplify`).

## 1. Purpose and reading guide

- **The decision asked:** accept ADRs 0110-0115, all six or a named subset, as the architecture of the terrain prototype. ADR 0116 (constitution v1.7) is already accepted; it is here as the baseline.
- **Acceptance means:** the six govern the prototype path now and become the production architecture if the A/B gate passes (mid-December, estimate). They do not switch the shipped bake before that gate.
- **In scope:** the surface, hydrology, the erosion solver, edit ordering, numerics and scale and store budgets.
- **Out of scope:** the renderer's look, materials, cliffs, cities, and generator realism tuning.
- **How to read:** §2-§3 in ten minutes; §4 per ADR (the card first, then the text); §7-§8 for the open points. Numbers come from `docs/gates/ledger.md` with the commit they were read on.

## 2. Context in one page

- **Why a replan (SPIKE-104, SPIKE-105, 2026-11-07/08).** The two-pass bake read different bases in the bake and at runtime (rms 12 m, max 209 m). Its channel hand-over re-dammed 13.3 % of land in pits. The fill depth, channels and regional change were becoming separate runtime height terms. The smallest erosion unit was a whole cube face, too large for the editor.
- **The target (A3, `docs/design/terrain-target.md`).** One authoritative surface `S = B + L + C + A`, with one baked elevation term `L`. Hydrology as persistent objects: H0 planet-wide, H1 per region, channels, basins, lakes. Erosion as rounds over a region, with catchment-local steps and edits below erosion propagating through keys.
- **Route chosen (owner D1, 2026-11-08):** option A, conditional. Build the prototype against the target. At the A/B gate the switch criteria S1-S5 decide between continuing (A) and a new core (B).
- **Where it stands (2026-11-11).** Items 1-11, 13, 14 and 18 of the briefing are done; item 15 (runtime: lakes, rivers, flight) is in progress. Checkpoint 1 (about 2026-11-20): edit locality is measured; a flyable region from stored `L` and the f32 rounds are open. Checkpoint 2 (about 2026-12-01): rivers, lakes, flight frame time. Then the gate.

## 3. Baseline: constitution v1.7 (ADR 0116, accepted 2026-11-11)

The rules these ADRs rest on, in short. The full text is `docs/constitution.md`.

| Rule | In short | Used by |
|---|---|---|
| CON-05 | canonical derived state is reproducible from declared identities and reusable when its whole dependency identity matches | 0112, 0113 |
| CON-06 | pure computations over any domain (planet, region, partition, catchment, tile, point) | 0111, 0112 |
| CON-07 | one selected implementation per authoritative field and identity; the CPU is canonical; prototype and production may coexist under distinct identities | 0110, interim of all six |
| CON-08 | staged with declared dependencies; global, regional, dynamic or partitioned stages; iterative stages may read their earlier rounds; locality is demonstrated, not assumed | 0111, 0112, 0113 |
| CON-14 | bit-exact CPU results across runs, threads, partitionings and Windows/Linux x86-64, for all authoritative state | 0114 |
| CON-28 | authoritative simulation on the CPU only; the GPU derives presentation | 0110, 0114 |
| CON-29 | incremental equals fresh build; reuse only on a full identity match | 0112, 0113 |
| CON-30 | partitioning, order and scheduling never change results | 0112 |
| CON-31 | no weakening of requirements or tests to pass; reports separate run, unrun and inferred results | process |

**Alignment result (architect, 2026-11-11).** No ADR needs a carve-out from the constitution. The only conflict was CON-08's "fixed grid with a bounded halo", and v1.7 removed it. The pass of 2026-11-11 also:
- removed six repeated interim lines (one remains, in 0112);
- replaced 0114's restatement of CON-14 with citations;
- fixed four stale lines;
- moved the build-memory values into the spec (BUDG-016).

## 4. The six ADRs, with their cards

Readiness legend: **accept now**, the decision is founded and what is open is measurement at the gate; **accept with a condition**, a named point must be settled with the acceptance; **wait**, not ready.

### 4.1 ADR 0110: the authoritative surface

| | |
|---|---|
| Decides | one surface `S = B + L + C + A` in f64; one base read for every consumer; `L` is the only baked height term; channels are burned into `L`; the fill depth leaves the runtime once lakes are drawn |
| Well founded | **G-SURF pass** at the cell centres: 5.7e-14 m before quantisation, 0.125 m after (ea6b1b2b), independently re-read 0.0 m (5cc69697). **G-SURF-I** in A3's criteria: 0.66 % of vertices over 0.25 m, p99 0.225 m, the error growing with the step (D9, D35). `L` is stored as kind 13 and measured. The creases are CPU only (no GPU twin, checked 2026-11-11). |
| Open | G-PROFILE (rivers' level profiles) not run; G-BASIN partial; the switch to the stored `L` at runtime (item 15 PR 5a) and the fill-channel deletion (PR 5b) not done |
| Amends | 0088 points 2, 4.2 and 5; 0099 point 3; 0100; replaces 0101 point 3 |
| Readiness | **accept now.** The fill-channel deletion (item 15 PR 5b) waits for this acceptance; it is the one item of work blocked on an ADR |

Full text (`docs/adr/0110-the-authoritative-surface.md`):

#### 0110 The authoritative surface: one composition, one base read, one elevation source
Status: proposed (2026-11-08). Acceptance: the owner's, with SPIKE-105's D2-D10. Production: G-SURF, G-PROFILE, G-BASIN at the A/B gate
Date: 2026-11-08
Source: `docs/design/terrain-target.md` §3; SPIKE-104 §3; SPIKE-105 §6, §11 (points 9A), §13, §18

##### Context
- The bake and the runtime read different bases: pass 2's window takes the base every 4 cells (407 m), the runtime the store's 611 m lattice (SPIKE-104: rms 12 m, max 209 m).
- A C1 read of a D8 channel network re-dams it at the local grid (13.3 % of land in pits, SPIKE-104).
- The fill depth, the channels and the regional change are each candidates for separate runtime terms, each a further elevation source.
- The runtime adds the fill depth back until lake water exists (ADR 0101 point 3).

##### Decision
1. The surface is `S = B + L + C + A`, composed in f64 in that order: the base `B`, the local change `L` (faded at the coast), the creases `C`, the above-erosion items `A`.
2. `B` is the store lattice's C1 read (GEN-107), plus below-erosion height items; it is the same read for every consumer (K4, K5, K7, tiles, CPU systems). Pass 2's stride-4 base is retired.
3. `L` is the only baked elevation term the runtime reads. The regional change, channels and basins are inputs of the bake. `C` is evaluated at read time on the CPU (no GPU twin; CON-07, CON-28) and stores nothing of its own; with the relief switch on, its amplitude reads the baked driver `k` (GEN-099).
4. Channels are burned in at the local grid (K7 lowers cells along a segment's level profile, then breaches, incises and settles); they are not a runtime height term. A later bank detail may lower the ground only, within a declared bound.
5. The runtime no longer adds the fill depth; basins show their floor and the water field draws lakes. The lake producer and its store exist (GEN-163, kind 19); this takes effect when the water source draws them and G-BASIN passes.
6. Every derived layer folds `base_id` into its key; a mismatch is refused. `base_id` covers the lattice key, the C1 read's version and parameters, the mapping, radius and `k`, the sea level, the below-erosion items at the tile, and the stored forms (A3 §3.2).

##### Consequences
- Amends ADR 0088 points 2, 4.2 (pass 2 gains K7's breach and loses its stride-4 base) and 5, ADR 0099 point 3, ADR 0100; replaces ADR 0101 point 3 (the fill-depth channel; its deletion waits for this ADR's acceptance).
- Gates G-SURF, G-CON, G-PROFILE (channels through the complete read).

### 4.2 ADR 0111: the hydrology model and identity

| | |
|---|---|
| Decides | H0 planet-wide, coarse, and constraining nothing; H1 per region with four quantities kept apart (geometric area, effective area `A_d`, discharge `Q`, routed discharge `Q_w`); a persistent channel graph; basins as objects with a water balance; ids as pure functions of the surface; lakes as water producers |
| Well founded | **G-CON pass** on five windows and the k=4 landmass: 0 disconnected, sums at most 1.5e-14 (several commits, independently re-read 5cc69697). Basins and lakes are built: the lake producer at bake time (GEN-163), the basin table (kind 19), the lake shore and water mask design (D18, D20). The units guard: `Q` is a discharge only (D33, #1345). |
| Open | G-BASIN partial (level rows and the high and zero `E_open` cases); point 7's cascade and open-water term in code; nested basins; basin losses in the carve's `A_d` |
| Amends | 0101 points 2 and 3; 0105 points 2 and 3; extends 0041 |
| Readiness | **accept now** |

Full text (`docs/adr/0111-hydrology-model-and-identity.md`):

#### 0111 Hydrology model: H0 and H1, channel graph against diffuse field, basin and segment identity
Status: proposed (2026-11-08). Acceptance: the owner's, with SPIKE-105's D2-D10; code still to follow point 7's cascade and open-water term. Production: G-CON, G-BASIN at the A/B gate
Date: 2026-11-08
Source: `docs/design/terrain-target.md` §5; SPIKE-105 §6, §8 F3, §12 F5-F6, §18, §19; ADR 0041, 0099, 0101

##### Context
- Rivers and basins exist only implicitly: a drainage-area byte (GEN-110), a fill-depth channel (ADR 0101). R4a and R4b need them as persistent objects.
- The carve already uses two models: jittered D8 receivers and MFD area (ADR 0099 point 2); they must not be conflated in the data model.
- A planet-wide route across cube edges costs about 7 s at 3.26 km with heights resident (F6); the largest catchment is 5.9e6 km^2.
- Uniform cell areas overstate drainage area by 18 % on average and change 7-14 % of channel cells (F5).

##### Decision
1. H0: a planet-wide route at the coarse grid across cube edges gives trunk connectivity and the level-1 preview. It constrains H1 in nothing: H1 determines drainage and channels from its own surface and regions from its own land mask (ADR 0112 point 1). H0 does not cut erosion domains.
2. H1 (regional grid), quantities kept apart (`a` true cell area; A3 §5.2): **geometric area** (sum of `a`, m^2); **effective area** `A_d` (MFD accumulation of `a w`, `w` the dimensionless runoff weight; m^2 of reference runoff), which drives the carve and initiation; **potential discharge** `Q` (m^3/s, the D8 accumulation of `a r` over every land cell, so hillslope runoff enters a channel at the first channel cell of its D8 path); **routed discharge** `Q_w` (`Q` after basin losses), which rivers report. `Q` and `Q_w` are discharges in m³/s; an area mode exists for tests only and every consumer refuses it (D33).
3. `A_d` is an f64 of the round. The stored byte (a log code, A3 §5.2) is one-way, for materials; no simulation stage reads it.
4. **The channel graph** (one D8 receiver per channel cell) gives connectivity: a channel continues downstream after initiation to its outlet, also below an endorheic basin (with `Q_w` = 0). It starts at a calibrated preset threshold of effective area (default 50-100 km^2).
5. Basins are objects with floor, spill level, spill point (topographic), active outlet (a hydrological state), area curve, type and water level. Types: artefact (no object, no water), lake, endorheic, intentional; class rules are preset parameters, computed from the final round. The artefact limit is in cells and depth, so that it scales with the grid.
6. Basins are found on the filled surface of the round's routing; touching basins are one component (nested basins are not separated).
7. Balance: `Q_in = A(h) E_open + q_out(h)`, `q_out` zero below the spill level, `E_open = PET - (P - r)`. A basin that cannot balance its inflow overflows at its spill point. Basins resolve upstream to downstream, so `Q_in` reads `Q_w`; all cases are in A3 §5.5.
8. Ids are pure functions of the current surface: a basin by its canonical lowest cell, a segment by its end cells. Continuity across an edit is a separate, non-authoritative map computed at commit.
9. Lakes are producers of the water field (ADR 0041 point 7); the fill depth stops being a stored term.

##### Consequences
- Gates G-CON, G-BASIN. Amends ADR 0101 points 2 and 3 and ADR 0105 points 2 and 3 (an endorheic basin by water balance, not by aridity; the fill-depth channel retired by point 9); extends ADR 0041.
- Rivers and lakes (later authoring) become constraints on K6 inputs, not new stages.
- Open: nested basins; basin losses in the carve's `A_d` (ADR 0105 point 2 is not in the round solver).

### 4.3 ADR 0112: the erosion solver (regions, rounds, catchments)

| | |
|---|---|
| Decides | a region (a land-mask component) is the routing scope; a round is one region-wide routing and its steps; catchments run the breach and sweeps independently on the shared routing; MFD clipped to the catchment; flood ties by (level, cell id); reuse only on bit-equal inputs |
| Well founded | **G-ROUND pass** (0 cells differ, 1 to 8 threads, windows of 1e6 to 44e6 cells, independently re-read). **G-PART pass** (tiled against resident route, 0 differ, a one-cell spiral corridor included). **G-LOCAL pass**, first measured 2026-11-11 by an independent kernel written from the spec: 0 of 1,040,400 cells differ on three regions, two rounds each; key completeness after the memo-key fix (D28, D31). **G-SCALE pass** below the 12e9 B limit: 11,387 MiB (BUDG-016). **G-LAND pass** (0.005-0.059 % unclassified, limit 1 %). |
| Open | G-EQ partial (the f32 round surfaces of 0114 point 4 are not built yet: every pass above is for f64 rounds; GEN-166 in progress). G-INC partial (work 6.5-9.8 % of a fresh build, the routing recomputed whole). G-SED partial (spatial cut of the sediment pass untested). Thermal dams: half of the pits left after the pass are dams the pass makes (D23). The 11e9 B working target is not met (897 MiB over). |
| Amends | replaces 0109 and 0105 point 1 at the gate (deposition moves, not dropped); amends 0088 point 4.1, 0099 points 1 and 2 |
| Readiness | **accept as the prototype architecture now**, with production at the gate as its status line says. The catchment-clipped MFD (point 5) is the decision to look at hardest (§7, O2) |

Full text (`docs/adr/0112-erosion-solver-regions-rounds-catchments.md`):

#### 0112 The erosion solver: regions, rounds and catchments
Status: proposed (2026-11-08). Acceptance: the owner's, with SPIKE-105's D2-D10, as the prototype's architecture; production only after G-LOCAL, G-ROUND, G-PART, G-EQ and G-INC pass on the actual kernels at the A/B gate (D3)
Date: 2026-11-08
Source: `docs/design/terrain-target.md` §6; SPIKE-105 F2, F4, F7, §18; ADR 0088 point 4, 0099

##### Context
- A catchment carved with its own routing differs from the same catchment in a larger carve (F2: rms 5 m; F4: 12.8 m at best); the carve is chaotic (72 m rms between jitter seeds).
- With one routing per round, the breach and sweeps per D8 catchment are bit-identical to the whole-window carve (F7).
- MFD shares and the flood's tie order couple catchments (F7).
- Routing is 85-90 % of a round.
- The smallest erosion unit today is a whole face (about 1.6e8 cells, ADR 0099); it cannot run in the editor (S105 X7, X8).

##### Decision
1. The scope of one routing is a **region**: a connected component of the land mask at the regional grid, bounded by the sea, re-derived at every commit. It is routed whole, resident or partitioned. A split at H0 divides is a tested fallback only.
2. The routing's reference is a definition (A3 §6.8): the flood's unique fixed point, the receiver rule, the area's donor order. Resident and partitioned routes must compute it bit for bit; G-PART proves it.
3. A **round** is one region-wide routing and the steps that follow; rounds continue the surface. The round count is fixed in the `SimulationProfile`.
4. A **catchment** is a D8 tree of a round's routing. The breach and the sweeps run per catchment, any order, on the shared routing. Thermal and pit spreading cross divides and run after the last round: per tile with a halo is the target; today they run on a whole grid and their spatial cut is untested. A later coupling is added to the contract; "independent" is not a permanent limit.
5. MFD is clipped to the catchment (a lower neighbour in another catchment is dropped from the weights and from their sum; the D8 receiver always remains, so no fallback is needed; `A_d` at an outlet is the sum of `a w`). The flood orders visits by (level, global cell id).
6. A re-solve's result is by definition a full region re-route; a full recomputation is always an allowed fallback. Reuse is an optimisation: each step is reused only if all its inputs are bit-equal, including a tile's incoming offers (dependency table, A3 §6.8). No dirty region is predicted (F9).
7. Every value a catchment-round reads is in its key, directly or through an immutable upstream key, in canonical form (A3 §6.5); a catchment's members are visited and hashed in `Routing.order` restricted to the catchment (D31). The same function makes a fresh build (CON-29).

##### Consequences
- On the gate, replaces ADR 0109 and ADR 0105 point 1: deposition is not dropped but moves to the post-round sediment pass (A3 §6.7; the basin infill is replaced through ADR 0111 point 5). Amends ADR 0088 point 4.1 and ADR 0099 points 1 and 2.
- CON-08 v1.7 (ADR 0116) allows the regional solver and its rounds; CON-30 is G-PART's basis.
- Interim, for ADRs 0110-0115 until the A/B gate: the accepted ADRs they amend govern the shipped bake; 0110-0115 govern the prototype path, a distinct implementation identity behind its switches (CON-07; architect D24).
- Gates G-ROUND, G-LOCAL, G-EQ, G-INC, G-SCALE, G-PART. A failure of G-LOCAL or G-EQ is switch criterion S1 or S2 of S105 §13.1. F7 did not test these kernels; passing on prototype fixtures is not passing a gate at scale.

### 4.4 ADR 0113: edit ordering and dependencies

| | |
|---|---|
| Decides | landform and driver edits sit below erosion, constructed features above it; dirtiness below erosion by keys over the routing, not by a radius; propagation ends at bit equality; regions re-derived at commit; one coherent commit per frame |
| Well founded | E1-E7 fixtures run (G-INC, G-BASIN, G-EQ partial); G-LOCAL passes (the locality the keys rely on); the commit record lives in the layer manifest (D34); the pairing of `L` and the basin table under one commit is defined (D17) |
| Open | G-INC on real edits at scale; G-EQ with the f32 forms; the editor's level-2 re-solve and its latency (item 16); undo through earlier keys not yet exercised end to end |
| Amends | 0007 (layer order); 0066 points 1, 2 and 5; 0089 point 11; settles 0086 point 3 |
| Readiness | **accept now.** It changes three accepted ADRs; the owner's own requirement (edits update erosion and hydrology, 2026-11-07) is the reason |

Full text (`docs/adr/0113-edit-ordering-and-dependencies.md`):

#### 0113 Edit ordering and dependencies: landform and driver edits below erosion
Status: proposed (2026-11-08). Acceptance: the owner's, with SPIKE-105's D2-D10 (it changes accepted ADRs 0007, 0066 and 0089). Production: G-EQ, G-INC on E1-E7 at the A/B gate
Date: 2026-11-08
Source: `docs/design/terrain-target.md` §7; SPIKE-105 §3 (X1-X4), §13.5 (D4), §18; ADR 0007, 0066, 0072, 0086

##### Context
- Authored items are stages after the base and the erosion layer; an edit moves heights, but rivers and erosion do not respond (ADR 0007, ADR 0066 point 4).
- Edits under the erosion were deferred to M9 and not taken up (ADR 0086 point 3).
- Reach is a radius (ADR 0066 point 1); a river can change far downstream.
- The owner's P0: edits update erosion and hydrology (R1a, R4b).

##### Decision
1. Landform height items and driver items (lithology, uplift, climate, class paint) sit **below** erosion: applied at K2 and K3; `B` gains a read-time term for them.
2. Constructed features (roads, plots, objects) stay **above** erosion, exact (ADR 0089 point 11).
3. Dirtiness below erosion is found by keys over the routing (ADR 0112 point 6), not by a radius; the radius of ADR 0066 point 1 stays for items above erosion.
4. Propagation ends at bit equality of the canonical stored forms of a catchment-round's inputs, never at a tolerance.
5. An edit that changes the land mask or landmass connectivity re-derives the regions at its commit; all routing and erosion state of an affected region follows through the keys, which hold no region id. A merged region over the job limits goes to a batch re-run. A change of the sea level is a full rebuild.
6. A commit is one coherent set of keys (surface, hydrology, basins, water levels, materials) under a `commit_id`. A frame reads one commit; the runtime draws the previous commit until the new one's drawn tiles are ready (A3 §7.4; CON-22: never blocking); a superseded result is dropped. Per-tile refresh (ADR 0072) stays for items above erosion.
7. H0 and the regional solver are stages over global and regional domains that read items, and the rounds of one solve are an iterative stage with a fixed count (CON-08 v1.7, ADR 0116). Point 4's bit equality is CON-29's rule.

##### Consequences
- Amends ADR 0007 (layer order), ADR 0066 points 1, 2 and 5 (items below erosion are bounded by keys, not by footprint or reach; point 4 is already amended by ADR 0068) and ADR 0089 point 11 (height edits leave the override stage); settles ADR 0086 point 3. Point 6 is new against ADR 0072 (runtime and editor work).
- Undo removes an item: earlier keys read back.
- Gates G-INC, G-EQ, on E1-E7 (E7: land bridge joined, then separated).

### 4.5 ADR 0114: simulation profile, cell metric and canonical forms

| | |
|---|---|
| Decides | one immutable `SimulationProfile` per solve; a true per-cell metric (area, distance); three representations kept apart, each value with one canonical stored form; determinism by named controls shown on the actual builds |
| Well founded | the true metric is built (GEN-138); G-DET 1 against 2, 5 and 8 threads, 0 cells differ (independently re-read) |
| Open | the `SimulationProfile` type does not exist yet (its values are in `CarveParams` and `PlanParams`); the f32 round surfaces are not built (GEN-166); G-DET Windows against Linux not measured for the new chain; point 6, the toolchain pin, is open |
| Amends | extends 0010; uses 0003's mapping |
| Readiness | **accept with a condition: decide point 6 with it.** The architect's recommendation is to pin the toolchain (`rust-toolchain.toml`) before the world build. CON-14 v1.7 requires determinism shown on the supported builds, and an unpinned `stable` moves that target with each release |

Full text (`docs/adr/0114-simulation-profile-cell-metric-and-canonical-forms.md`):

#### 0114 Simulation profile, cell metric and canonical forms
Status: proposed (2026-11-08). Acceptance: the owner's, with SPIKE-105's D2-D10 and point 6 (the toolchain pin). Production: G-EQ (the f32 form) and G-DET on both operating systems at the A/B gate
Date: 2026-11-08
Source: `docs/design/terrain-target.md` §4.2, §8, §9; SPIKE-105 F5, §11 (points 3, 9B, 9C), §13.5 (D9, D10), §18; ADR 0010

##### Context
- An edit must not advance erosion by an arbitrary amount of time; simulated time and editor time are separate.
- A uniform cell area and distance bias drainage area by 18 % on average, river initiation by 7-14 % of channel cells and slopes up to 1.5x at corners (F5).
- A re-solve and a fresh build must agree bit for bit (CON-14); reading in-memory f64 in one path and stored f32 in the other breaks that (`bake/tile.rs:322`).
- A chaotic carve has no pointwise reference; correctness is structural.

##### Decision
1. A `SimulationProfile` in the WorldDefinition holds every parameter that can change a result: rounds, steps per round, `K dt`, quality, process and routing parameters, the seed policy and the quantisation constants (A3 §8). It is immutable for a solve; simulated duration is rounds x steps x `K dt`. Every solve starts from the definition.
2. Every grid exposes true per-cell area and neighbour distance from the mapping; the uniform `dx` is retired; constants that read area are re-tuned once.
3. Three representations are kept apart: the canonical persistent form, the stage computation, the runtime composition (f64). Each value's forms and its single conversion point are in A3 §9's table.
4. Each value one stage passes to another has one canonical stored form; both a fresh run and a re-solve read that form. Round surfaces are f32 on the regional grid, narrowed once at the end of a round; a kernel computes in f64.
5. Determinism is CON-14, CON-28 and CON-30. This ADR adds: each source of divergence has a named control (A3 §9), shown by the actual builds (G-DET), not inferred; ties break by global cell id.
6. Open, proposed for the owner: a toolchain pin before the world build; CI uses `stable`.
7. Reproducibility (bit equality) and correctness (connectivity, conservation, seams, locality, relief) are separate gates.

##### Consequences
- Extends ADR 0010; uses ADR 0003's mapping. Changes erosion constants through the metric (D9).
- Gates G-EQ, G-DET. CI's determinism manifest must add a fixture of the round solver and H1.
- Point 4's f32 round surfaces are not yet built (main is f64 across rounds); GEN-166 builds them (architect D24, D27).

### 4.6 ADR 0115: scale as a parameter and the store budget

| | |
|---|---|
| Decides | grid counts from one integer `k` per body; physical, geological and hydrological parameters owned separately; hydrology needs an outlet; a 40 GB store gate on real data; a separate world-build memory budget; evictable caches |
| Well founded | **G-STORE pass** on steady state: worst sum 28.4 GB against 35 GB target and 40 GB hard, `L` measured on two regions, the four smaller A3 §10 items bounded (6b5bb0ac). **G-SCALE pass** below the hard limit (11,387 MiB worst of two runs, ee8347b7). |
| Open | the 11e9 B working target (owner, 2026-11-11) is 897 MiB away; the levers are the f32 storage of the round heights (GEN-166 PR 2) and the cleanup inside hydrology. The compaction peak is 2x the steady store (26-48 GB, upper 57 GB), reported, not gated. One large and one small body against the scaling model are not run |
| Amends | supersedes 0045 point 2 |
| Readiness | **accept now** |

Full text (`docs/adr/0115-scale-as-a-parameter-and-the-store-budget.md`):

#### 0115 Scale as a parameter and the store budget
Status: proposed (2026-11-08). Acceptance: the owner's, with SPIKE-105's D2-D10; point 7's budget set by the owner (2026-11-08, 2026-11-09). Production: G-STORE and G-SCALE measured at the A/B gate
Date: 2026-11-08
Source: `docs/design/terrain-target.md` §4, §10, §12; SPIKE-105 §3 (X11-X13), §13.5 (D6, D7), §16, §18; ADR 0045 point 2, ADR 0108

##### Context
- Grids are fixed counts per cube-face edge (`BakeShape::production`); the planet's scale is implicit.
- R5a: moon to larger than Earth in the engine; R7: about 40 GB at Earth, larger planets may use simpler features.
- At Earth the store is about 36 GB before hydrology (field bake 11.7 GB, layer and channels about 24 GB).

##### Decision
1. Grid counts are derived from one integer per body, `k`, chosen from the target local cell size: base lattice `2^k`, local `6 x 2^k`, regional `3 x 2^(k-2)`, H0 `3 x 2^(k-4)`. Earth at `k` = 14 is today's set.
2. All are named parameters of the world definition (ADR 0108). The body owns its physical parameters (radius, gravity, rotation, tilt, atmosphere, sea level); a geology preset owns K, U, lithology and class rules; hydrology and climate own runoff and evaporation. A preset supplies defaults. Radius sets no geology.
3. K4 to K7 run when the body's hydrology switch is on, skip when it is off. Hydrology on needs an outlet: in the MVP the sea. A body with runoff and no sea is refused with an error (closed basins as outlets are post-MVP); a body with a sea and no runoff switches hydrology off.
4. The 40 GB limit is a hard gate at Earth on real serialized data, including indexes, keys, graph metadata and edit overrides; peak temporary storage, replacement-commit space and retained versions are reported separately.
5. Field narrowing comes first; a coarser local grid (about 200 m) is a fallback only.
6. Larger planets take coarser cells at the same counts (2x Earth at `k` = 14, 204 m), unless given a store budget of their own. A scaling model in `k` and radius estimates cells, store and build memory (A3 §4.3).
7. A world build has its own memory budget, separate from CON-21's runtime 8 GB: a hard limit and a lower working target that keeps headroom (owner, 2026-11-08 and 2026-11-11), both read as peak commit charge of the complete build on the reference machine; the values are in BUDG-016. The compact resident route is the normal path (owner, 2026-11-09); the partitioned route stays a tested equal. A tile-wise route is prototyped only if the unavoidable working set exceeds the budget after retention and caches are removed.
8. Caches are evictable; eviction never changes a result or breaks undo and replay (owner, 2026-11-09; A3 §6.6).

##### Consequences
- Supersedes ADR 0045 point 2 (a ceil on the bake cell) with point 1's rule (the largest `k` whose local cell is at least the target; Earth 101.8 m at `k` = 14). Gates G-STORE, G-SCALE (one large and one small body).
- Point 7 is a new budget; the owner decides it with this ADR.
- The resident rounds of the largest landmass are measured (7.7 GiB, 55 bytes a cell, on a compact grid of the landmass, A3 §6.6); the partitioned route measured more memory and time as built (8,854 MiB, 2,974 s against 7,858 MiB, 1,716 s), so the 2026-11-08 choice of it as the normal path, made on the assumption that it bounds memory, is replaced.
- Since measured: the complete resident build at k=4, 11,387 MiB worst of two runs (GEN-158; ledger G-SCALE).

### 4.7 ADR 0116: constitution v1.7 (accepted, for reference)

Full text (`docs/adr/0116-constitution-v1-7-simulation-architecture.md`):

#### 0116 Constitution v1.7: rules for the simulation architecture
Status: accepted (owner, 2026-11-11: "adopt as suggested", from an external review of constitution v1.6)
Date: 2026-11-11
Source: external review of constitution v1.6 (owner, 2026-11-11; the PDF is not committed); spikes' ADR review pass (`spikes/prototype-gates/results/adr-review-pass.md` §1 row 1); A3 (`docs/design/terrain-target.md`); ADRs 0077, 0079, 0110-0115

##### Context
- The constitution was written for a staged, tile-oriented generator. The prototype computes globally (H0), per landmass region (H1, K5 rounds), per catchment and per partition, iterates rounds and re-solves after edits.
- CON-08 required "a fixed grid with a bounded halo"; ADR 0113 point 7 called the regional solver a reading of it. A reading cannot supply a bounded halo (ADR review pass, row 1).
- CON-06 named `tile_id` as the computation identity; CON-07 and CON-14 implied GPU parity for authoritative fields, although the design is CPU-authoritative.
- Fresh/incremental equivalence and partition invariance were tested (G-EQ, G-PART, G-INC) but had no constitutional basis.
- The constitution takes precedence over accepted ADRs, so ADRs 0110-0115 could not be accepted while they conflicted with it.

##### Decision
1. CON-05: canonical derived state is reproducible from its declared identities and may be reused when its complete dependency identity matches.
2. CON-06: the computation identity is general (planet, region, partition, catchment, tile, point); purity does not imply spatial locality.
3. CON-07: one selected implementation per authoritative field per cache identity; the CPU is canonical; versioned preview, production and reference implementations are allowed under distinct identities.
4. CON-08: stages declare dependencies and may run globally, regionally, over dynamic domains or partitions; iterative stages may read their own earlier iterations; spatial locality is demonstrated, not assumed.
5. CON-14: bit-exact CPU determinism covers all authoritative state and partitionings; the mechanism (`libm`) is policy, the outcome is the rule; parallel reductions are allowed when the result is scheduling-independent.
6. Added: CON-28 (authoritative simulation is CPU-only), CON-29 (incremental equals fresh build), CON-30 (partitioning does not change results), CON-31 (no weakening of requirements or tests to pass; reports separate run, unrun, unwritten and inferred results).
7. Refined: CON-04, CON-11, CON-13, CON-15, CON-18 to CON-23, CON-25 to CON-27 as in the review (runtime against build memory, VRAM named, adapter where relevant, ADR errata records, scripts as implementation details, Linux CI runs the determinism tests, provenance for code, assets and data).
8. The constitution's size cap (ADR 0079, BUILD-023) becomes 12,817 bytes, its size after this amendment.

##### Consequences
- ADRs 0110-0115 no longer conflict with CON-08; ADR 0113 point 7's "reading" is replaced by a citation of CON-08 v1.7 in the architect's text pass (P0). Their acceptance stays with the owner.
- G-EQ, G-PART, G-DET and G-INC now verify CON-29 and CON-30; requirements cite them.
- CON-25 no longer forbids every script; `CLAUDE.md` keeps its stricter rule (no `.sh` or `.ps1` files) until the owner changes it.
- Not in the constitution, by the review's advice: hydrology architecture, routing choices, f32/f64 conversion points, exact budgets; they stay in ADRs and specs.

## 5. Cross-cutting assessment

**How the six fit together** (A3's stage chain, simplified):

| Stage | What | Governed by |
|---|---|---|
| K1-K3 | definition, profile, base `B` with below-erosion items | 0110 points 1-2, 0113 point 1, 0114 point 1 |
| K4 | H0, planet-wide routing | 0111 point 1 |
| K5 | rounds over a region: routing, catchment breach and sweeps | 0112 |
| K6 | H1: channels, basins, `Q_w`, lakes | 0111 points 2-9 |
| K7 | local pass: burn-in with the width law, breach, carve, sediment, quantised to `L` | 0110 point 4, 0114 point 4 |
| K8-K11 | store, runtime composition, streaming, water | 0110 points 3 and 5, 0113 point 6, 0115 |

- **One elevation source** (0110) runs through every stage. Hydrology objects (0111) are inputs of the bake, not runtime terms. The solver (0112) and the edits (0113) share one dependency model: keys over the routing, reuse only on bit equality (CON-29). The numerics (0114) give every value one stored form. The budgets (0115) bound it all.
- **No duplication left.** Each rule lives in one place: the constitution states principles, the ADRs decide, A3 holds the contract tables, the specs hold the numbers (BUDG-016, the gate rows).
- **Interim state** is stated once (0112): until the A/B gate, the shipped bake follows the accepted ADRs; the prototype follows 0110-0115 under its own implementation identity (CON-07).

## 6. Evidence table (gate ledger, newest reading per gate)

| Gate | Verdict | Commit | Independent re-read | What it shows |
|---|---|---|---|---|
| G-ROUND | pass | 5cc69697 | yes (spikes) | region solver = whole-region carve, 0 cells differ, 1-8 threads |
| G-PART | pass | 5cc69697 | yes | tiled routing = resident routing, 0 differ |
| G-LOCAL | pass (key completeness after D28) | 5cc69697 | yes, first reading | a catchment round is a pure function of its key |
| G-CON | pass | 5cc69697 | yes | connectivity and conservation, at most 3.0e-14 |
| G-LAND | pass | e4c03141 and item 13 | yes | unclassified pits 0.005-0.059 % (limit 1 %) |
| G-SURF | pass | 5cc69697 | yes | runtime = bake at the cell centres |
| G-SURF-I | reported, in criteria | ea6b1b2b | yes (D35) | vertex error grows with the step |
| G-SCALE | pass below 12e9 B; target 11e9 B open | ee8347b7 | no | 11,387 MiB worst of two runs |
| G-STORE | pass (steady state) | 6b5bb0ac | no | 28.4 GB worst against 35 GB |
| G-DET | partial | 5cc69697 | yes | threads equal; Windows/Linux for the new chain open |
| G-EQ | partial | f6adbdf1 | no | f64 only; f32 forms open (GEN-166) |
| G-INC | partial | f6adbdf1 | no | 6.5-9.8 % of a fresh build's floods |
| G-SED | partial | 86c8422b | no | balance and catchment groups exact; spatial cut open |
| G-BASIN | partial | f6adbdf1 | no | E3 fixtures; level rows open |
| G-PROFILE | not run | - | - | waits for rivers (item 15) |
| G-RT | not run | - | - | waits for the flyable region (CP1) and rivers (CP2) |

## 7. Known risks and open decisions

| # | Point | Size | Who decides |
|---|---|---|---|
| O1 | **SPIKE-105's D2-D10 are not recorded as decided.** Every status line names them; only D1 is recorded. The work has followed the recommendations for D2-D7 (bake frozen, domains tested, edits below erosion, persistent rivers, store as a gate, scale now). | S to confirm | owner, in one pass with the acceptance |
| O2 | **Catchment-clipped MFD (0112 point 5)** changes the algorithm to make catchments independent. It is declared and measured (G-LOCAL passes; the clip changes 53-209 catchments of a region against unclipped flow), but its effect on landform realism near divides is not judged. | M | reviewer, then owner |
| O3 | **Toolchain pin (0114 point 6).** Recommendation: pin before the world build. | S | owner |
| O4 | **f32 round surfaces not built (0114 point 4).** All solver gates so far hold for f64 rounds; GEN-166 builds f32 and re-runs G-ROUND, G-PART, G-DET and G-EQ. | M | measurement (CP1) |
| O5 | **Thermal dams.** About half of the pits left after the sediment pass (0.35 % of land at k=4) are dams the pass makes, mostly thermal collapse into the carve's one-cell canyons, up to 824 m deep, growing with k. G-LAND still passes. | M | options brief to the owner after the k=2 count |
| O6 | **Memory headroom.** 57 MiB under the hard limit, 897 MiB over the 11e9 B target. The levers are named (f32 storage, hydrology cleanup). | M | measurement; owner at the gate if unmet |
| O7 | **Compaction peak.** The store needs 2x its steady size (up to 57 GB) during compaction; pack-wise compaction is the lever, not ordered. | M | owner at the A/B gate |
| O8 | **G-RT frame time** is unmeasured; it is the largest unknown for the date. | L | measurement (CP1 early, CP2 full) |

## 8. Questions for the external reviewer

1. Is a region (a land-mask component) the right routing scope, with catchments as the unit of independent steps? Or does a different decomposition serve edits better?
2. Is catchment-clipped MFD (0112 point 5) an acceptable price for catchment independence? What would you measure to judge its effect near divides?
3. Rounds with a fixed count over a region (0112 point 3): does continuing the surface across rounds risk drift or oscillation that the gates would not catch?
4. Is ending propagation at bit equality of canonical stored forms (0113 point 4, CON-29) practical once the f32 forms exist, or will near-equal results defeat reuse?
5. Basin identity by the canonical lowest cell and segment identity by end cells (0111 point 8): robust enough across edits, with continuity as a separate non-authoritative map?
6. The store and memory budgets (0115, BUDG-016): is 40 GB at Earth, with an 11e9 B build target, a sound frame, and is the compaction peak a design problem or an operations one?
7. Is anything missing that the A/B gate should test before production, given these ADRs?

## 9. Appendix: where to look

- A3, the contract: `docs/design/terrain-target.md`; the gate table is §12.1.
- The gate ledger: `docs/gates/ledger.md`.
- The briefing with phases and items: `docs/briefings/terrain-prototype.md`.
- The ADR review pass by spikes: `spikes/prototype-gates/results/adr-review-pass.md`; the repository audit: `spikes/repo-audit/results/audit.md`.
- The replan: `spikes/architecture-replan/results/results.md` (SPIKE-105; D1-D10 in §13.5).
- The architect's decisions of 2026-11-10/11 (D1-D38) live in `C:/DEV/planner-inbox/architect-decisions.md`, outside the repository. Those that changed the contract are carried into A3, the ADRs and the briefing's contract-change lines.
