# Terrain and hydrology: target architecture and data contracts (A3)

Status: proposed (spikes session for opus-planner, 2026-11-08; inbox #100). Route: option A, conditional (owner, 2026-11-08; SPIKE-105 §13.5). Decisions 2-10 of SPIKE-105 §13.5 confirmed at their recommended options (owner, 2026-11-11). ADRs 0110-0115 accepted for the prototype (owner, 2026-11-11), with the conditions of §12.4. Amended after the external reviews of A3 (S105 §15) and of ADRs 0110-0115 (S105 §18, §19). The prototype's binding contract; production approval follows the gates of §12 (the A/B gate).
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
| K4 | H0 coarse hydrology | planet-wide trunk drainage and watersheds at the coarse grid across cube edges (a preview, §5.1) | K3 | **N** | F6's probe; the coupled run of ADR 0077 was a predecessor |
| K5 | Regional geomorphology | per region: the erosion carve at the regional grid (§6) | K2, K3, K6 of the previous round | **X** | pass 1 (ADR 0088, 0099, 0102-0104); the kernels stay |
| K6 | H1 regional hydrology | the channel graph, the diffuse-area field, basins, lakes (§5) | the K5 surface | **N** (graph, lakes); **X** (basins from the filled surface of the routing) | GEN-110, GEN-145, ADR 0101 (GEN-121: the tile-seam minimax only) |
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
| 3 | `C(d)` | the creases: octaves below the local grid (GEN-109) | evaluated at read time on the CPU; stores nothing of its own. Analytic octaves; with the relief switch on, the amplitude reads the baked driver `k` (GEN-099) | none (CPU f64) |
| 4 | `A(d)` | items above erosion (§7.1): constructed features | item stages (ADR 0066) | as today |

- Surface: `S = B + L + C + A`, composed in f64, rounded to f32 only at the mesh (CON-13).
- **G-SURF measured (GEN-148, a tile of the prototype region at 102 m): holds at the centres of the local cells, where the bake defines its heights; the store-lattice vertices are a diagnostic (G-SURF-I, §12.1).** At the centres the runtime's `B + L` is the bake's surface (5.7e-14 m before quantisation, half a step after). At the lattice vertices 192 of 28,900 (0.66 %) differ by more than 0.25 m after quantisation (p99 0.22 m, maximum 2.55 m), spread over the tile and beside steep flanks. The reference is an interpolant of the bake's cell values (the bake defines `z` at centres only), which differs from the stored `B` by up to 2.6 m there; the clamp of the C1 read is not linear. Reading the composed cells and clamping once gives 0.16 m at all nodes at 16 lattice reads a sample. Owner, 2026-11-09: equality is required at the centres, the vertices stay reported, the clamp stays. Stored since: `L` as kind 13 (GEN-154, GEN-156); re-read on the canonical K7 with the same numbers (ea6b1b2b). Not done: G-PROFILE, a K6 level profile, tile continuity.
- **One elevation source:** nothing else adds height. `L` is the only baked elevation term the runtime reads (`k` is an amplitude, not a height). The regional change, the channels and the fill depth are inputs of the bake, not runtime terms (§3.3).
- Step 2 replaces today's `change + fill depth` (GEN-112, ADR 0101 point 3): the runtime no longer adds the fill depth back. A basin shows its floor and the water field draws its lake (§5.5). The lake producer exists (GEN-163, bake time) and its records are stored as kind 19 (STRM-080); the water extent is a mask in the kind-13 v3 record. The switch (item 15 PR 5a) waits for the water source, and the fill channel's deletion (PR 5b) for ADR 0110's acceptance.
- Presentation data (ground detail, normals) follows CON-14 and adds no height.

### 3.2 Identity

- The base lattice is one object per body. The bake reads `B` through the same read as the runtime: pass 2's stride-4 window (`sample_window_coarse`) is retired (SPIKE-104 D-B).
- `base_id` is the hash of everything that fixes `B`'s value at a direction:

| Input | Why |
|---|---|
| the lattice's bake key (ADR 0006), its source and version | the stored values |
| the C1 read's version and parameters (GEN-107), clamp included | the same nodes read differently are another surface |
| the mapping, radius and `k` (§4) | direction to node |
| the sea level | the coast fade (GEN-113), outlets, the land mask |
| the below-erosion height items whose footprint meets the tile, in canonical order (§7.1), by their canonical stored form | `B` includes them at read time |
| the stored form of the lattice node (f32) and of each read (f64) | §9 |

- Every derived layer (K5, K6, K7, channels) folds `base_id` into its key, so a layer can only be read against the base it was taken on. **X** (GEN-114 folds the bake key today; the base's read and the item list become part of the identity).
- The surface's identity adds `C`'s band, parameters and version, the `k` channel's hash (GEN-099, GEN-109), and the items above erosion.
- A mismatch is refused, naming both keys (STRM-065 behaviour). No silent application.

### 3.3 What the bake passes to the runtime

| Bake product | Runtime term? | Use |
|---|---|---|
| K5's regional change | no | input of K7 (and of re-solves); a bake record (§10) |
| K6's channel centrelines and levels | no | input of K7: burned in (below); drawn as rivers by K11 |
| K7's total change against `B` | yes: `L` | the surface |
| Fill depth | no | retired as a surface term; the basin object replaces it (§5.3) |

- **Burn-in rule (K7):** K7 starts from `B + C1(ΔR)` (the regional change read C1). It lowers cells along each channel segment to the segment's level profile, in a trench `max(1 cell, w)` wide with `w = a Q_w^b` (GEN-165; `a` = 7.2, `b` = 0.5 as named bake parameters, Q_w in m³/s, never an area: D19, D21, D33), rasterised along the segment's D8 path at the local grid, then runs the breach (SPIKE-104 D-A: pass 1's rule at the local grid), the incising carve, thermal and deposition. The burn-in only lowers; thermal and deposition after it may raise cells (G-PROFILE bounds the result along a segment). Channels are thus part of `L`, not a separate term. The same `w` draws the river (D6).
- **The hand-over rule:** a C1 read must not be used to transfer a channel network between grids: it re-dams diagonal links (SPIKE-104 §3, 13.3 % of land). Burn-in plus breach is the transfer **[gate G-CON]**.
- **The channel after the sampler:** a channel correct on the local grid must stay correct through the composition `S`. Along every segment, at the local grid's nodes: no rise above one quantisation step (0.25 m), continuous across tile borders, and a confluence's floors meet. Between nodes a C1 read lifts a diagonal link toward its off-diagonal neighbours, by design; that rise is reported, not gated, as is the rise from `C` **[gate G-PROFILE]**.
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

- `k` follows from the body's target local cell size: the largest `k` whose local cell is at least the target (supersedes ADR 0045 point 2's ceil on the bake cell; Earth: 101.8 m at `k` = 14, printed as 102 m). Here `k` is the grid exponent; the relief driver is `krelief` and the round count `rounds`. The grid counts are never constants of the code (X11 of S105 §3).
- Nesting: the base lattice's nodes are every 6th local cell corner; a regional cell is 8 local cells; an H0 cell is 4 regional cells.

### 4.2 The cell metric

- Every grid exposes `area(i, j)` and `distance(i, j, dir)` from the mapping (tangent warp, ADR 0003): area by the exact spherical quad (S105 F5: the sum matches the face area to 3e-14), distance by the great-circle between cell centres. **N** (`Grid` today has one `dx`).
- Routing, MFD area, slopes, `f = K dt sqrt(A) / d`, river thresholds and basin areas read the metric. The uniform `dx` is retired. Effect today: areas are overstated by about 18 % on average (F5), so `K dt` and the area-based constants are re-tuned once.
- Cross-face neighbours come from one function over directions (F6), the same everywhere; it is symmetric, including at cube corners, by rule: a cell's neighbours are computed once and the relation is symmetrised by canonical pairs **[gate G-CON]**.

### 4.3 Scale (design-level)

| Body | Radius | `k` | Local cell | Land share (assumed) | `B + L + hydrology` store | H0 (F6 scaled) | Notes |
|---|---|---|---|---|---|---|---|
| Moon | 1,737 km | 12 | about 111 m | no runoff | `B` + stamps only | skipped | hydrology off: K4 to K7 skip (R5b) |
| Earth | 6,371 km | 14 | 102 m | 29 % | see §10 | about 7 s | target |
| 2x Earth | 12,742 km | 14 | 204 m | 29 % | as Earth (same counts) | as Earth | the larger planet takes coarser cells so the store stays inside R7; the local detail comes from creases |
| 2x Earth at 102 m | 12,742 km | 15 | 102 m | 29 % | about 4x Earth: over R7 | about 4x | allowed only with a store budget of its own |

- Three owners, all named parameters of the world definition (ADR 0108), with Earth as the default values:

| Owner | Holds |
|---|---|
| Body | radius, gravity, rotation, tilt, atmosphere, sea level, the hydrology switch |
| Geology preset | K, U, lithology distributions, class rules, thresholds |
| Hydrology and climate | precipitation, runoff model, PET, initiation `T` |

- A preset supplies defaults; the body remains the owner of its physical parameters. A radius alone sets no geology; it does set the cell metric and the grids.
- **Hydrology switch.** K5 to K7 run when the body's hydrology is on, whether or not it has a sea. A body with hydrology off has `B` and items only, and K4 to K7 skip. Hydrology on needs at least one outlet of the flood: in the MVP the sea. A body with runoff and no sea is refused with an error (its closed basins as outlets are post-MVP; §5.6). A body with a sea and no runoff switches hydrology off.
- **The three scale levers are different choices:** a larger body at the same `k` (coarser cells, same store); `k` + 1 (the same cell size, 4x store); fewer features (a body-specific budget).
- **Scaling model (design-level; G-SCALE checks it at two bodies):**

| Quantity | Formula |
|---|---|
| Regional cells on land | `land_share x 6 x (3 x 2^(k-2))^2` |
| Local cells on land | `land_share x 6 x (6 x 2^k)^2` |
| Store | `store(k) = store(14) x 4^(k-14)` x `land_share / 0.29`, at the same channels |
| Build memory, largest landmass | bytes a cell (55 for the resident rounds on the compact landmass, §6.6) x the largest landmass's regional cells |
| Local cell size | `2 pi R / (4 x 6 x 2^k)` (nominal) |

- Climate at scale: orography lattice counts follow the same `k` rule; the preset's winds and tilt (S105 X12).
- Terrain, hydrology and store for the 2x case are checked as a design table here and one 2x H0 run in the prototype (S105 §13.3).

## 5. The hydrology model

Two levels and two representations. The levels are H0 (planet-wide, coarse) and H1 (regional grid). The representations must not be conflated: the **channel graph** (a tree of discrete rivers) and the **diffuse field** (a raster of accumulated area).

### 5.1 H0: the coarse planet-wide route (K4) **N**

- Grid: H0 cells (3.26 km at Earth), all six faces as one graph, neighbours across cube edges (§4.2), one priority flood from the sea, D8 receivers, area over true cell areas. F6: 7 s with heights in memory, 2.2 GB.
- Output, exactly: trunk connectivity and watersheds for the level-1 preview (§11), and a coarse estimate of the landmass partition for planning work. H0 constrains H1 in nothing: H1 determines the authoritative drainage and channel topology from its own surface, and the regions from its own land mask (§6.1). An isthmus or strait under the H0 cell exists at H1 only.
- H0 does **not** cut erosion domains (F4: 105 m rms and a seam).

### 5.2 H1: the channel graph and the diffuse field (K6)

| Representation | What | Grid | Stored | Mark |
|---|---|---|---|---|
| **Diffuse field** | `A_d`, the effective area accumulated by MFD (Freeman, exponent 1.1); the magnitude at every land cell | regional grid | f64 scratch of a round; a one-way byte for materials (below) | **X** |
| **Channel graph** | one D8 receiver per channel cell (the routing of the filled surface, jitter from the global cell id); channel cells are defined below | regional grid | as segments only (§5.4), each vertex with its routed discharge `Q_w` | **N** |
| Receivers elsewhere (hillslopes) | scratch of a solve | regional grid | not stored; recomputed by a re-solve | - |

Rules. The quantities, kept apart (`a` the true cell area in m^2):

| Quantity | Definition | Unit | Used for |
|---|---|---|---|
| **Geometric area** `A_g` | the sum of `a` over the members of a D8 tree, at an outlet or a tree node | m^2 | catchment size, classification; `A_d`'s closure when `w` = 1 |
| **Effective area** `A_d` | MFD accumulation (clipped to the catchment, §6.3) of `a w`; `w` the dimensionless `discharge_weight` (GEN-129: the runoff over the reference 300 mm a year, clamped to 0.02 to 8) | m^2 of reference runoff | the carve's erosion (ADR 0099), initiation, materials |
| **Initiation** | a cell where `A_d >= T` | - | starts a channel |
| **Potential discharge** `Q` | accumulated along the D8 tree: `Q(c) = a(c) r(c) + sum of Q over all D8 donors of c`, over **every** land cell, channel or not; `r` the runoff in m/s (GEN-129's `runoff_mm`, not weight-clamped, non-negative) | m^3/s | the conservation check |
| **Routed discharge** `Q_w` | `Q` with the basins' losses applied (§5.5) | m^3/s | rivers, widths, lakes' inflow |

- **`A_d` closes on its own quantity.** At an outlet `A_d` equals the sum of `a w` over the catchment's members (to the floating-point error of the sum; G-CON), and equals `A_g` only where `w` = 1.
- **Runoff reaches the channel along D8.** Every cell has a D8 receiver in the routing; a hillslope cell's runoff follows its D8 path and enters the channel graph at the first channel cell it reaches. The hillslope receivers are scratch of the solve.
- **Connectivity comes from D8.** A channel cell is an initiated cell, or the D8 receiver of a channel cell: once started, a channel **continues downstream** to its outlet even where `A_d` falls below `T`; there is no restart.
- **`Q` is conserved.** Confluence rule: `Q` is the sum of its donors plus the local term, so it is non-decreasing along a segment by construction (`r >= 0`) and the sum of `Q` at all outlets equals the sum of `a r` over land (G-CON). The running maximum of `A_d` used in the earlier draft is dropped.
- `A_d` and `Q` differ by design: MFD spreads diffuse flow, D8 keeps a river as one line; `w` is clamped, `r` is not. The carve keeps `A_d`; rivers report `Q_w`.
- **The stored byte.** `round(8 log2(A_d / a_n))`, `a_n` the body's mean regional cell area: code 0 is one reference cell or less, one step is 9 %, 255 is 2^31.9 cells and saturates. It is one-way, for materials and detail (GEN-110, GEN-119). No simulation stage reads it: the carve, the initiation and the key read the routing's f64.
- **River initiation:** `T` is a preset parameter in m^2 of effective area (default 50-100 km^2, SPIKE-105 §13.5), calibrated with the runoff model, not a universal constant. Because `A_d` is runoff-weighted, a rainfall edit moves initiation with no height change (E5).
- A channel's heads are initiated cells with no channel donor. Confluences have two or more channel donors. A channel ends at an outlet of the routing (the sea, the mouth); it passes through a basin and continues below its spill point. In H1's graph a channel passes through a basin. The stored river pieces do not: the lake producer drops the channel cells inside a kept lake, so a piece ends at the last cell before the water and the next starts at the spill cell (GEN-163; inside = below the lake's level). Below an endorheic basin the channel continues with `Q_w` = 0, a dry valley that K7 still burns in and K11 does not draw as water.

### 5.3 Basins

A basin is a connected set of cells (8-neighbour) below the routing surface's spill level of more than `pit_min_m` (0.5 m), found on the filled surface of the round's routing (GEN-145). **X** Stored as one object per basin above a size threshold, on the regional grid:

| Field | Meaning |
|---|---|
| id | canonical identity (§5.4) |
| floor level | lowest cell height (canonical form, §9) |
| spill level | the filled level at the spill point |
| spill point | the rim cell of the lowest filled level (ties by cell id); exists for every basin |
| active outlet | the spill point when the water reaches the spill level (§5.5), else none |
| area curve | surface area and volume at 16 fixed level steps between floor and spill |
| type | one of the classes below |
| water level | the lake's state (§5.5) |

| Class | Rule (presets, ADR 0108) | Behaviour |
|---|---|---|
| artefact | fewer than `min_cells` cells and shallower than `min_depth_m` (in cells, so that it scales with the grid) | no object and no water; the pit stays in the surface unless K5's or K7's breach removes it (the breach does not read classes); counted by G-LAND |
| lake | deeper or larger, with an active outlet by §5.5 | kept; lake at the spill level |
| endorheic | no active outlet by §5.5 (ADR 0105 point 2) | kept; sink for water and sediment; water level from §5.5; playa if the level is at the floor |
| intentional | named by an item (§5.6) or a driver (caldera, karst later) | kept whatever its size |

- Thresholds are preset parameters; a basin's class is a pure function of the surface and the drivers.
- The canonical flood's level is the minimax spill level plus the epsilon ramp, by its construction (GEN-141); `bake/spill.rs` is the tile-seam minimax and gives no spill points or basin objects, so it is not used for basins. The equality of the two levels is not tested.
- A component whose depth from floor to rim is under `pit_min_m` is the epsilon ramp of a flat, not a basin (it is counted, not kept).
- **Touching basins are one component.** Two basins with different spill levels that touch are one basin with one floor and one spill (the lowest rim); nested basins are not separated (GEN-145). Separating them needs the spill graph's hierarchy and is open.
- A class is computed once, from the final round's surface (K6). A basin exists if the final surface has it: a depression a breach has opened is no longer a basin and has no object; the id of §5.4 is what continues. An artefact and a natural endorheic basin differ by size and depth, and by the balance of §5.5. Protecting an intentional basin from the breach (a mask read by K5) is open, with §5.6.

### 5.4 Identity under change

Identities are **pure functions of the current surface**, so a from-scratch build of the edited definition yields the same ids (§9). Continuity across an edit is a separate, non-authoritative map.

| Object | Id | Definition |
|---|---|---|
| Basin | `basin_id = hash(body, canonical cell)` | canonical cell: the lowest cell of the basin, ties by the global cell id |
| River segment | `segment_id = hash(body, downstream end cell, upstream end cell)` | ends are confluences, heads, outlets; a stored piece also ends at a lake entry or starts at a spill cell; cell ids on the regional grid |
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

- A lake is a basin with a water level. Producer of the water field W (ADR 0041 point 7's retrofit path). **X**
- **Balance (steady state, no time stepping; a pure function of the surface and the drivers).** `Q_in = A(h) E_open + q_out(h)`:
  - `Q_in`: the routed discharge `Q_w` of the basin's exit cells (those whose receiver lies outside the basin and whose flow does not re-enter it). It includes the local runoff of the basin's cells.
  - `E_open = PET - (P - r)`, the mean over the basin's cells (PET of GEN-117): open-water evaporation in excess of the land's, charged only where the ground is flooded. Never negative: Budyko's land evaporation `P - r` is at most PET.
  - `A(h)`: the area curve (§5.3), linear between its steps, non-decreasing.
  - `q_out(h)`: 0 below the spill level.

| Case | Level `h` | Class | `q_out` |
|---|---|---|---|
| `Q_in` <= `A(spill) E_open`, `Q_in` > 0 | the lowest `h` with `A(h) E_open = Q_in` | endorheic | 0 |
| `Q_in` > `A(spill) E_open` (also every `E_open` = 0 with inflow) | the spill level; the active outlet is the spill point | lake | `Q_in - A(spill) E_open` |
| `Q_in` = 0 | the floor | endorheic (playa) | 0 |

- A finite level below the spill exists exactly when evaporation can balance the inflow; otherwise the basin overflows, which is not a failure.
- **The cascade (`Q_w`).** Computed like `Q`, cells in descending filled level (ties by the global id), with one change at a kept basin: the flow of its exit cells stops there, and `q_out` enters at its spill cell. A basin's cells all stand above its spill level, so a basin is resolved before anything below it, and a chain of basins resolves in one pass.
- **Closure:** the sum of `Q_w` at the outlets plus the sum over basins of `Q_in - q_out` equals the sum of `a r` (G-CON).
- **Open:** nested basins are not separated (§5.3): a basin below its spill uses one water surface over its sub-basins. The carve's `A_d` runs through every basin to the sea (ADR 0105 point 2 is not in the round solver), so `A_d` below an endorheic basin is overstated; reported at G-LAND. A fix would read the previous round's classes.
- The fill depth stops being a surface term (§3.1) and a stored channel; it is the basin's water level minus the ground, derived from the basin object at the local grid.
- Water surfaces render through ADR 0041's path; a basin's area curve answers "what is flooded at level x" without a raster.

### 5.6 Authoring rivers and lakes (post-MVP; not blocked)

A later item kind sets constraints on K6's input (a basin of class intentional, a fixed channel), keyed like any item below erosion (§7.1).

## 6. The erosion solver: regions, catchments and rounds

The reviewers asked for the outer iteration's contract. Evidence: S105 F2 and F4 (a catchment carved with its own routing differs by 12.8 m rms at best) and F7 below. The contract replaces "watershed domains" as first proposed (S105 §6, §13.4).

### 6.1 Scopes

| Unit | Definition | Lifetime |
|---|---|---|
| **Region** | a connected component of the land mask at the regional grid (8 neighbours, cube edges joined), with its sea outlets. Border = the sea: no artificial cut. A landmass is routed whole; it need not be resident (§6.6). Re-derived from the mask at every commit (§7.2) | the scope of one routing |
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
| **MFD area crosses D8 borders** (a cell shares its area with a lower neighbour in another catchment) | 24k-52k cells of a catchment differ in area, max ratio up to 20 and more | clipped (local form, GEN-141): lower neighbours in another catchment are dropped from a cell's weights and from their sum; the cell shares among its own catchment's lower neighbours. Its D8 receiver is always one of them (strictly lower on the filled surface, `receiver_of`, and in its catchment by definition), so a non-outlet cell always has a positive weight and needs no fallback. A small change to the kernel (§6.4). Catchments become closed for `A_d`: at an outlet it is the sum of `a w` over the members (§5.2). A form that sends the foreign share to the cell's own receiver keeps the foreign levels in the denominator and is not closed in this sense (measured below) |
| **The flood's tie order** (FIFO `flat` queue, global heap order on equal levels) | after round 1: 3 to 182 cells of different receivers, up to 3,191 filled cells differing, by up to 0.245 m | canonical: the flood orders its visits by (level, global cell id), with no FIFO queue (GEN-141). The result is a pure function of the heights. It does **not** make a catchment's flood independent of its neighbours: a cell on a flat can be first reached from a neighbour in another catchment (measured below). Routing locality is gated as the tile-partitioned route against the whole-region route **[gate G-PART]** |
| **Region border** | none for a landmass (the border is the sea) | fallback only (§6.1) |

### 6.4 The two kernel changes

Both change the bake's output slightly; they carry no kernel version bump, but a key byte when on (GEN-141):
1. MFD clipped to the D8 catchment (`route.rs`).
2. Flood tie rule by (level, global cell id) (`flood.rs`), replacing the FIFO order; the cost on flats is measured **[gate G-SCALE]**.

Result: a catchment's round depends only on the inputs of its key (§6.5): its members, receivers, `A_d`, surface, drivers and the round rules. **[gate G-LOCAL]** verifies that on the new kernels (the round is a pure function of its key inputs; G-ROUND is the exactness test). Both rules are behind switches (`RoundRules`, default off; a key byte only when on).

Measured (GEN-141, `openspec/changes/round-kernels/design.md`; four 96 x 96 lands, about 820 catchments; a catchment **routed alone** is the grid masked to its members with its outlet the only outlet):
- Member areas that differ from the region routing, smooth land: today's kernels 26,041 of 26,265; clip to the receiver 23,939; local clip 0; local clip with the canonical flood 0.
- Terraced land (flats): 27,090 of 27,942; 25,043; 45; 7. Receivers that differ: 0 to 1.
- The remainder is the flood: a member on a near-sea-level flat is first reached from a lower neighbour in another catchment, its filled level moves by 0.4 mm, a neighbouring receiver flips.

Contract:
- A catchment routed alone does not equal its part of the region routing, for the measured flood rules (FIFO and canonical). It is not part of the contract.
- A catchment's round reads receivers and `A_d` from the region re-route (run as tiles, §6.5) as key inputs. G-LOCAL is the round as a pure function of those inputs; G-ROUND shows it exact.
- Routing locality is G-PART: the tile-partitioned route against the whole-region route, 0 cells differ.
- Option kept in reserve: a raised cell takes its flooder as receiver. On the same lands 0 of 54,207 member cells then differ when catchments are routed alone (probe, not in the kernels).
- Flood cost at 3,072 cells a side, one thread: a land 1.30 s FIFO, 1.37 s canonical; broad terraces 0.29 s and 0.99 s. **[gate G-SCALE]** reports it on the largest landmass.

### 6.5 Re-solve of an edit (the incremental path)

The problem: the new catchments are not known before a routing, yet routing is 85-90 % of a round. Four approaches:

| Approach | What it needs | Verdict |
|---|---|---|
| 1. Re-route the whole region after an edit | nothing beyond the round | correct and simple; F8 (largest landmass, 1.49e8 cells at H1): flood 55 s, receivers 6.5 s, MFD 77 s a routing, 4.5 GiB resident. Acceptable for level 3; not for level 2 inside the editor |
| 2. Re-route a conservatively expanded dirty region | a proven border and expansion rule | rejected: no rule is available that guarantees water does not cross the border after the edit, without already having routed |
| 3. Incrementally update the routing graph (dynamic flood, depression hierarchy) | considerably more algorithm | later optimisation only |
| 4. Re-route the old dirty catchments only | that all divide changes are discoverable from the old routing | rejected: divide movement is exactly what the old routing cannot show (reviewer's circularity) |

**Chosen: approach 1's semantics, executed as a partitioned route with memoised tiles.** The re-solve's result is by definition a full region re-route; only the work is memoised, so correctness has no extra assumption:

1. The region is cut into fixed tiles of the regional grid (256 cells at Earth: 208 km).
2. A tile's flood is a pure function of its inputs: the tile's heights, its outlet and activity flags, the offers from its neighbours and the grid's parameters (`partition.rs`, the memo key). A tile is re-flooded when any of them changed: **a tile whose surface did not change re-floods if an offer did**.
3. The borders are solved for the whole region, every time: the tiles are swept, each flooded from the values its neighbours offer, until no offer changes (GEN-143; 5 sweeps, 65 tile floods for 35 tiles on the prototype window). It discovers the new divides and levels: there is nothing to predict. The minimax border graph of F9 (`SpillGraph`: 146,881 nodes, 17 ms for 3,072^2 cells) gives the spill level but not the epsilon-filled surface: epsilon steps do not compose across a tile in floating point, so the graph does not reproduce the resident bits.
4. Receivers and `A_d` follow the dependencies of §6.8. Today they are recomputed whole (GEN-143); their memo is an optimisation, to be justified by G-INC.
5. Catchments are read from the receivers; a catchment-round is recomputed if its key (below) is new; otherwise its stored round result is used.

F9 (E2-type edit on 144 tiles): the flood output differs in exactly the tiles whose heights changed (2, then 4 after one round); levels, receivers and area change in 2 to 4 tiles (2.8 %, under 0.25 % of cells), and tiles plus graph reproduce the whole-window flood exactly (0 cells differ). The work per round is therefore of the order of the changed tiles and the tile sweeps, with no prediction of the dirty set. GEN-143 measures the tile floods only (E2: 3 of 65, 4.6 %); the offers, receivers, roots, order and area sweep are recomputed whole (1.58 s an edit against 0.71 s for the resident route on 1.6e6 cells). **[gate G-INC, on E2]**

**The catchment-round key.** The rule: every value the catchment-round function reads is in its key, directly or through an immutable upstream key, in its canonical form (§9). Today's read set is `CatchmentInputs`; G-LOCAL enforces the rule by computing a round from the key's inputs alone. The hash, in canonical order, of:
- the member cell ids, in `Routing.order` restricted to the catchment: the depth-first walk of the donors from the outlet in cell-id order (the `cut` order), a function of the ids and receivers; a round on the same members in another order is not a key match;
- the round rules (`RoundRules`) the round reads;
- the outlet cell, and its base level (sea or basin spill);
- the receiver of every member, whether it is an outlet of the region, and `A_d` at every member (the routing topology and magnitude, §6.3);
- every member's distance to its receiver (the metric, or the mapping's identity);
- every member's round-start surface in canonical form (§9);
- the drivers at the members (erodibility by height, uplift, runoff, vegetation, taper);
- the `SimulationProfile` and the kernel versions.

With clipped MFD and the canonical flood a catchment has no other border conditions: the key holds everything its round reads. The jitter seed enters through the receivers; `base_id` and the routing's inputs enter through the surface and `A_d`, and are not read again.

- Equality with a fresh build follows: a fresh build is the same function with no memo hits.
- Divide movement needs no separate rule: it shows up as changed membership, hence a new key.

### 6.6 Memory model

| Use | What is resident | Measured or bounded |
|---|---|---|
| One routing of the largest landmass, resident (the simplest, used by the prototype and the first world build) | z f32, filled f64, receiver u32, order u32, area f64, id map u32: 32 bytes a cell | F8: 1.49e8 cells at H1 (a landmass of 8.4e7 km^2), 4.5 GiB, 5.1 GiB peak |
| One round of that landmass, resident | routing plus the carve's arrays | measured on the compact landmass at 12,288 cells: 4 rounds peak at 7,858 MiB (55 bytes a cell); the H1 pass is not measured at that size (81 bytes a cell at 37e6 cells: 12.1e9 bytes extrapolated) |
| Partitioned route | one tile's arrays at a time and the offers across tile borders. **Arrays resident; tile-wise form not built.** GEN-143 proves equality, GEN-149 runs it on the compact landmass. No memory saving over the resident route yet (62 against 55 bytes a cell), 1.7 times the time | 4 rounds without memo caches at 12,288 cells: peak 8,854 MiB (62 bytes a cell), 2,974 s; with memo caches about 340 bytes a cell |
| Level-2 re-solve in the editor | the changed tiles and the catchments dirty in that round, within terrain's 4 GB (BUDG-005) | gate G-INC |

- **Measured on the whole prototype landmass at 12,288 cells** (`examples/landmass_scale`, GEN-147, a compact grid of its 149.3e6 cells): 4 resident rounds peak at 7,858 MiB (55 bytes a cell; 62 at 37e6 cells, 85 at 9e6) in 1,716 s on 8 threads; the H1 pass costs 81 bytes a cell as built (2,905 MiB at 37e6 cells; not run at 12,288 cells: 12.1e9 bytes extrapolated, at the budget). The rounds fit the budget; the H1 pass, extrapolated, is at it. The partitioned rounds (GEN-149, no memo caches) peak at 8,854 MiB (62 bytes a cell) in 2,974 s, bit-equal to the resident rounds at 3,072 cells; their arrays are still resident, so a tile-wise form stays open.
- Measured at the erosion grid (12,288 cells, `examples/region_scale`, windows of the prototype region of 1.0e6 to 44e6 cells): the resident rounds peak at about 90 bytes a cell (74 without the example's height copies) and about 2 microseconds a cell on 8 threads; the whole prototype landmass (about 150e6 cells) is measured below (the bound of 11 to 13.5 GB was an extrapolation). The 2.98 GiB and the 39.5 % channel share of the GEN-144 and GEN-145 runs are at 3,072 cells (3.26 km); at 12,288 cells 7 % of land cells are channel cells.
- A landmass routing does **not** have to be resident. The partitioned route produces the same result (F9: levels equal, 0 cells differ); the tile flood is swept to a fixed point from the neighbours' offers and the accumulation is a topological sweep across tiles with border shares (`bake/partition.rs`, GEN-143).
- The world build runs the compact resident route (owner, 2026-11-09). The partitioned route stays a tested equal (G-PART), not the normal path. Editor jobs keep the partitioned route with memoised tiles (§6.5); G-INC judges it.
- World-build memory is a separate budget from CON-21 (a runtime rule): 12 GB (owner, 2026-11-08; ADR 0115 point 7, the ADR otherwise proposed), measured by G-SCALE as the process's peak commit charge on the reference machine, 12e9 bytes (11,444 MiB), the worst of two runs, with inputs and drivers, flood and routing arrays, accumulation, erosion state, tile-border state, temporary allocations, parallel overhead and the allocator, for the complete build (rounds, H1, sediment pass). It is separate from terrain's 4 GB at runtime (BUDG-005). An estimate is not a pass.
- **The compact resident route is the normal path (owner, 2026-11-09; replaces the partitioned route of 2026-11-08, whose memory bound is not built).** As built the partitioned route holds its arrays resident: 8,854 MiB in 2,974 s against 7,858 MiB in 1,716 s.
- **When the complete resident build exceeds 12 GB,** in order: (1) remove unnecessary retention, duplicate buffers and caches; (2) only if the unavoidable working set still exceeds the budget, prototype a tile-wise route that releases intermediate state. Moving state into other global buffers does not count. No tile-wise route is built before that.
- **H0-region split at artificial borders** (the earlier fallback) is kept only as a tested fallback for a landmass over the budget after that rule; G-SCALE measures its divide migration across the split. The design no longer relies on it.
- **Cache eviction is required (owner, 2026-11-09).** Without it the edit path's memo caches reach about 340 bytes a cell (about 50 GB on the largest landmass). Eviction never changes a result: undo, replay and fresh-build equality hold after any eviction (G-EQ).

| Data | Policy |
|---|---|
| User-authored sources, world definition, edit history | never evicted |
| Current committed result | retained |
| Data of the active solve | pinned until it completes |
| Previous-round intermediates, inactive edit caches | evictable |
| Recomputable derived data | recomputed on demand |

- **Scenarios measured (G-SCALE), peak working set and peak process memory, temporary allocations included:** fresh build; full build with rivers and basins; level-2 regional re-solve; repeated edits without undo; undo and redo after eviction; a landmass at the worst-case routing size. A scenario that needs a part not yet built is reported as not run.

### 6.7 Downstream regions and K7

- K6 (H1) reads the final round's routing of the region. K7's tile refinement reads K5 and K6 of its region and is keyed per tile as today.
- K7 stays per tile with its halo (GEN-101): it does not route globally; it reads the channel levels as boundary conditions (§3.3).
- **K7's tiling is part of its definition (ADR 0119, owner, 2026-11-12).** The tile size, grid origin, alignment and halo are K7's inputs and part of its key; their values live in the spec (GEN-101, layout requirement pending). Thread count, scheduling and order never change the result (CON-30, tier B). A different layout is a different definition: its difference is reported, not asserted. The seams of the canonical layout are checked by **[gate G-SEAM]**.
- **What is catchment-local.** The breach and the implicit sweeps: inside a round a catchment reads and writes only its members. Across rounds it couples through the surface.
- **Breach base level (GEN-162, 2026-11-10):** a breach never cuts below its chain's exit. Every cell of the chain is lowered at most to the exit's height (a flat floor), on land and at sea alike. A depression whose exit cannot be lowered (a window's land-height cut, a fixed outlet) is not excavated below it.
- **What is not.** Thermal and one-cell pit spreading read 8 neighbours and cross divides. They are not catchment-round steps: they run after the last round. Per tile with a halo, keyed per tile (as K7), is the target; today they run on a whole grid and the spatial cut is untested (GEN-155 tests groups of whole catchments) **[gate G-SED, G-SURF]**.
- **Sediment contract of the prototype (owner, 2026-11-09; describes what is built, GEN-151, GEN-152):**

| Question | Answer |
|---|---|
| When does sediment move? | Only in the sediment pass after the last round: thermal, the deposition sweep, the one-cell pit spreading. No transport inside a round: the carve removes material and tracks no load |
| Does the pass change elevation? | Yes: deposition raises cells; thermal and pit spreading move volume between cells |
| Does it redistribute sediment? | Yes: the eroded column (`original - z`, on land) is carried downstream along the surface's own route, fills depressions, settles over capacity |
| Is it only accounting? | No. The balance (G-SED) is a separate check, read from the heights and the carried load |
| Does it cross domain borders? | Today it runs on a whole grid and has no tile or catchment borders. Load reaching an outlet (the sea, a grid or landmass boundary) leaves as export. A later cut into tiles with a halo must pass G-SED's partition test |
| Does its output enter the next round? | No. A round reads the previous round's stored surface. GEN-152's sweep after each round runs on a copy and is a measurement |

- Within-round transport is revisited at the A/B gate, only on evidence: implausible deposition, artificial deposits at domain borders, a material change of drainage or basins, or missing large-scale landforms.
- "Independent catchments" is a statement about today's solver, not a limit on later sediment models: a later process that couples catchments is added to §6.3's table with its contract.

### 6.8 The reference computation and the dependency contract

The bit-equal claim of §6.6 has one normative reference, and it is a definition, not an algorithm: resident routing and partitioned routing are two ways of computing it. Heights are the round's canonical form, widened to f64 (§9).

| Result | Definition |
|---|---|
| Filled surface `F` | the unique solution of `F(c) = z(c)` for an outlet, else `F(c) = max(z(c), min over neighbours q of F(q) + eps)`, computed in f64 |
| Receiver of `c` | `receiver_of`: the steepest jittered drop on `F` over `c`'s neighbours, jitter a pure function of the seed, the global id and the direction |
| `A_d(c)` | `a(c) w(c)` plus the donors' shares, added in the order (donor's `F` descending, global id) |
| `Q(c)`, `Q_w(c)` | the local term plus the donors, in the order of the D8 topological sort (a pure function of the receivers) |

- **Why a fixed point is enough.** `x -> max(z, x + eps)` is monotone under IEEE rounding and strictly raises `x` (`x + eps > x`, asserted in both floods), so the fixed point is unique (generalised Dijkstra). The visiting order changes nothing: equal offers give equal candidates. The (level, id) tie order is what makes the **heap** reach it; the FIFO flat queue of the old flood does not (§6.3).
- **Offers.** A cross-tile offer is a pair (border cell, the lowest `F` of its neighbours outside the tile). The representation is f64 bits.
- **Termination.** Offers only fall within a solve: a tile's flood is monotone in its offers. After sweep `n`, every cell whose flood path crosses at most `n` tile borders is final. Sweeps are bounded by one plus the most border crossings of any flood path, at most the count of border cells; a repeated offer change is a fall.
- **Equivalence guaranteed:** bit equality of `F`, the receivers, `A_d`, `Q` and `Q_w` for any tile size, any sweep order and either route. A 128-bit memo collision is the only excluded case. **[gate G-PART]** on adversarial fixtures: depressions and divides across tile borders, a spiral flood path across tiles (the worst case for sweeps), tiles at cube corners.

**The dependency contract.** Correctness never rests on the memo: a full region recomputation is the fallback, and always allowed. Each step is reused only if all its inputs are bit-equal:

| Step | Inputs (a change in any one recomputes it) | Today |
|---|---|---|
| Tile flood | the tile's heights, outlet and active flags, its incoming offers, grid, `eps` | memoised (GEN-143) |
| Offers | `F` of the border rings | from the flood's result |
| Receivers of a tile | `F` of the tile and of its neighbour ring, the seed | recomputed whole |
| `A_d` of a tile | its `F`, its receivers (for the clip: membership), `w`, and the shares across its border, by tile and sweep in the topological order | recomputed whole |
| Catchment membership | the receivers | recomputed whole |
| Catchment round | its key (§6.5) | memoised by design |

- A region where these propagate widely is still correct; it is simply a slower edit, and §11's levels take it. G-INC reports the work. On the prototype window GEN-143's edit took 1.58 s against 0.71 s for the resident route, so the memo has not yet paid at that size.

## 7. Edits, order and dependencies

### 7.1 The layer order

| Position | Items | Effect |
|---|---|---|
| Below erosion | height items that are landforms (sculpt, raise, lower, landform stamps); driver items (lithology, uplift, rainfall and temperature regions, class paint) | applied at K2 and K3, before H0, K5, K6 and K7; erosion and hydrology respond |
| Above erosion | constructed features: roads, flattened plots, cuttings, built objects | `A(d)` in §3.1; applied after the surface, as ADR 0007's stack does today; exact, never eroded (ADR 0089 point 11's override stage) |

- SPIKE-105 §13.5 (edits below erosion). Amends ADR 0007 and ADR 0066 point 4: "authored layers are stages after the base" holds for the above-erosion layers only.
- `B` stays one function: `B(d) = C1(lattice) + sum of below-erosion height items at d` (a read-time term; the lattice is not re-baked). The item's footprint re-keys what reads `B`.

### 7.2 Keys and dependencies

- Stage keys (ADR 0006) carry everything. New keys:
  - a catchment-round key (§6.5);
  - a K6 key per region, from its final round;
  - a K7 tile key folding the local inputs of its window.
- **Reach follows the routing, not a radius:** the dirty set of an edit is what §6.5 finds by keys. ADR 0066 point 1's radius bound applies to above-erosion items only.
- Invalidation of the runtime tiles: K7 tiles whose window intersects a changed catchment, and K8 tiles above them (ADR 0072's refresh path).
- **Landmass topology.** An edit can change the land mask: a land bridge joins two regions, a coast cuts an island off, a basin reaches the sea. Regions are re-derived from the mask at every commit; a changed partition is a new set of regions. Tile and catchment keys hold no region id, so what did not change is still reused. All K5 and K6 state of an affected region is re-derived through those keys. A merged region that exceeds the job limits of §7.5 goes to level 3. A change of the sea level changes every outlet and is a full rebuild.

### 7.3 Propagation end

A catchment-round is recomputed exactly when its key changes. There is no tolerance test ("quantised to zero" is not a stopping rule). "Exact" means bit equality of the canonical stored forms (§9), not of in-memory values. A threshold crossing or a spill inside a catchment is a change of its output and flows into the next round's keys. Downstream catchments are affected through their inputs only (divide movement, and the routing's area via §6.3's clipped MFD).

### 7.4 Transactions

- A commit is a set of keys under one `commit_id`: the K5 rounds, K6 (channels, basins, water levels), K7, materials and the regions of the dirty set. **N**
- **Coherent reads.** A frame reads one commit, for surface, channels and water together. The runtime keeps drawing the previous commit until the new one's drawn tiles are ready, then swaps the commit (CON-22). New terrain with old channels, or new channels with old water levels, cannot be drawn.
- This is new against ADR 0072, which refreshes tile by tile. Per-tile refresh stays for items above erosion, where no hydrology depends on them.
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
| routing parameters | the flood's `eps`, the MFD exponent, the clip rule, the flood's tie rule (`RoundRules`) | today's |
| seed policy | the jitter's stream: a pure function of the world seed and the global cell id | today's |
| quantisation | `QUANT_STEP_M` (§3.1), the canonical forms of §9, the area code | today's |

- The profile is immutable for the duration of a solve, and holds everything that can change a result. The tile size of the partitioned route is deliberately not in it: it cannot change a bit (G-PART). This holds for routing; K7's tile layout and halo are in K7's key (ADR 0119).
- Simulated duration is `rounds x steps_per_round x k_dt` in carve units, not years and not editor time.
- **Every solve starts from the definition.** An edit or a re-run never advances a previous surface. A different duration is a different profile, hence different keys; two histories are compared by running two profiles.

## 9. Determinism and canonical forms

Hard constraints (CON-07, CON-14, CON-29, CON-30, ADR 0010). The contract: given the same world definition, seed, profile and edit history, and the same kernel versions, a supported build on 1 or 8 threads produces identical canonical serialized results. Supported builds: **Windows (MSVC) and Linux on x86-64**. ARM and macOS are excluded until they are added to CI (CON-26). No GPU result enters any authoritative value. Bit equality must be shown by the actual builds, not inferred: **[gate G-DET]**.

| Source of divergence | Control | State |
|---|---|---|
| Platform maths | `libm`; `f64::sin`, `powf`, `mul_add` and the rest banned by clippy `disallowed-methods` (COORD-009) | enforced |
| Fused multiply-add | Rust does not contract `a * b + c`; `mul_add` banned | enforced |
| x87 excess precision | x86-64 uses SSE2 | by target |
| SIMD against scalar | the authoritative kernels are scalar (no `std::arch`, `target_feature` in `bake`, `erosion`). A SIMD kernel must equal the scalar one bit for bit, by test | true today |
| Reduction order, task order | fixed order; `par::map` returns in index order; tested on 1, 3 and 8 threads (`bake_rounds.rs`) | enforced |
| Denormals, rounding mode | the default control word on bake threads; nothing sets FTZ, DAZ or the rounding mode | by convention |
| NaN payloads | outputs asserted finite (ADR 0010) | enforced |
| `HashMap` order | used for lookups only (the memo), never iterated into a result | by review |
| Compiler version | CI uses `@stable`, no `rust-toolchain` file | **open:** a toolchain pin before the world build (proposal for the owner) |
| The test | CI's determinism manifest (`generate --manifest`, ADR 0010) covers tile hashes | **open:** the round solver and H1 are not in it; G-DET adds them |

| Contract | Rule |
|---|---|
| Graph topology and basin class | f64 comparisons as today (deterministic on the supported targets, no fast-math); ties by global cell id |
| Flood order | canonical (level, global cell id) (§6.4) |
| Canonical form | every value one stage hands to a later one has **one stored form**, and the later stage reads that form, in a fresh run as in a re-solve (below) |
| Reproducibility against correctness | bit equality of a re-solve and a fresh build is a test of determinism, not of correctness; correctness has its own gates (§12) |
| Edit replay | the same item list and the same order give the same key set on any run (ADR 0007, §7) |

Three representations, kept apart: the **canonical persistent form** (what is stored and reproduces a result), the **stage computation** (a kernel's working precision), and the **runtime composition** (f64, §3.1). Conversions happen only at the points named. A widening (f32 to f64) is exact; a narrowing rounds to nearest.

| Value | Canonical persistent form | Stage computation | Conversion point | Read by |
|---|---|---|---|---|
| Base `B` | f32 lattice node | f64 | the C1 read widens | all |
| Round surface (a stage output) | f32 height on the regional grid (0.25 m steps would flatten plains' slopes of 1e-4 at 814 m into false flats) | f64 inside the round; the routing and sweeps read it widened | narrowed once, at the end of a round | the next round, K6, K7 |
| Regional change into K7 | none: derived | f64: last round (widened) minus `B` at the regional node, then read C1 | at K7's read | K7 |
| K7's surface | none: scratch of the tile | f64 | narrowed to `L` | - |
| Local change `L` | i16 in steps of 0.25 m (kind 13) | f64 after the read | quantised at K7's end | K8 |
| Channel levels, basin levels, `Q_w` | f32 | f64 in K6 | narrowed at K6's end | K7, K11 |
| Filled surface, receivers, `A_d` | none: scratch of a round | f64, u32 | - | the round, K6 |
| Diffuse area code | u8 (§5.2) | - | one-way, at K6's end | materials only |
| Surface `S` | none | f64 (runtime composition) | to f32 at the mesh (CON-13) | K11 |

- Fresh builds and re-solves read the same canonical inputs at every stage.
- A round reads the previous round's **stored** form (f32), not an in-memory f64. Today's bake holds the carve in f64 and stores f32 heights for the post-pass only (`bake/tile.rs:322`); `solve_rounds` carries f64 across rounds. Under this contract a fresh build and a re-solve pass through the same form.
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
| Catchment-round key table and per-tile border summaries | - | to measure (a measured item of G-STORE) | |
| Edit overrides and continuity maps | - | to measure (a measured item of G-STORE) | |
| Temporary re-bake space; extra space while a replacement version is committed; versions retained for undo | - | to measure, each separately from steady state | |
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
| Lithology or uplift region | the driver map (GEN-122, GEN-125) | a coarse carve: one round at the H0 grid over the edit's region, with the new driver, shown as an overlay (no routing change, a response preview) | catchments under the region | as above |
| Rainfall or temperature region | the climate map (GEN-130) | H0 with the runoff change | catchments whose runoff changed | climate reach beyond the region |
| Class paint | weights preview | coarse carve as above | as height edit | as above |
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
| G-SURF | runtime minus bake surface at the bake's defined positions, the centres of the local cells | at most 1e-9 m before quantisation; at most half a step (0.125 m) after (GEN-154) |
| G-SURF-I (diagnostic) | the same difference at other positions, the store-lattice vertices included (the tile's core vertices, 170 × 170 at 102 m): p50, p95, p99, maximum, share over 0.25 m, error against local base slope or step height, spatial concentration, overshoot | reported; no systematic discontinuity or visible artefact, no uncontrolled overshoot. The clamp of the C1 read stays. GEN-148's tile: 0.66 % over 0.25 m, p99 0.22 m, maximum 2.55 m |
| G-CON | channels and conservation at every sampling: on H1's whole graph, every channel cell's D8 receiver is a channel cell or an outlet (a channel never ends mid-course, also where `A_d` falls below `T` and through a basin); every stored river piece ends at a lake entry, a confluence, a head or an outlet, and restarts at the spill cell (D24); `Q` at a confluence equals the sum of its donors plus the local term; sum of `Q` at the outlets against the sum of `a r`; sum of `Q_w` at the outlets plus the basins' losses against the sum of `a r`; `A_d` at outlets against the sum of `a w`, and `A_g` against the sum of `a`, across cube edges | 0 disconnected segments; relative error at most 1e-9 for every sum |
| G-PROFILE | §3.3: along every segment through the complete read `S` at the local grid's nodes, with `C` off and with `C` on; tile borders; confluences | with `C` off: no rise over 0.25 m, tile-border step at most 0.25 m. Rises between nodes and with `C` on are reported |
| G-SEAM | the canonical K7 layout's tile borders against equivalent interior cells, on steep mountains, low relief, narrow valleys and a large river (ADR 0119): height step, slope change, curvature or roughness across the border; river cut depth and valley cross-section where a channel crosses a border; any difference by distance to the halo edge. The alternate-layout difference (cells, p99, max, distance to seam) is reported | border p99 and max within the interior's p99 and max for each statistic; no concentration toward the halo edge; no seam visible in low-sun hillshade crops. Bars are revisited after the first reading |
| G-LOCAL | on the new kernels (clipped MFD, canonical flood): the round is a pure function of its key inputs: a catchment carved from the key inputs alone, in any order, against its part of the region round (§6.4 for why a catchment routed alone is not the test) | 0 cells differ |
| G-ROUND | the round solver against a whole-region carve **with the same new kernel rules** in the reference | 0 cells differ |
| G-EQ | a re-solve against a fresh build of the edited definition, E1-E7, on the **complete** result: the rounds, K6 (channels, `Q_w`, basins, levels), K7 and the sampled surface: bit equality. The canonical forms (f32 rounds, 0.25 m `L`) against an f64 run: relief, **and flood topology and initiation** (catchment membership, channel cells, basin count and classes) | bit-equal; relief within 2 %; membership and channel-cell status changed on at most 1 % of land and channel cells respectively; no connectivity change |
| G-INC | on E2 and each edit: the routing work (tiles re-flooded, graph solve, tiles of receivers and area recomputed) against a full region routing; the dirty catchment set found per round; the catchment keys recomputed | work reported per round; E2 at most 10 % of a full routing (F9: about 3 % of tiles); the level-2 job within §11.3 |
| G-BASIN | E3's merge, split, spill and outlet migration; a chain of basins; each row of §5.5's cases, with fixtures for high and zero `E_open` | ids and continuity maps follow §5.4; the levels and `q_out` of §5.5 to 1e-9 relative |
| G-SED | per catchment of the sweep's route and per round (the sweep on each round's result against the previous round's surface); the residual reported per catchment; thermal and pit spreading volume-conserving over the grid; the same scenario under different domain partitions | eroded = deposited + exported to 1e-9 relative; across partitions the totals are identical, no load lost or counted twice |
| G-LAND | relief p50 and hypsometry against the whole-region carve; sinks | within the seed noise floor; unclassified sinks at most 1 % of land (reported, not the main gate) |
| G-STORE | §10: real serialized data, including packs, indexes, keys, catchment-round key table, border summaries, edit overrides and continuity maps; temporary re-bake space, replacement-commit space and retained versions separately | 35 GB target, 40 GB hard, steady state; the others reported |
| G-SCALE | the complete resident build of the largest landmass (rounds, H1, sediment pass; §6.6's scenarios; process peak; time; flat-area cost); a partitioned route of it; the H0-split fallback, measuring divide migration and flow across its artificial borders; H0 at 2x radius; one large and one small body against §4.3's scaling model; the runtime's memory separately (BUDG-005) | the complete resident build within the world-build budget (BUDG-016: below the 12e9 B hard limit; below the 11e9 B working target by the A/B gate), measured; over it, §6.6's rule; partitioned equals resident (G-PART); the split measured; F6 x 4 within 1.5x; the model within 1.5x |
| G-RT | the region's flight | no held-back tiles; frame within BUDG-001/002 |
| G-MFD | the round path's area and breach rules (C) against unclipped region-wide MFD (U), and the boundary-flux exchange (F) against U, after the rounds, per fixture (ADR 0112 condition 1; SPIKE-107) | against U's noise floor (U under a second jitter seed): erosion displacement (abs dz) p95 and the relief and hypsometry statistics within the floor; channel-cell membership change and heads moved over 2 cells within the floor; outlet closure exact for a clipped area (U and F are not closed by construction); no visible line along catchment borders in hillshade crops, with the slope across borders reported |
| G-DET | 1 and 8 threads; Windows and Linux builds; resident and partitioned; edit replay; a fixture of the round solver and H1 in CI's determinism manifest | identical canonical serialized results |
| G-PART | §6.8's definitions: a partitioned route (tile floods from the neighbours' offers, cross-tile accumulation) against a resident route of the same region, **with runoff weights**; adversarial fixtures of §6.8; this is where routing locality is gated (receivers, filled levels, `A_d`) | bit-equal, also on the edited surfaces |

- Correctness is structural (SPIKE-105 §13.5): connectivity, conservation, seams, locality, relief. Closeness to one reference run is not a criterion (F4).
- **Every structural gate runs after each edit E1-E7 as well as on the fresh build.** A gate passed only on a generated world is not passed. **E7:** connect two landmasses by a bridge crossing the sea level, then separate them again.
- The Earth comparisons (SPIKE-102, SPIKE-104) stay reported values, not gates (R6).

### 12.2 Fixtures

Named landscape fixtures (the first review's suite), the prototype region and the edit cases E1-E6 are defined in `spikes/prototype-region/results/` (S105 §13.3); E7 in §12.1.

### 12.3 Who judges

The architect judges numeric gates (`docs/routing.md`). A failure of an architecture or requirement gate (G-LOCAL, G-ROUND, G-EQ, G-STORE, G-SCALE) and the A/B gate go to the owner (SPIKE-105 §13.5).

### 12.4 Conditions of the ADR review (2026-11-11)

ADRs 0110-0115 are accepted for the prototype (owner, 2026-11-11), with conditions from an external review. Acceptance is not production readiness. Each ADR lists its own conditions; across them, these close before the pipeline is called production-ready:

| Area | Condition | ADR | Priority |
|---|---|---|---|
| Owner decisions | SPIKE-105 §13.5's decisions 2-10 recorded as the owner's explicit choices: done, all at the recommended options (owner, 2026-11-11) | all | P0 |
| MFD | catchment-clipped MFD stays provisional until compared with unclipped region-wide MFD and a boundary-flux exchange (ADR 0112 condition 1) | 0112 | P0 |
| f32 rounds | the numerical and correctness gates re-run on the f32 round form, with near-degenerate fixtures (ADR 0114 condition 1) | 0114 | P0 |
| Determinism | toolchain, `Cargo.lock`, profile, flags, CPU features and floating-point settings pinned; Windows and Linux CI across thread counts and repeated runs; incremental equals fresh | 0114 | P0 |
| Thermal | pits and dams from thermal erosion: targeted fixtures, parameter and resolution sweeps | 0112 | P1 |
| Resources | memory headroom (BUDG-016's target); compaction peak and temporary disk measured; crash-safe replacement tested; smaller and larger bodies | 0115 | P1 |
| Editor | commit readiness domain, supersession and failure; undo and replay with basin splits and merges; measured latency of preview, regional re-solve and full rebuild | 0113 | P1 |
| Runtime | frame time on the reference machine (G-RT) | 0110 | P1 |
| Hydrology | connectivity, terminal basins, downstream connection, confluence conservation, outlets and water balance, also across cube faces and after edits that change landmass connectivity; the three channel levels and the balance's units (ADR 0111 conditions 1-2) | 0111 | P1 |
| Plausibility | fixtures for mountain divides, headwaters, low-relief plains, coastal mountains, endorheic basins (glacial valleys when supported); convergence or oscillation indicators per round | 0112 | P1 |

ADR 0116 (constitution v1.7) was not assessed by this review; a separate review is open.

## 13. ADRs (accepted for the prototype, 2026-11-11)

| ADR | Subject | Amends |
|---|---|---|
| 0110 | The authoritative surface | 0088 points 2, 4.2 and 5, 0099 point 3, 0100, 0101 point 3 |
| 0111 | Hydrology model and identity | 0041 (extends), 0101 points 2 and 3, 0105 points 2 and 3 |
| 0112 | The erosion solver: regions, rounds, catchments | 0088 point 4, 0099 |
| 0113 | Edit ordering and dependencies | 0007, 0066 points 1, 2 and 5, 0089 point 11; CON-08 v1.7 (ADR 0116) |
| 0114 | Simulation profile, cell metric, canonical forms | 0010 (forms), 0003 (metric use) |
| 0115 | Scale as a parameter and the store budget | 0045 point 2 (superseded) |

ADR 0109 (infill) and ADR 0105 point 1 (pass-1 deposition for pits) are replaced by 0112 if the gate passes: deposition moves to the post-round pass (§6.7), it is not dropped. Until the gate the accepted ADRs govern the shipped bake and 0110-0115 the prototype path (CON-07); the kernel-level changes (MFD clip, canonical flood) are within 0112.

Open items, now conditions of §12.4 or still open:
1. Whether clipped MFD changes the look (the kernels' area at divides): §12.4, MFD.
2. The cost of the canonical flood on large flats.
3. Whether a landmass as a region fits a world build; the split fallback's error.
4. The f32 canonical form of round surfaces against a run in f64: §12.4, f32 rounds.
5. Nested basins (§5.3) and basin losses in the carve's `A_d` (§5.5).
6. Protecting an intentional basin from the breach (§5.3).
7. A toolchain pin (§9): decided, §12.4 determinism.
8. A tile-wise route: built only under §6.6's rule.
