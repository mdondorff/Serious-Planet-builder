# Terrain and hydrology: target architecture and data contracts (A3)

Status: proposed (spikes session for opus-planner, 2026-11-08; inbox #100). D1 = option A, conditional (owner, 2026-11-08). D2 to D10 of SPIKE-105 §13.5 are working assumptions at their recommended options. This document goes to one external review round before the prototype builds on it (SPIKE-105 §14).
Source: `spikes/architecture-replan/results/results.md` (SPIKE-105; "§" numbers below without a file name are of this document, "S105 §" of that file). Evidence: S105 §8 and §12 (F1-F6), SPIKE-104.
Scope: the terrain and hydrology from the world definition to the runtime surface. Rendering, streaming, keys and the store format stay unless a section says otherwise.
Reading rule: every element is marked **E** (exists as needed), **X** (exists, extended) or **N** (new). A statement that the prototype must confirm is marked **[gate]** with the gate of §12.

## 1. Requirements and non-goals

| # | Requirement (owner, 2026-11-07) | Priority | Section |
|---|---|---|---|
| R1a | Edits update erosion and hydrology; the author edits in the editor before shipping | P0 | §7 |
| R1b | A sandbox game with limited player edits may forbid "too global" edits | consideration | §7.5 |
| R2 | Every edit kind has a quick preview; global edits fall back to a low-res preview, then an on-demand re-bake with a longer delay; regional edits include climate, lithology and other properties | P0 | §11 |
| R3 | The shipped product is realtime without stutter on Rasierklinge | P0 | §11.3 |
| R4a | Hydrology is a first-class output (rivers, basins) | P0 | §5 |
| R4b | Rivers change after an edit | P0 | §7 |
| R4c | Seasons and floods | post-MVP | not blocked by §5 |
| R4d | Authoring rivers and lakes directly | post-MVP; the architecture allows it | §5.6 |
| R5a | Planet sizes from moon to larger than Earth (engine); MVP one planet | P0 engine | §4 |
| R5b | A simpler generator may serve moons | consideration | §4.3 |
| R6 | Plausible and consistent wins over close to Earth measurements | P0 (decision rule) | §12 |
| R7 | About 40 GB at most for an Earth-like planet, lower is better | P0 | §10 |
| R8 | The M9 bake may be delayed; little owner attention | P0 (process) | §12.3 |

Non-goals (MVP): fluid simulation; erosion evolving live under the brush (R2 asks for preview, then re-bake); seasons, floods and sediment dynamics in time; meanders below the local grid; overhangs (ADR 0091); GPU participation in any authoritative result (CON-07, CON-14).

## 2. Components

Dependency direction follows the list; §6 defines the one feedback loop (K5, K6 and H1 inside a world-build run).

| # | Component | Owns (authoritative) | Reads | Mark | Today |
|---|---|---|---|---|---|
| K1 | Planetary state | WorldDefinition: seed, body parameters, preset (ADR 0108), target cell sizes (§4), `SimulationProfile` (§8), authored items (§7.1) | - | **X** | WorldDefinition (CON-05), items (ADR 0007, 0066) |
| K2 | Drivers | plate provinces, lithology K(x, z), uplift U, class weights, wind (analytic); precipitation and temperature (field-baked) | K1 and driver items | **X** | GEN-094 to 098, 116, 117, 123, 128 |
| K3 | Base field | base elevation on the store's lattice | K1, K2, height items below erosion (§7.1) | **X** | field bake (ADR 0026), C1 read (GEN-107) |
| K4 | H0 coarse hydrology | planet-wide drainage regions at the coarse grid across cube edges | K3 | **N** | F6's probe; the coupled run of ADR 0077 was a predecessor |
| K5 | Regional geomorphology | per domain: the erosion carve at the regional grid (§6) | K2, K3, K4, K6 of the previous round | **X** | pass 1 (ADR 0088, 0099, 0102-0104); the kernels stay |
| K6 | H1 regional hydrology | the channel graph, the diffuse-area field, basins, lakes (§5) | the K5 surface | **N** (graph, lakes); **X** (basins from the spill graph) | GEN-110, GEN-121, ADR 0101 |
| K7 | Local refinement | per tile, the local grid: breach, incise, thermal, deposition, channels burned in (§3.3) | K3, K5, K6 | **X** | pass 2 (GEN-101) |
| K8 | Authoritative surface | the composition and the sampler (§3) | K3, K5, K7, creases | **X** | runtime field (GEN-099, 112) |
| K9 | Dependencies and versions | keys, reach along the graph, transactions (§7) | all | **X** | keys (ADR 0006, 0066), stale pre-emption (ADR 0072) |
| K10 | Editor | item commands, undo, previews (§11) | K1, K9 | **X** | items, session undo (ADR 0072) |
| K11 | Runtime | streaming, LOD, render, materials, water from K6 | K8, K6 | **X** | CON-03, CON-22 |

Domains and regions are simulation units, not render units: render LOD and tiles never select or drive a simulation (ADR 0014, ADR 0086).

## 3. The authoritative surface

### 3.1 Composition

The surface at a direction `d` is one function, evaluated in this order. Every consumer (K4, K5, K7, tiles, collision, picking) uses it or a defined earlier term of it.

| Step | Term | Definition | Read | Quantisation |
|---|---|---|---|---|
| 1 | `B(d)` | the base: the store lattice's C1 read (GEN-107), clamped as today | `read_elevation_c1`, pure in `d` | stored f32 per lattice node |
| 2 | `L(d)` | the local change: what the bake added against `B` at the local grid, faded at the coast | C1 read of kind 13, clamped to the four cells (GEN-112), times `coast_fade(B)` (GEN-113) | stored i16 in steps of 0.25 m (`QUANT_STEP_M`) |
| 3 | `C(d)` | the creases: octaves below the local grid (GEN-109) | analytic | none (CPU f64) |
| 4 | `A(d)` | items above erosion (§7.1): constructed features | item stages (ADR 0066) | as today |

- Surface: `S = B + L + C + A`, composed in f64, rounded to f32 only at the mesh (CON-13).
- **One elevation source:** nothing else adds height. The regional change, the channels and the fill depth are inputs of the bake, not runtime terms (§3.3).
- Step 2 replaces today's `change + fill depth` (GEN-112, ADR 0101 point 3): the runtime no longer adds the fill depth back. A basin shows its floor and the water field draws its lake (§5.5).
- Presentation data (ground detail, normals) follows CON-14 and adds no height.

### 3.2 Identity

- The base lattice is one object per body: `base_id` = its bake key (ADR 0006). The bake reads `B` through the same read as the runtime: pass 2's stride-4 window (`sample_window_coarse`) is retired (SPIKE-104 D-B).
- Every derived layer (K5, K6, K7, channels) folds `base_id` into its key, so a layer can only be read against the base it was taken on. **X** (GEN-114 folds the bake key today; the base's read becomes part of the identity).
- A mismatch is refused, naming both keys (STRM-065 behaviour). No silent application.

### 3.3 What the bake passes to the runtime

| Bake product | Runtime term? | Use |
|---|---|---|
| K5's regional change | no | input of K7 (and of re-solves); a bake record (§10) |
| K6's channel centrelines and levels | no | input of K7: burned in (below); drawn as rivers by K11 |
| K7's total change against `B` | yes: `L` | the surface |
| Fill depth | no | retired as a surface term; the basin object replaces it (§5.3) |

- **Burn-in rule (K7):** K7 starts from `B + C1(ΔR)` (the regional change read C1). It lowers cells along each channel segment to the segment's level profile (rasterised, 8-connected, along the segment's D8 path at the local grid), then runs the breach (SPIKE-104 D-A: pass 1's rule at the local grid), the incising carve, thermal and deposition. It only lowers. Channels are thus part of `L`, not a separate term.
- **The hand-over rule:** a C1 read must not be used to transfer a channel network between grids: it re-dams diagonal links (SPIKE-104 §3, 13.3 % of land). Burn-in plus breach is the transfer **[gate G-CON]**.
- **Below the local grid:** a bank profile may be added later as a lower-only term with a declared bound, in the place of creases (GEN-109); not in the MVP.

## 4. Grids, metrics and scale

### 4.1 Grids

One free integer per body, `k`, sets every grid (counts per cube-face edge). Earth at `k` = 14 is today's set.

| Grid | Count | Earth cell | Role |
|---|---|---|---|
| Base lattice | `2^k` | 611 m | `B` |
| Local grid | `6 x 2^k` | 102 m | K7, `L` |
| Regional grid | `3 x 2^(k-2)` | 814 m | K5, K6 |
| H0 grid | `3 x 2^(k-4)` | 3.26 km | K4 |

- `k` follows from the body's target local cell size: the largest `k` whose local cell is at least the target (ADR 0045 point 2 made normative). The grid counts are never constants of the code (X11 of S105 §3).
- Nesting: the base lattice's nodes are every 6th local cell corner; a regional cell is 8 local cells; an H0 cell is 4 regional cells.

### 4.2 The cell metric

- Every grid exposes `area(i, j)` and `distance(i, j, dir)` from the mapping (tangent warp, ADR 0003): area by the exact spherical quad (S105 F5: the sum matches the face area to 3e-14), distance by the great-circle between cell centres. **N** (`Grid` today has one `dx`).
- Routing, MFD area, slopes, `f = K dt sqrt(A) / d`, river thresholds and basin areas read the metric. The uniform `dx` is retired. Effect today: areas are overstated by about 18 % on average (F5), so `K dt` and the area-based constants are re-tuned once.
- Cross-face neighbours come from one function over directions (F6), the same everywhere; it is symmetric, including at cube corners, by rule: a cell's neighbours are computed once and the relation is symmetrised by canonical pairs **[gate G-CON]**.

### 4.3 Scale (design-level)

| Body | Radius | `k` | Local cell | Land share (assumed) | `B + L + hydrology` store | H0 (F6 scaled) | Notes |
|---|---|---|---|---|---|---|---|
| Moon | 1,737 km | 12 | about 111 m | no runoff | `B` + stamps only | skipped | no sea, no runoff: K4 to K7 skip (R5b) |
| Earth | 6,371 km | 14 | 102 m | 29 % | see §10 | about 7 s | target |
| 2x Earth | 12,742 km | 14 | 204 m | 29 % | as Earth (same counts) | as Earth | the larger planet takes coarser cells so the store stays inside R7; the local detail comes from creases |
| 2x Earth at 102 m | 12,742 km | 15 | 102 m | 29 % | about 4x Earth: over R7 | about 4x | allowed only with a store budget of its own |

- The preset holds what scale does not: gravity, rotation, tilt, atmosphere, K, U, thresholds (ADR 0108); a radius alone sets no geology.
- Climate at scale: orography lattice counts follow the same `k` rule; the preset's winds and tilt (S105 X12).
- Terrain, hydrology and store for the 2x case are checked as a design table here and one 2x H0 run in the prototype (S105 §13.3).

## 5. The hydrology model

Two levels and two representations. The levels are H0 (planet-wide, coarse) and H1 (regional grid). The representations must not be conflated: the **channel graph** (a tree of discrete rivers) and the **diffuse field** (a raster of accumulated area).

### 5.1 H0: the coarse planet-wide route (K4) **N**

- Grid: H0 cells (3.26 km at Earth), all six faces as one graph, neighbours across cube edges (§4.2), one priority flood from the sea, D8 receivers, area over true cell areas. F6: 7 s with heights in memory, 2.2 GB.
- Output: **regions** (§6.1), the planet's catchment structure at this grid, and trunk connectivity for the level-1 preview (§11).
- H0 does **not** cut erosion domains (F4: 105 m rms and a seam).

### 5.2 H1: the channel graph and the diffuse field (K6)

| Representation | What | Grid | Stored | Mark |
|---|---|---|---|---|
| **Diffuse field** | accumulated area by MFD (Freeman, exponent 1.1), weighted by the true cell area and runoff; the magnitude at every land cell | regional grid | one byte a cell (GEN-110's code) | **X** |
| **Channel graph** | one D8 receiver per channel cell (the routing of the filled surface, jitter from the global cell id); channel cells are those whose diffuse value reaches the initiation threshold (below) | regional grid | as segments only (§5.4) | **N** |
| Receivers elsewhere (hillslopes) | scratch of a solve | regional grid | not stored; recomputed by a re-solve | - |

Rules:
- **Connectivity comes from D8; magnitude comes from MFD.** A segment's discharge at a node is the diffuse value at the node. Downstream of a confluence the graph's connectivity is authoritative: the diffuse value is made non-decreasing along each segment by a running maximum, so a discharge cannot fall downstream where MFD diverges.
- **River initiation:** a channel cell has `A_true,runoff >= T`, with `T` a preset parameter (default 50-100 km^2, D5). Because the area is runoff-weighted, a rainfall edit moves initiation with no height change (E5).
- A channel's heads are cells whose donors are all below `T`. Confluences have two or more channel donors. Outlets are sea cells, region border cells (§6.1) or basin spills (§5.3).

### 5.3 Basins

A basin is a connected set of cells (8-neighbour) below the routing surface's spill level of more than `pit_min_m` (0.5 m), found by the spill graph (`bake/spill.rs`, GEN-121). **X** Stored as one object per basin above a size threshold, on the regional grid:

| Field | Meaning |
|---|---|
| id | canonical identity (§5.4) |
| floor level | lowest cell height (canonical form, §9) |
| spill level | the spill graph's level |
| outlet | the cell through which it drains, or none (endorheic) |
| area curve | surface area and volume at 16 fixed level steps between floor and spill |
| type | one of the classes below |
| water level | the lake's state (§5.5) |

| Class | Rule (presets, ADR 0108) | Behaviour |
|---|---|---|
| artefact | below `min_area_km2` and shallower than `min_depth_m` | breached in K5/K7 (no object) |
| lake | deeper or larger, with an outlet | kept; lake at the spill level |
| endorheic | aridity under the threshold (ADR 0105 point 2) | kept; no outlet; sink for water and sediment; water level from the water balance; playa if dry |
| intentional | named by an item (§5.6) or a driver (caldera, karst later) | kept whatever its size |

- Thresholds are preset parameters; a basin's class is a pure function of the surface and the drivers.

### 5.4 Identity under change

Identities are **pure functions of the current surface**, so a from-scratch build of the edited definition yields the same ids (§9). Continuity across an edit is a separate, non-authoritative map.

| Object | Id | Definition |
|---|---|---|
| Basin | `basin_id = hash(body, canonical cell)` | canonical cell: the lowest cell of the basin, ties by the global cell id |
| River segment | `segment_id = hash(body, downstream end cell, upstream end cell)` | ends are confluences, heads, outlets; cell ids on the regional grid |
| Region, domain | by their outlet set | scratch ids of a solve, not persisted beyond it |

| Event | Effect on ids | Effect on the continuity map (at the commit of an edit) |
|---|---|---|
| Two basins merge | one canonical cell survives (the deeper); the other id disappears | old ids map to the merged id |
| A basin splits | new canonical cells; the deepest part keeps the old id if its lowest cell is unchanged | old id maps to the parts by overlap |
| A breach gives a basin an outlet, or the outlet moves | the id stays while the lowest cell stays; the class may change | - |
| A confluence moves | the segments at it get new ids | old segments map to new ones by overlapping cells |

- The continuity map (old id to new ids by cell overlap, in canonical order) is computed when an edit commits. It is written beside the edit (K9), used by previews and later by game references. It is not an input of any stage, so it cannot change a result.
- A consumer that needs a stable anchor (a bridge, a save game) anchors by a position and re-finds the object, and follows the continuity map when present.

### 5.5 Lakes

- A lake is a basin with a water level. Producer of the water field W (ADR 0041 point 7's retrofit path): level = spill level for class lake, from the water balance for endorheic. **X**
- The fill depth stops being a surface term (§3.1) and no longer a stored channel; it is the basin's spill level minus the ground, derived from the basin object at the local grid.
- Water surfaces render through ADR 0041's path; a basin's area curve answers "what is flooded at level x" without a raster.

### 5.6 Authoring rivers and lakes (post-MVP; not blocked)

A later item kind sets constraints on K6's input (a basin of class intentional, a fixed channel), keyed like any item below erosion (§7.1).

## 6. The erosion solver: regions, catchments and rounds

The reviewers asked for the outer iteration's contract. Evidence: S105 F2 and F4 (a catchment carved with its own routing differs by 12.8 m rms at best) and F7 below. The contract replaces "watershed domains" as first proposed (S105 §6, §13.4).

### 6.1 Scopes

| Unit | Definition | Lifetime |
|---|---|---|
| **Region** | a connected landmass with its sea outlets, from H0. Border = the sea: no artificial cut. A landmass larger than the solver's memory is split at H0 divides, as a fallback **[gate G-SCALE]** | the world build's scope of one routing |
| **Round** | one routing of the region's surface and the carve steps that follow on it; today's re-route interval (5 steps) | a fixed part of the `SimulationProfile` (§8) |
| **Catchment** | one D8 tree of the round's routing (all cells that drain through one outlet) | one round |

### 6.2 One round (the solver's contract)

Given the region's surface at the start of the round:
1. **Route once, region-wide:** flood, jittered D8 receivers, MFD area over the true cell metric (§4.2), runoff weighted, with the §6.4 rules. Cost: about 85-90 % of a round (F7).
2. Partition the cells into catchments by the receiver chains.
3. **Per catchment, in any order and on any thread:** the breach and the implicit sweeps of the round's steps, on the shared routing.
4. Assemble. The next round routes the new surface.

**Exactness (F7, measured):** a round computed this way is bit-identical to the whole-window carve (0 cells differ, forward, reverse and 4-thread catchment orders, F2's window): inside a round a sweep touches a cell and its receiver only, and the area is computed once globally.

Answers to the reviewers' five questions:
| Question | Answer |
|---|---|
| What is iterated? | A continued surface: each round starts from the previous round's result. A round is the carve's re-route interval. There is no fresh restart (F4: no better than one round) |
| Stopping rule? | A fixed round count in the profile (§8), part of the keys. There is no fixed-point iteration to converge: the carve is a transient of fixed length (ADR 0103 amendment) |
| If it does not stabilise? | Nothing to stabilise: the rounds reproduce the whole-region carve exactly (F7). Whether the transient looks right is a separate question for the gates (§12) |
| How are boundaries treated? | Catchment borders are re-derived from the routing every round, so divides move between rounds, as in the whole-region carve. Inside a round they are exact separators. The region's border is the sea |
| What does a border guarantee? | Inside a round no cell of a catchment reads or writes another catchment (exact, F7). Across rounds, nothing is frozen |

- F2 and F4 tested a **different** solver: each catchment with its own routing and walls. F4's error came from MFD area crossing catchment borders and from the flood order (below). The carve itself is chaotic, so a pointwise comparison across seeds is meaningless (F4: 72 m rms between jitter seeds), but the round solver has no such error against the whole-region carve at all.

### 6.3 Where the coupling sits

F7 measured what couples catchments in a routing, and the contract removes each coupling or names it:

| Coupling | Measured (F7, catchment alone against the window routing) | Contract |
|---|---|---|
| **MFD area crosses D8 borders** (a cell shares its area with a lower neighbour in another catchment) | 24k-52k cells of a catchment differ in area, max ratio up to 20 and more | clipped: a catchment's MFD shares only among its own cells; a share towards a lower neighbour in another catchment goes to the cell's own receiver. A small change to the kernel (§6.4). Catchments become closed for area |
| **The flood's tie order** (FIFO `flat` queue, global heap order on equal levels) | after round 1: 3 to 182 cells of different receivers, up to 3,191 filled cells differing, by up to 0.245 m | canonical: the flood orders its visits by (level, global cell id), so a catchment's flood does not depend on other catchments' cells **[gate G-LOCAL]** |
| **Region border** | none for a landmass (the border is the sea) | fallback only (§6.1) |

### 6.4 The two kernel changes

Both change the bake's output slightly and carry a kernel version bump:
1. MFD clipped to the D8 catchment (`route.rs`).
2. Flood tie rule by (level, global cell id) (`flood.rs`), replacing the FIFO order; the cost on flats is measured **[gate G-SCALE]**.

Result: for any set of catchments closed under the routing, its round depends only on its own cells' surface at the round start and its drivers. **[gate G-LOCAL]** verifies it.

### 6.5 Re-solve of an edit (the incremental path)

- The world build and an edit re-solve run the same rounds; the difference is memoisation.
- Each catchment-round is a pure function: `result = f(catchment's cells at round start, drivers on them, profile, kernel versions)`. Its key is a hash of those inputs.
- **The re-solve is memoisation by that key:** a catchment-round whose key exists is read back; one whose key is new is computed. An edit changes keys only where its inputs changed.
- Each round's surface is kept as a bake record (§10), so a clean catchment's start-of-round state is readable.
- Divide movement enlarges the set by itself: a cell that joins another catchment changes that catchment's inputs at the next round. The closure grows only where the routing actually changes **[gate G-INC]**.
- A round's region-wide routing is the cost that remains (F7: 85-90 %). In a re-solve it runs only over the union of the dirty catchments (closed under drainage), which equals the region-wide routing there by §6.3 **[gate G-LOCAL]**. A level-2 job's memory is that union's, at about 64 bytes a cell: the largest catchment (at most 5.9e6 km^2 in F6, on an older store; the current base has 9.1e5 km^2 at a cube-edge crossing) is about 0.6 GB at most.
- **The equality that matters:** a re-solve equals a from-scratch build of the edited definition, because both are the same memoised function.

### 6.6 Downstream regions and K7

- K6 (H1) reads the final round's routing of the region. K7's tile refinement reads K5 and K6 of its region and is keyed per tile as today.
- K7 stays per tile with its halo (GEN-101): it does not route globally; it reads the channel levels as boundary conditions (§3.3).

## 7. Edits, order and dependencies

### 7.1 The layer order

| Position | Items | Effect |
|---|---|---|
| Below erosion | height items that are landforms (sculpt, raise, lower, landform stamps); driver items (lithology, uplift, rainfall and temperature regions, class paint) | applied at K2 and K3, before H0, K5, K6 and K7; erosion and hydrology respond |
| Above erosion | constructed features: roads, flattened plots, cuttings, built objects | `A(d)` in §3.1; applied after the surface, as ADR 0007's stack does today; exact, never eroded (ADR 0089 point 11's override stage) |

- D4 of S105. Amends ADR 0007 and ADR 0066 point 4: "authored layers are stages after the base" holds for the above-erosion layers only.
- `B` stays one function: `B(d) = C1(lattice) + sum of below-erosion height items at d` (a read-time term; the lattice is not re-baked). The item's footprint re-keys what reads `B`.

### 7.2 Keys and dependencies

- Stage keys (ADR 0006) carry everything. New keys:
  - a catchment-round key (§6.5);
  - a K6 key per region, from its final round;
  - a K7 tile key folding the local inputs of its window.
- **Reach follows the routing, not a radius:** the dirty set of an edit is what §6.5 finds by keys. ADR 0066 point 1's radius bound applies to above-erosion items only.
- Invalidation of the runtime tiles: K7 tiles whose window intersects a changed catchment, and K8 tiles above them (ADR 0072's refresh path).

### 7.3 Propagation end

A catchment-round is recomputed exactly when its key changes. There is no tolerance test ("quantised to zero" is not a stopping rule). The inputs are the canonical stored forms (§9). A threshold crossing or a spill inside a catchment is a change of its output and flows into the next round's keys. Downstream catchments are affected through their inputs only (divide movement, and the routing's area via §6.3's clipped MFD).

### 7.4 Transactions

- A commit is a set of keys: the K5 rounds, K6, K7 and materials of the dirty set. The runtime keeps drawing the previous commit until the new one's tiles are ready (CON-22, ADR 0072).
- A background result for a superseded commit is dropped by key (ADR 0072 point 3).

### 7.5 Limits

- Game mode (R1b): an edit whose dirty set exceeds a budget is refused or deferred to a batch.
- Undo removes the item: its keys are the earlier ones, so the earlier results are read back; no reverse simulation.

## 8. The simulation profile

A `SimulationProfile` belongs to K1 and is part of every derived key. It is the only source of simulation time and quality.

| Field | Meaning | Earth default today |
|---|---|---|
| `rounds` | re-route rounds per run | 4 |
| `steps_per_round` | implicit steps per round | 5 |
| `k_dt` | stream-power `K dt` per step | 0.05 |
| `quality` | the level of a run: `preview` (the H0 route and overlays, §11) or `full` | full |
| process parameters | breach limits, thermal, deposition, thresholds | presets (ADR 0108) |

- Simulated duration is `rounds x steps_per_round x k_dt` in carve units, not years and not editor time.
- **Every solve starts from the definition.** An edit or a re-run never advances a previous surface. A different duration is a different profile, hence different keys; two histories are compared by running two profiles.

## 9. Determinism and canonical forms

Hard constraints (CON-07, CON-14, ADR 0010): same authoritative output across runs, thread counts, Windows and Linux on x86_64. ARM and macOS are not supported targets until they are added to CI (CON-26). No GPU result enters any authoritative value. `libm`, integer hashing and fixed-order reductions as today.

| Contract | Rule |
|---|---|
| Graph topology and basin class | f64 comparisons as today (deterministic on the supported targets, no fast-math); ties by global cell id |
| Flood order | canonical (level, global cell id) (§6.4) |
| Canonical form | every value one stage hands to a later one has **one stored form**, and the later stage reads that form, in a fresh run as in a re-solve (below) |
| Reproducibility against correctness | bit equality of a re-solve and a fresh build is a test of determinism, not of correctness; correctness has its own gates (§12) |
| Edit replay | the same item list and the same order give the same key set on any run (ADR 0007, §7) |

| Interface value | Canonical form | Read by |
|---|---|---|
| Base `B` | f32 lattice node, C1 read to f64 | all |
| Round surface (a stage output) | f32 height on the regional grid (0.25 m steps would flatten plains' slopes of 1e-4 at 814 m into false flats) | the next round, K6, K7 |
| Regional change into K7 | last round minus `B`, read C1 | K7 |
| Local change `L` | i16 in steps of 0.25 m (kind 13) | K8 |
| Channel levels, basin levels | f32 metres | K7, K11 |
| Diffuse area | u8 code (GEN-110) for consumers; the routing's f64 is scratch within a round | materials, K6 |

- A round reads the previous round's **stored** form (f32), not an in-memory f64. Today's bake holds the carve in f64 and stores f32 heights for the post-pass only (`bake/tile.rs:322`); under this contract a fresh build and a re-solve pass through the same form.
- The effect of the f32 form on the carve against a run in f64 is measured **[gate G-EQ]**.

## 10. Store and budget

R7: about 40 GB at most at Earth. **Hard gate G-STORE:** measured on real serialized data, including pack layout, indexes, keys, graph metadata and edit overrides; steady state and peak temporary (a planet build and a level-2 re-solve) are reported separately.

| Item | Today | Target (estimate, Earth) | Status |
|---|---|---|---|
| Field bake | 11.7 GB measured | 5-8 GB (narrow channels, ADR 0045 point 4) | to measure |
| Round records (4 rounds, regional grid, land, f32) | - | about 4 GB | a cache, evictable once the final is built and when no edit needs it |
| Local change `L` and channels (kind 13) | about 24 GB with fill and channels | 16-24 GB; fill channel removed | to measure |
| Diffuse area byte | present | 0.1 GB | |
| H0 | - | under 0.1 GB | |
| Channel graph | - | 0.8-1.1 GB at a 50-100 km^2 threshold, vertices on the regional grid (F3, extrapolated) | to measure |
| Basins, lakes | - | under 0.1 GB | |
| Catchment-round key table | - | to measure | |
| Edit overrides | - | to measure | |
| **Total** | about 36 GB | about 26-38 GB with the round records; about 22-34 GB without | gate |

Fallback if the gate fails with field narrowing done: the local grid at about 200 m (C-f). It is not an assumption of this design.

## 11. Runtime and previews

### 11.1 Preview levels per edit kind (R2)

| Level | What | Time | Authoritative? |
|---|---|---|---|
| 0 | the item drawn over the current surface (ADR 0072) or its driver map | frames | no: a preview field with its own id (CON-07) |
| 1 | H0 re-route (planet-wide; heights resident), the trunk rivers and watersheds as an overlay | seconds (F6: 7 s) | no |
| 2 | the re-solve of the dirty set (§6.5), then K6, K7 | minutes, background | yes |
| 3 | a planet or continent re-run, on demand | hours | yes |

| Edit kind | L0 | L1 | L2 scope | L3 when |
|---|---|---|---|---|
| Height, landform stamp | the item over the surface | H0 re-route | dirty catchments | continental edits |
| Lithology or uplift region | the driver map (GEN-122, GEN-125) | none | catchments under the region | as above |
| Rainfall or temperature region | the climate map (GEN-130) | H0 with the runoff change | catchments whose runoff changed | climate reach beyond the region |
| Class paint | weights preview | none | as height edit | as above |
| Lake, river (post-MVP) | the object | H0 with the constraint | conditioned solves | - |

### 11.2 What the runtime reads

The runtime reads `S` (§3.1), river segments and lake levels from K6 (tile data, not per-frame work). The plan never requests a solve (CON-03, ADR 0086's lesson).

### 11.3 Realtime (R3)

- Solves run on terrain's core share (BUDG-003) and within terrain's 4.0 GB (BUDG-005).
- A level-2 job is bounded by its catchment union (§6.5); a landmass-wide run is level 3.
- A frame draws the last committed set (§7.4); the commit swaps by key (ADR 0072).

## 12. Validation

### 12.1 Gates

These are S105 §13.3's table, renamed, with the changes the round solver brings.

| Gate | Measure | Pass |
|---|---|---|
| G-SURF | runtime minus bake surface at lattice nodes | 0 before quantisation; at most 0.25 m after |
| G-CON | segments and channel cells at every sampling; drainage area at the outlets against the sum of true cell areas, across cube edges | 0 disconnected; every channel cell has a downhill path; relative error at most 1e-9 |
| G-LOCAL | a catchment's round alone against its round inside the region, with clipped MFD and the canonical flood | 0 cells differ |
| G-ROUND | the round solver against a whole-region carve | 0 cells differ (the exactness of F7, kept under §6.4) |
| G-EQ | a re-solve against a fresh build of the edited definition, E1-E6; the f32 round form against a run in f64 | bit-equal; effect on relief within 2 % |
| G-INC | the dirty set of each edit | grows only through the routing; report sizes, target a level-2 job within §11.3 |
| G-BASIN | E3's merge, split, spill and outlet migration | ids and continuity maps follow §5.4 |
| G-SED | per catchment and per round | eroded = deposited + exported to 1e-9 relative |
| G-LAND | relief p50 and hypsometry against the whole-region carve; sinks | within the seed noise floor; unclassified sinks at most 1 % of land (reported, not the main gate) |
| G-STORE | §10 | 35 GB target, 40 GB hard |
| G-SCALE | a landmass as one region: memory and time; flat-area cost of the flood; H0 at 2x radius | fits a world build within budget (CON-21 applies to runtime; the bake budget is proposed in ADR 0115); F6 x 4 within 1.5x |
| G-RT | the region's flight | no held-back tiles; frame within BUDG-001/002 |
| G-DET | 1 and 8 threads; Windows and Linux; edit replay | bit-equal |

- Correctness is structural (D10): connectivity, conservation, seams, locality, relief. Closeness to one reference run is not a criterion (F4).
- The Earth comparisons (SPIKE-102, SPIKE-104) stay reported values, not gates (R6).

### 12.2 Fixtures

Named landscape fixtures (the first review's suite), the prototype region and the edit cases E1-E6 are defined in `spikes/prototype-region/results/` (S105 §13.3).

### 12.3 Who judges

The planner judges numeric gates. A failure of an architecture or requirement gate (G-LOCAL, G-ROUND, G-EQ, G-STORE, G-SCALE) and the A/B gate go to the owner (D8).

## 13. ADRs (proposed, numbers reserved)

| ADR | Subject | Amends |
|---|---|---|
| 0110 | The authoritative surface | 0088 points 2 and 5, 0099 point 3, 0100, 0101 point 3 |
| 0111 | Hydrology model and identity | 0041 (producers), 0101 |
| 0112 | The erosion solver: regions, rounds, catchments | 0088 point 4, 0099 |
| 0113 | Edit ordering and dependencies | 0007, 0066 points 1 and 4, CON-08 reading |
| 0114 | Simulation profile, cell metric, canonical forms | 0010 (forms), 0003 (metric use) |
| 0115 | Scale as a parameter and the store budget | 0045 point 2 |

ADR 0109 (infill), ADR 0105 point 1 (pass-1 deposition for pits) are superseded by 0112 if the gate passes; the kernel-level changes (MFD clip, canonical flood) are within 0112.

Open items for the review:
1. Whether clipped MFD changes the look (the kernels' area at divides).
2. The cost of the canonical flood on large flats.
3. Whether a landmass as a region fits a world build; the split fallback's error.
4. The f32 canonical form of round surfaces against a run in f64.
