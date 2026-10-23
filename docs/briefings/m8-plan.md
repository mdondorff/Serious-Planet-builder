# M8 Natural landscape: plan (opus-planner, 2026-10-23)

Status: proposed; owner decisions D1 to D6 below. Inputs: SPIKE-28 to 38 (the architecture and process review with refactors R1 to R6: `docs/briefings/m8-review.md`), ADR 0076 (driver maps), ADR 0077 (stage model), `docs/briefings/m8-driver-maps.md`.

## Goal
The base layer, without instances, reaches the owner's Outerra reference image at sunset and midday from 300 m, 30 m and 1.8 m (owner, 2026-10-22).

## Exit criteria (proposed)
| # | Criterion | Tier |
|---|---|---|
| 1 | Owner's look review against the Outerra image at fixed poses (300 m, 30 m, 1.8 m; sunset, midday) on the 16384 store | D |
| 2 | Block fields: identical values at block borders; CPU results bit-exact across threads and operating systems | A |
| 3 | No frame waits for a block (CON-22); a block lands within 10 s at a 300 m pan; the switch fades in, with no step over 0.25 m p99 between LOD levels | A, C |
| 4 | Global coupled run at most 10 min on Rasierklinge per world version | perf |
| 5 | Shadows and the material map together at most 1.5 ms of terrain-pass GPU time at 1440p | perf |
| 6 | An edit over a block shows within the M6 budgets, and its eroded result within 10 s | A, perf |

## Steps
| # | Step | Depends on | Lane |
|---|---|---|---|
| R | Refactors R1 to R6 of `docs/briefings/m8-review.md` (R2 inside M8-2, R5 inside M8-1) | — | crusader, explorer |
| M8-1 | Global coupled stage: stream power and drainage at 2048, 70 to 100 steps; stored heights, drainage area, receivers; debug views (ADR 0030 point 2, ADR 0077 point 1) | — | crusader |
| M8-2 | Block field infrastructure: `Scope::Block`, key, apply level, plan-side requests, level-aware height (box-filtered pyramid), stand-in and fade, eviction; a stand-in compute (SPIKE-34) | — | explorer |
| S35 | SPIKE-35: A-net open items: halo below 5 km, a breach rule with every receiver downhill, drainage not aligned to grid axes, real threads | — | spikes |
| M8-3 | A-net block stage at 4 to 10 m: incision along the global network, diffusion, slumping (SPIKE-33) | M8-1, M8-2, S35 | explorer |
| M8-4 | Look review 1: engine renders of landforms at the fixed poses | M8-3 | owner |
| M8-5 | Horizon shadows per block plus a coarse horizon from the global run (SPIKE-32 step 2) | M8-2 | crusader |
| M8-6 | Driver maps per block (wetness from drainage area, soil depth) to material weights (ADR 0076; SPIKE-32 step 3) | M8-3 | explorer |
| M8-7 | Edits over blocks (D3) | M8-3 | crusader |
| M8-8 | Look review 2, perf window for criteria 4 to 6, acceptance | all | owner, planner |

## Owner decisions
| # | Decision | Recommendation |
|---|---|---|
| D1 | Accept ADR 0077 (blocks of a fixed grid; the global coupled run fixes the river network) | accept |
| D2 | CON-08: a reading of "per tile with a halo" as "per block of a fixed grid with a bounded halo", or a one-sentence amendment | amendment: the text then says what the code does |
| D3 | Edits over blocks: (1) edits apply after erosion, instant, erosion ignores them; (2) the edit shows at once, the eroded result follows within 10 s; (3) edited areas skip block erosion | 2 |
| D4 | The exit criteria above | accept |
| D5 | Lanes from 2026-10-25: explorer and crusader on M8, spikes on SPIKE-35 for its first day | accept; one main lane again if quota runs short |
| D6 | The photo layer (SPIKE-32 step 4, asset cache ADR): in M8 after criterion 1 is met, or in M9 | M9; M8 keeps the procedural look extensible |

## Risks
- The look: A-net's hillshades beat droplets and noise, but no shaded engine render exists yet; review 1 comes early for that reason.
- D8 rills along the grid axes and straight coarse grooves (SPIKE-33); S35 and M8-3 address them.
- Block latency of 6 s at 0.25 s per km2 assumed; A-net measured 0.06 s per km2 at 4 m.
