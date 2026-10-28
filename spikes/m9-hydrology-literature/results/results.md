Opened: 2026-10-28
Question: Does the M8 strategy (a runtime stream-power solve per 4.9 km chunk at 8 m along a frozen global river network, its change applied from level 15 over a base field) not fit this application, or was it implemented wrongly? What do real systems and the literature do instead?
Why it matters: opens M9 before any hydrology design (owner, 2026-10-28); M8's chunk approach failed the D0 stop rule.
Time box: 1.5 days
Success criteria: (1) survey with sources, measured vs claimed; (2) comparison against the constitution, LOD-013, ADR 0074, budgets, edit latency, store and bake; (3) 3 to 4 candidates with risks and cheap falsifying experiments; (4) which M8 failures are inherent to chunked runtime solves and which are implementation choices
Fallback if it fails: none (desk study)
Code location: none (research only)
Closed: 2026-10-28

## Method
- Four research lanes (Sonnet helpers, web): academic papers, engines and tools, LOD and tile consistency, editing and analytic erosion. Source checking, comparison and conclusions: Opus.
- Marks: **M** measured (numbers in the source), **C** claimed (qualitative), **I** inference of this study. Depth: **full** (full text read), **abs** (abstract or summary only), **helper** (full text read by a helper through the in-app browser; quote not re-read by Opus because the host blocks direct download).
- M8 evidence: SPIKE-31, 33 to 35, 50, 51; ADR 0074, 0077, 0081, 0084; the D0 verdict (`docs/milestones/M8/d0/README.md` on `origin/change/m8-group-d`); `docs/status.md` (gate level steps); `docs/background/architecture-report.md`.

## Headline answers
1. **The strategy was implemented correctly for what it guarantees, and that guarantee is not the one this application needs.** A frozen network makes neighbouring chunks agree exactly (SPIKE-35: 0 of 125,000 core cells differ at a 3 km halo). That is horizontal consistency. The failures are vertical (between LOD levels) and come from running the solve at runtime, where its result exists only at the levels that read ready chunks.
2. **No source runs a drainage-coupled solve per chunk at runtime under LOD.** Tiled drainage and erosion appear in the literature only as offline processing with a global stitch (Barnes 2016, 2017), as a single whole-domain solve that is then amplified or split (Schott 2023 and 2024, World Machine, Gaea), or not at all: rivers from a frozen network evaluated per point or per primitive (Génevaux 2013, Derzapf 2011, Cortial 2020). The project's own architecture report (§5) recommended "global coarse hydrology baked once per definition, then local amplification"; the runtime chunk solve entered with ADR 0077.
3. **Inherent to runtime chunk solves here:** the entry step and the level steps of the chunk pyramid, and the held-back tiles from a readiness-independent bounds margin. **Implementation choices:** the uphill rivers (finer relief not subordinate to the bed) and the D0 carve on the coarse polyline. **Separate decision:** WYSIWYG editing fails under any method that erodes after an edit.
4. **Once the coarse part is baked, the runtime solve adds little:** D0 at h=1 leaves detail of p99 0.19 to 0.45 m (max 4.5 m) for about 3.3 s per chunk on 4 threads (GEN-052).

## 1. Survey
### 1a. Papers
| Source | Method | Levels / LOD | Rivers across scales and borders | Edit locality | Cost | Depth |
|---|---|---|---|---|---|---|
| Génevaux et al. 2013, TOG 32(4) [1] | network first (Horton-Strahler, Rosgen profiles), terrain as a construction tree of primitives | C: "native multiresolution support"; an analytic continuous function | C: correct by construction; rivers subdivide upstream only | C: sub-regions evaluate alone (30 km2: 2,068 primitives, 225 kB) | M: each stage 0.1 to 5.3 s; about 1.5 kB per km2 | full |
| Derzapf et al. 2011, CGF 30(7) [2] | planet: coarse base mesh with continents and rivers, GPU view-dependent refinement | C: same result whatever the exploration path; seeds from edge vertices | C: river vertices interpolated between river end points, kept over levels | none | M: base mesh 0.27 s; 34 to 58 ms frames (GTX 580) | helper |
| Cortial et al. 2020, Vis. Comput. 36(10) [3] | planet: CPU network at about 50 km, GPU rule subdivision to about 50 cm | C: per-vertex seeds; ghost vertices, no cracks | C: new river vertex respects the flow; a new crest is higher than both adjacent rivers; tributaries only on 3 to 25 km edges | not tested | M: relief every 10 frames, 80 ms average (GTX 1080 Ti) | helper |
| Cordonnier et al. 2016, CGF 35(2) [4] | implicit stream power on a Poisson graph, landform kernels | C: a coarse-to-fine refinement "needs to be uniform, in order to preserve the possible emergence of local stream" | one global graph | global re-solve | M: 10k samples 6.4 s, 160k 252 s to converge (50 x 50 km) | full |
| Schott et al. 2023, TOG 42(5) [5] | uplift authoring, explicit stream power on a GPU grid, drainage by iterative relaxation | one resolution, 80 to 600 m cells; output "needs to be post-processed using amplification" | converges to all flow reaching the boundary | none: drainage is global | M: 1024^2 9.1 s, 4096^2 1036 s to converge (GTX 1080 Ti) | helper |
| Schott et al. 2024, TOG 43(4) [6] | amplification: upsample, then erosion per level, then a global multi-scale breaching | C: the levels "may still exhibit hydrological inconsistencies" until the breaching | whole map each step; patches beyond 8192^2 are an open "technical challenge" | none | M: erosion 19.2 ms per iteration at 8192^2 (RTX 3080) | helper |
| Tzathas et al. 2024, CGF 43(2) [7] | closed-form stream power along the downstream path; multigrid for the 2D field | multigrid is an accelerator, not LOD | a mode that keeps a given network | I: an uplift edit changes every cell upstream | M: 2048^2 in 34 s vs 4,799 s reference (1 thread) | helper |
| Vanek et al. 2011, IEEE CG&A 31(6) [8] | shallow water and thermal erosion on adaptive GPU tiles | C: interpolation error between unequal tiles; mesh cracks | C: neighbour reads give seamless water flow (local stencil, no drainage area) | C: only touched tiles | M: 21 to 25 ms per step | full |
| Barnes 2016, C&G 96 [9]; Barnes 2017, EMS 92 [10] | exact tiled depression filling and flow accumulation | n/a | exact with a global perimeter graph; no halo; directions fixed first | n/a | M: 2e12 cells in 24 min on 48 cores (2017) | full |
| Jain et al. 2024 FastFlow, CGF 43(7) [11] | GPU flow and depression routing | n/a | global | n/a | M: 5x flow, 34 to 52x depression routing vs prior work at 1024^2 | full |
| Bangerth 2026, arXiv 2606.12800 [12] | massively parallel flow routing | n/a | global | n/a | M: 1.88e9 points in 4.0 s on 12,288 processes | abs |
| Losasso and Hoppe 2004 [13]; Proland [14] | clipmaps / coarse + residual tiles | C: finer levels are built from coarser ones, never the reverse | n/a | Proland stores edits as residuals (C, not verified) | n/a | abs |

### 1b. Engines and tools
| System | Hydrology / erosion | Where it runs | Rivers across tiles and LOD | Depth |
|---|---|---|---|---|
| SpaceEngine [15] | noise "pseudo rivers": no inflows, can form loops | runtime, per point | consistent per point; no network | full |
| Outerra [16] | rivers from OpenStreetMap vectors with a bed profile; real DEM refined fractally | runtime from stored vectors | from data | abs |
| Horizon Zero Dawn [17] | runtime GPU placement only; terrain and rivers authored | editor bake | authored | abs |
| Far Cry 5 [18] | Houdini tools generate freshwater networks in the editor | editor bake | baked | abs |
| World Machine [19][20] | erosion in tiled builds "can produce different results"; developer: no "pixel-perfect results building only part of the map"; workaround: large-scale erosion as one build, then tiled detail | offline | overlap blending, never exact | full |
| Gaea [21] | tiled build with overlap (25 % default); TileGate bakes upstream at one resolution | offline | blending | full |
| Johansen 2026 erosion filter [22] | gradient-aligned gully octaves; height plus gradient; no drainage area; MPL-2.0 | runtime, per point | C: "trivial to generate in chunks"; streaks "just stop halfway down a mountainside" | full |
| Minecraft 1.18+ [23] | rivers from noise bands | runtime, per column | seamless by construction; no downhill guarantee | full |

### 1c. Common pattern
- **Global drainage is solved once (or authored, or taken from data) and stored; what runs per tile at runtime is a pure function of stored inputs and position**, so every level and every tile agree (C in the sources, I as a pattern).
- Exact tiled drainage needs a global exchange (perimeter graph) or a frozen network that supplies both inflow (upstream) and outlet heights (downstream) [10][7] (I).
- Rule-based planet methods keep rivers across levels with two rules: a new river point never rises downstream, and new ground beside a river never falls below it [3] (C).

## 2. Against this architecture
| Constraint | Runtime chunk solve (M8) | Bake + per-point amplification | Network refinement per level | Baked chunk solves |
|---|---|---|---|---|
| CON-14 determinism | met (SPIKE-35: bit-exact at 1, 4, 16 threads) | met if the bake has a fixed order | met with seeds from parents [2][3] | met |
| CON-11, CON-03 | met | met | met (CPU decides; a GPU path needs a CPU twin, CON-07) | met |
| LOD-013 (no level depends on readiness) | **conflict**: coarse levels cannot show the change; entry step 63 to 110 m | met: every level reads stored data | met: each level is a function of its parents | met |
| ADR 0074 morph room | **conflict**: a flat margin for an unknown change; 0 m holds back 0, 10 m holds tiles at 30 m altitude, 300 m holds up to 141 (SPIKE-51; 152 in the gate) | bounds exact from stored data plus a stated per-point amplitude | bounds from the rules | per-tile bounds stored |
| 1440p/60 fps | solves off the frame; 3.3 s per chunk on 4 threads | per-vertex cost in tile generation | Cortial: 80 ms per relief update on a GPU [3] | none at runtime |
| Edit visible within 10 s | chunk re-solves in reach; edit then changes (not WYSIWYG) | tiles re-evaluate; a network change needs a regional re-route | the edited subtree re-refines | re-bake the chunks in reach |
| Store (global16384, 600 m) and bake | coupled run about 20 s; chunks at runtime | 600 m exact streamed bake estimated 2.5 min on 16 threads; 100 m 1.5 h and 55 to 116 GB (SPIKE-31) | network only | 8 m over land: about 6.2e6 chunks x 3.3 s on 4 threads, about 240 days (I); 4.6 TB of raw 16-bit heights |

## 3. Candidates
| Id | Approach | What changes here | Risks | Cheap falsifying experiment |
|---|---|---|---|---|
| A | **Bake + per-point amplification** (architecture report §5): the coupled run with carve and stream power baked and stored; every level reads it; finer detail per point (erosion filter, noise); river channels as per-point primitives along the stored network | the chunk stage leaves the runtime; base = stored eroded field; a channel primitive with a monotone bed; detail clamped beside channels | rivers only as fine as the stored network (600 m); fine tributaries procedural; filter streaks end mid-slope [22] | (A1) the SPIKE-51 metric on a per-point channel primitive with the bed and bank clamp at site9 and the 48 chunks: 0 rises over 0.5 m expected; (A2) entry and level steps and held-back tiles with no chunk stage: 0 by construction (SPIKE-51: 0 at margin 0); (A3) a one-face 100 m bake: time and size against SPIKE-31's estimate |
| B | **Network refinement per level** [2][3]: each LOD level adds river vertices and tributaries under monotone rules, seeded from its parents | the tile generator becomes network-aware; drainage is a property of the refinement, not a solve | rule-based look, not physical; sparse networks (Cortial's own limit); a large rewrite | (B1) three levels of one tile chain with monotone river vertices: rises and level steps; cost per tile |
| C | **Baked chunk solves**: the A-net solve run offline, change pyramid stored, levels read it | no runtime solve; store per region | planet-wide at 8 m is about 240 days and 4.6 TB (I); fits authored regions only | (C1) bake 100 chunks: time, size, entry step 0, held-back 0 |
| D | **Runtime solve for sub-level detail only** (D0 at h=1 done right): coarse from the bake, chunks add only detail under 9.5 m | keeps the chunk stage with a small bound | 3.3 s per chunk for p99 0.19 to 0.45 m of detail (D0); rivers still rise (D0: 275 to 367 per 100 km) | (D1) the detail against A's per-point filter in look-review stills at the same sites |

- I: A is the pattern of every shipped system and of the project's own first plan; C is the fallback for authored regions; B is the research option if A's rivers look too coarse; D does not pay for its cost on the D0 numbers.

## 4. Inherent or implementation
| M8 failure | Evidence | Verdict | Why |
|---|---|---|---|
| Entry step 63 to 110 m (pop between levels) | SPIKE-50: p99 110 m today, 63 m with d1; SPIKE-34 predicted p99 25 m at 30 m amplitude, "morph does not hide it" | **inherent to runtime solves under LOD-013** | coarse levels cannot read a not-yet-ready solve, and the solve has km-scale content: D0's h=1 correction equals the whole change (p99 12.4 m random, 34 m site9). Not inherent to chunks: a baked chunk pyramid has no entry step. No LOD scheme in the survey adds content that only finer levels see [13][14] |
| Level steps 12/11 8.96 m p99 (gate) | `docs/status.md`: 12/11 8.961, 13/12 3.825, 14/13 1.850, 15/14 0.712 m (`anet_steps_report`, the chunk's own pyramid) | **inherent**, same root | the box-filtered change carries coarse content; the steps shrink only by removing it (D0 h=1) |
| Held-back tiles | SPIKE-51: no edits, margin 0 m holds 0 of 275 poses; 10 m holds 11 to 104 at 30 m and below; 300 m holds up to 141 (152 in the gate) | **inherent to runtime solves**; the flat margin is an implementation choice | bounds must hold before the solve, so they carry the stage's maximum; exact per-tile bounds need the change before drawing, which is a bake |
| Uphill rivers 99.6 to 227 per 100 km | SPIKE-51: 0 at cell centres; between centres from the chunk change against finer base relief; D0's polyline carve 275 to 367 | **implementation** | no rule keeps finer relief under the bed; [3] constrains river and crest heights; GIS stream burning shows the same failure when network and relief disagree [24] |
| Edits not WYSIWYG | owner test E12: a 400 m hill eroded to half | **separate decision** (D3 option 2) | uplift authoring is not WYSIWYG either [4][5]; editors erode first and apply edits after, or replay [21] |
| D0 stop rule | D0 verdict: level boundaries pass only at h=1, rivers fail in every form | **inherent** to D0's goal | removing the coarse content leaves a solve worth 0.2 to 0.45 m p99 |
| "Chunked solves are standard" | ADR 0077 context; architecture report §5 | **half true** | standard for horizontal tiling offline with a global stitch, or for local stencils [8][10]; no source keeps vertical (level) consistency with a runtime drainage solve |

## Findings
1. The M8 chunk stage met its horizontal guarantee (exact chunk borders) and failed on vertical consistency, which no surveyed method provides for runtime drainage solves.
2. Entry step, coarse level steps and held-back tiles follow from running the solve at runtime under LOD-013; a bake removes all three by construction.
3. Uphill rivers are fixable inside any approach with two rules from [3]: a river point never rises downstream; ground beside a river never falls below it.
4. WYSIWYG is a choice of order (erode, then apply edits), not a property of chunking.
5. Recommended first experiments: A1 and A2 (half a day each, existing probes); A3 if the 600 m network looks too coarse.
6. Limits: Schott 2023 and 2024, Tzathas 2024, Derzapf 2011 and Cortial 2020 rest on full texts read by a helper (quotes not re-read here); NMS, Star Citizen, Elite, Decima talks beyond abstracts not reached; no source on erosion on a cube-sphere; no shipped system reports runtime erosion cost.

## Sources
1. Génevaux et al. 2013: https://www.cs.purdue.edu/homes/bbenes/papers/Genevaux13ToG.pdf
2. Derzapf et al. 2011: https://cg.cs.uni-bonn.de/backend/v2/files/publications/derzapf-2011-river/pdf/derzapf_Planets_cd71999d4a.pdf
3. Cortial et al. 2020: https://hal.science/hal-02967067
4. Cordonnier et al. 2016: https://www.cs.purdue.edu/homes/bbenes/papers/Cordonier16CGF.pdf
5. Schott et al. 2023: https://hal.science/hal-04049125 ; code (MIT) https://github.com/H-Schott/StreamPowerErosion
6. Schott et al. 2024: https://hal.science/hal-04565030
7. Tzathas et al. 2024: https://hal.science/hal-04525371
8. Vanek et al. 2011: https://cs.purdue.edu/homes/bbenes/papers/Vanek11CGaApp.pdf
9. Barnes 2016: https://arxiv.org/pdf/1606.06204
10. Barnes 2017: https://arxiv.org/abs/1608.04431
11. Jain et al. 2024 (FastFlow): https://people.cs.uct.ac.za/~Jgain/wp-content/papercite-data/pdf/jain2024.pdf
12. Bangerth 2026: https://arxiv.org/abs/2606.12800
13. Losasso and Hoppe 2004: https://hhoppe.com/proj/geomclipmap/
14. Proland: https://proland.inrialpes.fr/doc/proland-4.0/terrain/html/index.html
15. SpaceEngine: https://spaceengine.org/news/blog131222/
16. Outerra: https://outerra.blogspot.com/search?q=river
17. Horizon Zero Dawn: https://www.guerrilla-games.com/read/gpu-based-procedural-placement-in-horizon-zero-dawn
18. Far Cry 5: https://blog.playstation.com/2018/03/22/the-procedural-world-generation-of-far-cry-5/
19. World Machine tiled builds: https://help.world-machine.com/topic/world-machine-professional-edition-addendum/
20. World Machine forum, 2015-02-12: https://forum.world-machine.com/t/better-erosion-support-in-big-maps/4082
21. Gaea tiled build: https://docs.quadspinner.com/Guide/Build/Tiled.html ; erosion: https://docs.quadspinner.com/Guide/Using-Gaea/Erosion.html
22. Johansen 2026: https://blog.runevision.com/2026/03/
23. Minecraft: https://minecraft.wiki/w/World_generation
24. AGREE stream burning: https://caee.utexas.edu/prof/maidment/gishyd97/terrain/agree/agree.htm

## Visuals
Nothing to look at: a desk study, no renders.
