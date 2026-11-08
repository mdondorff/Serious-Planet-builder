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

<!-- next -->
