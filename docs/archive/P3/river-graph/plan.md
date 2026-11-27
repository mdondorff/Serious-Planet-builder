# River graphs across scales: plan

Status: draft 4 (lead, 2026-11-16). Draft 3 passed two Fable rounds (`critique-fable-1.md`, `critique-fable-2.md`). Draft 4 takes in Agt C's revision (`plan-review-agc.pdf`, `g0-contract-addendum.md`; the addendum is part of this plan and wins where the two differ).
Owner, 2026-11-16: G0 approved; the pre-work and G1a continue; no KS, KG, KV or KC implementation until the graph and budget contracts (G0) are agreed.
Three levels of evidence, each separate (addendum §1): a component works; the chain works; the architecture is selected. Passing one does not imply the next.
Sources:
- Review A: "Critical review: river architecture across scales" (owner's Downloads, 2026-11-16). It addresses review v3 and is the "Idea: River Graphs" note that review B evaluates.
- Review B: Antigravity, `ag_serious-planet-builder/reviews/2026-11-16-river-review-v4.md`, on review v4 and the proposed ADRs 0124-0127.
- Owner, 2026-11-16: strong support for river graphs at several scales; the fuller graph approach first, then the alternatives and the sub-details; no algorithm tuning yet; look for the better overall solution.

## 1. Terms
| Term | Definition |
|---|---|
| `z_b` | the base B sampled at 814 m cell centres (K5's input) |
| `z_s` | the regional surface after K5 at 814 m |
| `r` | the base's 101.8-814 m band: `r = B - C1(z_b)` at L |
| `S0` | the unconditioned start surface: `S0 = C1(z_s) + r = B + C1(z_s - z_b)`; the reference of every delta measure, for B and RG alike |
| sill | of an 814 m edge: the minimax height over L cells of `S0` on a path between the two cells' lowest L cells, inside the two cells |
| `H` (HAND) | a K6' field at 814 m: a cell's height over the H1 channel cell its 814 m flow path reaches; read at L through C1, so near 0 along the trunk line and rising across its cell |
| `H_f` | fine HAND by flow path: `KS0(x) - bed(first graph cell on x's receiver chain)` on KG-trib's routing of KS0; `H_f <= H` |
| `c_cut` | the lowering cap: `min(50, max(10, 0.5 × relief10))` m (the D-1 corridor bar); R1h 27.5 m, R3a 10 m |
| bed | of a graph line: the bounded two-sided correction of KS0 along it: pits become ponds (SPIKE-126's rule), rises are cut by at most `c_cut`; a rise over `c_cut` is listed as a D-2 reach, never cut |
| divide | the boundary of a KG-trib catchment |
| KS | conditioned start surface: `z = C1(z_s) + r'`, `r' = min(r, α·H)` (KS0) or `min(r, α·H_f)` (KS1); `r <= 0` kept |
| KG | the fine graph: H1 trunks drawn at L, tributaries down to `A_crit`, L-scale basins |
| KV | valley construction around the stored beds |
| KC | bounded correction: fills or breaches the pits that are not basin nodes, at most `c_cut` deep, every patch counted |
Terrain-change budget (addendum §2.5): per cell, `KS_used + KV_used + KC_used <= c_cut` against `S0`, consumed in stage order; each stage uses only the remainder. KS's formula alone does not bound its lowering (`S0 - KS0 = max(0, r - α·H)`), so KS enforces the remaining budget explicitly and records every cell where the conditioning cannot be met as a residual. The cap is never exceeded silently and never raised to make a stage pass.

## 2. What the reviews say
| Point | Review A | Review B | Consequence |
|---|---|---|---|
| Authority of rivers | an explicit river and basin graph at landmass scale; the raster route is a validation tool | the same thesis; start it at H1 (814 m), keep K0-K5 | RG (§3) |
| Terrain near rivers | built for the graph: valley envelopes from discharge, relief and geology; bounded correction | R2 valley stamping with divide protection; R3 the band scaled by HAND | KS, KV, KC |
| H1 blind spots | - | R1: H1 routes on sub-cell sills (on B; this plan uses `S0`, §3.3) | K6' |
| Lakes | basin hierarchy, Fill-Spill-Merge, early | L-scale lakes from 0.25 km2 and 2 m; Earth's pit share an upper bound (2-10 %) | basin graph in K6' and KG; ADR 0127 amended |
| Runtime | - | R4: the channel drawn from kind 20; L holds the valley floor | proposal in G3, built in P4 |
| D-1 | keep the L round as benchmark and fallback | accept B now | owner: graph first; B is benchmark and fallback; ADR 0124 stays proposed |
| ADR 0126 | regions consume a global graph | approve as written | holds under RG and under the fallback |

### Review A's experiment list against P3
| Item | State |
|---|---|
| Breach only against carve only | done (E2, SPIKE-120) |
| Scale-dependent K | done (B, SPIKE-121) |
| Topology-preserving prolongation | run as D and E; not falsified: review B F1 traces their ridge cut to two implementation choices (valley-diagonal split of non-flow quads, damping of `r > 0`); the constrained (ANUDEM, skeleton-only) form was not run. KS is its graph form |
| Multi-scale breach alone on the C1 surface | not run (D and E ran it after their rebuild); KC is its bounded form |
| Schott fair, mountain window | done (F) |
| Schott with retargeting or scale-consistent uplift | not run; parked |
| Graph prototype, graph-conditioned valleys, constrained reconstruction | this plan |
| Synthetic architecture fixture | G1d |

## 3. The candidate architecture RG
### 3.1 The graph-first ladder
| Rung | Tributaries | Band | Role |
|---|---|---|---|
| RG-0 | as RG-1 | `S0` (KS off) | free control: what the clip buys |
| RG-1 | extracted by one routing of KS0 at L, then frozen (the rediscovery loop's step 3, run once) | KS0 | intermediate of RG-2, measured for free |
| RG-2 | as RG-1 | KS1 (`H_f` by flow path on the fine graph); KV on KS1 | **the prototype (the fuller form)** |
| RG-3 | generated on the H1 graph (Horton ratios, junction angles), KS as the cost field; extraction as validator | KS1 | next rung; a swap of KG-trib behind its output contract |

KG-trib's output contract (fixed now, so RG-3 is a swap): lines with drained area `A`, discharge `Q`, Strahler order, bed profile, owning tile, id.

### 3.2 Stages of RG-2
| Stage | Domain | Output | Replaces |
|---|---|---|---|
| K0-K4 | landmass, 814 m | base, climate | - |
| K5 (or K5') | landmass, 814 m | `z_s`; K5' routes each round on the sill edges (static per-edge minimax of `r` plus the round's `z_s`) | K5 on point samples, if G1a shows trunks leaving K5's channels |
| K6' | landmass, 814 m | H1 trunk graph (>= 75 km2) and 814 m basin graph on node heights and sill edges; `H` | K6's point-sampled routing |
| KG-trunk | landmass, sparse | trunk centrelines at L inside the corridor on KS0 (KR's main-stem pass, E4 F2.1-F2.3); beds; `Q`; spill levels; D-2 list; stored in kind 20 | ADR 0125 points 2-3 |
| KS0 | K7 tile + halo | `C1(z_s) + min(r, α·H)` | `S0` as the start |
| KG-trib | K7 tile + halo | tributaries down to `A_crit` by canonical flood plus clipped MFD (GEN-141) on KS0, trunk cells forced receivers (not a burn); area at the halo border seeded from H1's diffuse `A_d` (40-75 km2 tributaries exceed the 13 km halo); ids as GEN-145; a tributary crossing a tile border belongs to the tile of its downstream end (GEN-182's rule); beds; L-scale basin nodes (>= 0.25 km2, >= 2 m) | K7's tributary routing |
| KS1 | K7 tile + halo | `C1(z_s) + min(r, α·H_f)`; `KS1 <= KS0` | KS0's strip along trunks |
| KV | K7 tile + halo | L: cross-sections around the beds, wall slope `S_wall`, floor `W_floor(Q)`, weight zero within `d_buf` of a divide; lakes flat at their spill level; lowering capped (§1) | the D-1 round; K7's burn and pass-2 carve |
| KC | K7 tile + halo | pits that are not basin nodes filled or breached (at most `c_cut`); patches counted | breach and thermal as river makers |
| K7 remainder | - | not run in the prototype: no thermal, deposition, pit spreading or final channel pass (owner decision 5) | - |
| runtime | - | the current read in the prototype; R4 proposed in G3 | - |

The raster route of the final L is a gate, never a source.

### 3.3 Design facts
- CON-08: K5', K6' read B and K5 outputs only; KG reads KS0, which reads H1; KS1 reads KG. One direction, no loop.
- Sills on `S0`, not B: K5's carving (`z_s - z_b`, up to hundreds of m) is in `S0`; sills on B would route H1 around valleys K5 cut.
- K5 routes on `z_b` at cell centres (`region_bake.rs:206`); with K6' alone, trunks can leave K5's carved valleys. G1a measures the share; above 10 % of trunk cells, K5' joins RG.
- `H` is tile-local through C1 (a landmass field like `z_s`). KS0 clears positive `r` near trunk lines (the strip); KS1 narrows it to the fine lines.
- KS keeps `r <= 0` (its pits become ponds, basin nodes or KC patches) and clips only positive band that would dam. `H_f` follows flow paths, so it never reaches across a divide (review B F1's mechanism avoided).
- KS guarantees drainage only where `H = 0`; elsewhere G-DRAIN is a gate.
- Beds, `Q` and spill levels are graph products (trunks at landmass scale, tributaries per owning tile), so KV stays tile-local.
- Edits (owner, 2026-11-07): a base edit dirties the sill field and `S0` locally; a change of `z_s` or of the sills dirties K6'; the H1 graph diff dirties KG-trunk and the touched tiles. A local edit that leaves H1 unchanged redoes only its tiles.
- Deep cuts are a fault (owner, 2026-11-13): no lowering over `c_cut`; a larger rise is a listed D-2 reach.
- Cliffs stay a separate layer (owner, 2026-11-01).

### 3.4 Parameters (one value each; no sweeps)
| Parameter | Value | Reason |
|---|---|---|
| `α` | 1 | the band never rises above the height over the channel |
| `A_crit` | 1 km2 (96 L cells) | K7's local carve area today (its upper bound) |
| `S_wall` | 0.20 (20 m per L cell) | the 204 m ring stays at 41 m, under the 50 m bar; 0.25 reaches 50 m |
| `W_floor(Q)` | 3 × the GEN-165 channel width | floodplain wider than the channel |
| `d_buf` | 200 m | review B R2 (2-3 L cells) |
| `c_cut` | `min(50, max(10, 0.5 × relief10))` m | the D-1 corridor bar |
| `k_c` | 2 | P3 D-1 rules |

### 3.5 Cost
| Item | Estimate |
|---|---|
| Sill field | 64 base samples per 814 m cell: about 9.7e9 base evaluations per face (the count of K7's whole base read; time per face measured in G1a); cached per edge, about 0.7 GB per face at 30 % land |
| KG-trunk | sparse over trunk cells; landmass resident |
| KS, KG-trib, KV, KC | per K7 tile + halo; memory independent of the landmass |

## 4. Programme
Ordered by risk, not by pipeline (addendum §3). Each milestone has its own evidence and exit; the details of every row are in the addendum.
| Milestone | Scope | Exit | Lane | Dates (estimate) |
|---|---|---|---|---|
| Pre | Review B F2 coastal snap fixed (`river_suite.rs:734`), the 42 rows and the D-2 list re-read; density with a threshold parameter and Strahler `R_b` on the GLO-30 tiles and the five `S0` windows at 1 km2 | baselines and known issues recorded; the density bar fixed before any RG result | explorer | 11-17 |
| G0 | Contracts (graph, reach, basin, rasterisation, budget, determinism, partition, edit) as a note; then serialisation, validators and the fixture of addendum §2.7 | addendum §2 exit: contracts agreed by the owner; validators catch injected invalid edges, cycles, duplicate ids, open endpoints, budget overruns; fixture runs under two partition layouts; thresholds fixed | lead (note); crusader (fixture terrain now; harness after agreement) | note 11-17; harness 11-18 to 11-19 |
| G1a | Sills of `S0`; H1 re-routed on five windows, R1h first; plus topology diff, sensitivity to small perturbations of `S0` and sills, the exception map | a connected trunk graph with exceptions explicit; K5' only if major trunks are unstable or off K5's channels | spikes | 11-17 to 11-19 |
| G1b | KS0, KS1 in isolation on known graph lines; RG-0/1/2 | lowering distribution against `c_cut`, per-stage budget, clipped band, new pits, residuals; broad or deep change stops KS and opens constrained reconstruction | explorer | after G0 |
| G1c | KG-trunk and its rasterisation, no tributaries | continuous profiles, stable ids, boundary and partition equivalence, no L erosion round | spikes | after G0 |
| G1d | KG-trib, extractor-1 (flood plus clipped MFD on KS0) behind the reach contract | no invalid, dangling or duplicate reaches; conservation; hierarchy and density; halo cases; poor network: RG-3 behind the same contract | crusader | after G1c |
| G1e | Basin hierarchy and protected basin surfaces | nested, shared-spill, endorheic, cross-tile cases; ids and spills stable under partition | crusader or spikes | after G0 |
| G1f | KV and KC, separately switchable (KV only, KC only, both) | budget enforced; fixture passes; correctness and partition gates before G2 | explorer | after G1b-G1e |
| G2 | Integrated RG-2 on five windows against B | §4.1 by gate class; cost and memory; partition and edit tests; owner flight | explorer (gate), owner | about 11-25 |
| G3 | Architecture decision: adopt RG, a targeted second prototype, partial adoption, or B with refinements | decision record per component; production ADRs only then | lead | after G2 |

Lanes start a milestone only once its contract is agreed (addendum §6); no lane redefines graph or bed semantics. Total: about 20 PRs; dates after G0 depend on the agreement.

### 4.1 Success criteria (set before any RG result; every delta against `S0`)
Every result carries its gate class (addendum §4): correctness, partition invariance, incremental correctness, preservation, geomorphology, product, owner judgement. A pass in one class never covers a failure in another; a flight never covers a partition failure. Raster G-DRAIN and graph validity stay separate. Added graph reports: trunk continuity and outlet connectivity, junction conservation, bed-profile residual and exception count, graph-to-raster agreement, tributary attachment and hierarchy, basin parent/child and spill consistency, boundary graph diff, stable-id diff after rebuild and edit. G0 fixes each metric's operational definition (basin for G-DRAIN and its denominator, saddle area, valley width on asymmetric sections, GLO-30 matching, graph/raster tolerance, per-cell or per-reach or area-weighted) before any RG result.
Gates on all five windows:
| Criterion | Bar |
|---|---|
| G-DRAIN on the whole surface, no round at L | basins <= 10 % |
| Trunk corridor share on the final L against H1 (BUILD-057, `k_c` 2, snapped), the same measure as B's 97.86 / 95.07 % | the owner's share (review B: 97.5 % outside mountains, 95 % on M1) |
| Bed non-increasing along graph lines outside lake and waterfall nodes; the D-2 list (rises over `c_cut`) | reported per window, against B's list |
| G-INCISION | the existing bars |
| Cut-through saddles (1 km2 and 5 km2), ground lowered > 50 m (total and per stage), relief lost | each <= B's |
| Crest rounding: p95 ridge lowering | <= 15 m hilly, <= 35 m mountain (review B Q5) |
| Cross-sections T1-T3 against GLO-30 at matched relief (P3's E3 bar) | acceptable to the owner |
| Valley width at 10 m and 50 m depth | inside the GLO-30 range of the matched tiles |
| Relief spectrum slope | within the benchmark's range; loss against `S0` no larger than B's |
| Fine-graph density and `R_b` at `A_crit` | inside the bar set by the pre-phase (11-17), before any RG result |
| KC patches | count and volume reported; none deeper than `c_cut` |
| Tile-locality | one window baked as one and as two regions: same bits on shared tiles |
| One ridge edit end to end | dirty set, time, memory; incremental equals fresh (CON-29) |
| Cost per window | <= B's |
| Owner's flight | RG-2 at least as plausible as B |
Reported only: the fine graph's self-consistency (KS1/KV routes onto its own lines).

### 4.2 Early stops, fallbacks and partial adoption
Addendum §5's decision table holds. In short:
| Signal | Step |
|---|---|
| G1a: re-routing changes major trunk topology, or over 10 % of trunk cells leave K5's channels | K5' investigated before the chain |
| G1a: under half of the D-2 list gains a route | KG-trunk's demotion and widening (review B F3 c, a) become the main path |
| G1a: sills too costly or unstable | sparse evaluation, caching or another representation |
| G1b: KS exceeds the budget, or the strip flattens the look | constrained reconstruction (review A §3.3) as a separate prototype; the cap is not raised |
| G1d: tributary network poor, trunk sound | RG-3 behind the same contract; KV is not tuned to compensate |
| G1e: basin topology unstable | basin work kept separate; pits are not all made lakes |
| G1f or G2: valleys look stamped, graph sound | KV revised alone |
| Partition invariance fails | integration stops; the contracts are fixed |
| RG-2 fails preservation or the flight | the passing components stay as experiments; compared against B. A failed RG-2 does not reject the graph architecture; the decision record names what passed and what failed |
| Fallback | B with review B's refinements (F2, F3's order, K7's `A_crit`, F5's execution); ADRs 0124-0127 amended |

## 5. Disposition of every review item
| Item | Disposition |
|---|---|
| B F1 (D/E ridge-cut mechanism) | recorded in SPIKE-122's note; KS avoids it (positive damming `r` only, `H_f` by flow path) |
| B F2 (coastal snap, 97 reaches) | fixed first (pre) |
| B F3 (D-2 repair) | pre-K6 sills = K6' on `S0` (G1a); demotion and widening are KG-trunk's fallback |
| B F4, Q4 (K7 carve without `A_crit`) | RG: KV replaces the carve, K7's remainder not run; fallback: `A_crit` in K7 |
| B F5 (landmass execution of the L round) | RG has no L round; tile-locality is a gate; fallback: F5's two forms |
| B F6, Q10 (runtime read, coast fade) | with R4 in G3 (proposal); built in P4; coast fade moves into the bake |
| B Q1 (split D-1 / D-2) | RG answers it differently: no D-1 round; H1 agreement through K6' and KG |
| B Q2 (ANUDEM-style hand-over) | KS is its graph form |
| B Q3 (repair order) | as F3 |
| B Q5 (crest-rounding bar) | §4.1 |
| B Q6 (saddle proxy) | kept, 5 km2 beside 1 km2 |
| B Q7 (pass share) | the owner's bar in §4.1 |
| B Q8 (pit share an upper bound) | ADR 0127 point 6's input |
| B Q9 (L-scale lakes) | KG-trib's basin nodes; ADR 0127 point 3 amended |
| B ADR 0125 refinement 3 (pin then fade) | with R4 in G3 |
| B R1-R4 | K6' (on `S0`), KV, KS, R4 proposal |
| A §3.1 global graph | RG |
| A §3.2 valley envelopes | KV |
| A §3.3 constrained reconstruction | named alternative (§4.2) |
| A §4A Schott retargeting and uplift | parked; F stays the benchmark |
| A §4B, §4C prolongation, multi-scale breach | §2 table: not falsified; KS and KC are their forms in RG |
| A §4D MFD for hillslopes | KG-trib's area uses the clipped MFD (a change: K7 runs D8 today) |
| A §6 Fill-Spill-Merge | basin graph; ADR 0127 |
| A §9.1, §9.3 | §4.1 graph and preservation gates |
| A §9.4 reference set | parked (provisional benchmark) |

## 6. Owner decisions
| # | Decision | State |
|---|---|---|
| 1 | G0 first; the pre-work and G1a continue; no KS, KG, KV, KC code before the contracts | approved (owner, 2026-11-16) |
| 2 | The G0 contracts (`g0-contracts.md`) | agreed (owner, 2026-11-16, after Fable's check `critique-fable-g0.md`) |
| 3 | The success criteria (§4.1), with the corridor share | open, with G0 |
| 4 | KS changes the start surface's band near rivers, inside the budget | open, with G0 |
| 5 | `S0` as the reference for every delta | open, with G0 |
| 6 | K7's thermal, deposition and final pass not run under RG | open, with G0 |
| 7 | P3's exit list: E3 closes as "benchmark measured, no pick"; E6 "river graphs: G0 to G3" | done with the approval of 1 |

## 7. Explicitly deferred (addendum §7)
Dynamic lake water balance, evaporation and time-varying levels; a full Horton generator unless extractor-1 fails; meandering and migration; floodplain deposition and sediment transport; runtime optimisation beyond a correct read; general incremental graph editing beyond one representative edit and the invalidation contract. Graph identity and invalidation are designed now.

## 8. Results so far (lead, 2026-11-17)
| Item | Result | Consequence |
|---|---|---|
| Pre: coastal snap (#1562) | 146 of 711 D-2 reaches were snap artefacts | the D-2 list is 565 |
| G0 (#1561, #1563, #1564) | contracts agreed; fixture, types, serialisation, validators, both layouts | G0 passes: the density and `R_b` bar is fixed (BUILD-059, #1574, `spikes/river-suite/results/network-bar.md`; N35 W076 left out of R3a/R3b's 75 km2 floor, too little land) |
| G1a, SPIKE-127 (#1566) | K6' on sills: no real change of the top-10 rivers (outlet > 8 km or a tributary to another river) on four windows, R3a at noise; 59 % of the remaining D-2 reaches gain a route on B; sills do not clear `S0`'s dams | K6' is the trunk base |
| SPIKE-131 (#1569) | K5' re-routes 2-4 top rivers really, erodes 16-49 % less, passes fewer corridors | K5' not adopted; K5 stays |
| Stop test definition | "major trunk change" counts real changes only (outlet moved > 8 km, or a tributary joining another river); a confluence moved along the same river is not a change | applies to all later G1 stop tests |
| Next | G1c (#132) on K6''s trunks with today's K5 (DEC spikes-3, A); G1b (explorer #164); G1e (crusader #156) | |
| SPIKE-134 (#1580) | the basins on trunks come from the base's 814 m depressions and, on M1, from K5; the sill-routed H1 adds band dams | owner decision 5 (base or K5 fix); today's K6 the trunk base meanwhile (proposed) |
| SPIKE-135 (#1583) | `S0`'s debt: 77-93 % of breach cells on four windows from reading `z_s` at L, base 6-10 % (F1 61 %), band 1.4-13 %, K5 0-1.1 %; a drained hand-over removes 68-97 % of breach volume and 54-97 % of D-2 failures; 36-92 % of KS's lowering lies away from breach need | **proposed amendment: a hand-over stage KH (a drainage-preserving read of `z_s` at L) replaces `C1(z_s)` under KS; SPIKE-136 picks its form** |
