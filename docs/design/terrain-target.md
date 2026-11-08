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

<!-- next -->
