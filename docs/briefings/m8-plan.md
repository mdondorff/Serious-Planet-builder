# M8 Natural landscape: plan (opus-planner, 2026-10-23)

Status: proposed; owner decisions D1 to D6 open, D7 decided. Inputs: SPIKE-28 to 39 (the architecture and process review with refactors R1 to R6: `docs/briefings/m8-review.md`), ADR 0076 (driver maps), ADR 0077 (stage model), `docs/briefings/m8-driver-maps.md`, terms: `docs/domain.md` (chunk: ADR 0077's regional square).

## Goal
The base layer, without instances, reaches the owner's Outerra reference image at sunset and midday from 300 m, 30 m and 1.8 m (owner, 2026-10-22).

## Exit criteria (proposed)
| # | Criterion | Tier |
|---|---|---|
| 1 | Owner's look review against the Outerra image at fixed poses (300 m, 30 m, 1.8 m; sunset, midday) on the 16384 store | D |
| 2 | Chunk fields: identical values at chunk borders; CPU results bit-exact across threads and operating systems | A |
| 3 | No frame waits for a chunk (CON-22); a chunk lands within 10 s at a 300 m pan; the switch fades in, with no step over 0.25 m p99 between LOD levels | A, C |
| 4 | Global coupled run at most 10 min on Rasierklinge per world version | perf |
| 5 | Shadows and the material map together at most 1.5 ms of terrain-pass GPU time at 1440p | perf |
| 6 | An edit over a chunk shows within the M6 budgets, and its eroded result within 10 s | A, perf |
| 7 | Speed envelope (owner, 2026-10-23): a perf scene flies level over unvisited ground at fixed altitudes (30 m, 100 m, 300 m, 1 km) and raises the speed until the stand-in share of the screen at the chunk's apply level exceeds a bound; it reports the highest speed per altitude on Rasierklinge. M8 records the curve; the owner sets the floor from it; later milestones keep it as a regression check | perf |

## Steps
| # | Step | Depends on | Lane |
|---|---|---|---|
| R6, R4 | Small cleanup and the core helpers (many files, short PRs) | — | crusader, day 1 |
| R1, R2 | One home for the field chain; the `HeightField` split with the level-aware height | R4 | explorer |
| M8-1 | Global coupled stage with R5: stream power and drainage at 2048, 70 to 100 steps; stored heights, drainage area, receivers; debug views (ADR 0030 point 2, ADR 0077 point 1) | — | crusader |
| M8-2 | Chunk field infrastructure: `Scope::Chunk`, key, apply level, plan-side requests, box-filtered pyramid, stand-in and fade, eviction, prefetch along the direction of travel; a stand-in compute (SPIKE-34) | R1, R2 | explorer |
| S35 | SPIKE-35: A-net open items: halo below 5 km, a breach rule with every receiver downhill, drainage not aligned to grid axes, real threads | — | spikes |
| M8-3 | A-net chunk stage at 4 to 10 m: incision along the global network, diffusion, slumping (SPIKE-33) | M8-1, M8-2, S35 | explorer |
| M8-4 | Look review 1: engine renders of landforms at the fixed poses | M8-3 | owner |
| M8-5 | Horizon shadows per chunk plus a coarse horizon from the global run (SPIKE-32 step 2) | M8-2 | crusader |
| M8-6 | Driver maps per chunk (wetness from drainage area, soil depth) to material weights (ADR 0076; SPIKE-32 step 3) | M8-3 | explorer |
| M8-7 | Edits over chunks (D3), with R3 first | M8-3 | crusader |
| M8-V | The speed-envelope scene (criterion 7) | M8-2 | crusader |
| M8-8 | Look review 2, perf window for criteria 4 to 7, acceptance | all | owner, planner |

## Lanes and files (to avoid merge conflicts)
- explorer owns the field chain and the chunk path: `generators/height*.rs`, `generators/stage.rs`, `app/worldfield.rs`, `frame/lod.rs`, `streaming`.
- crusader owns the global stage and the stored channels (`generators/global.rs`, `cache/global_store.rs`), then `render` (shadows) and the perf scene.
- Shared files (`lib.rs` module lines, specs) change in small PRs; each lane merges main into its branch at least daily; a PR is at most about a day of work.
- R4 and R6 touch many files and run on day 1 while explorer writes the M8-2 proposal without code.

## Owner decisions
| # | Decision | Recommendation |
|---|---|---|
| D1 | Accept ADR 0077 (chunks of a fixed grid; the global coupled run fixes the river network) | accept |
| D2 | CON-08: a reading of "per tile with a halo" as "per chunk of a fixed grid with a bounded halo", or a one-sentence amendment | amendment: the text then says what the code does |
| D3 | Edits over chunks: (1) edits apply after erosion, instant, erosion ignores them; (2) the edit shows at once, the eroded result follows within 10 s; (3) edited areas skip chunk erosion. Affects the editor only: a viewer receives a finished world | 2 |
| D4 | The exit criteria above | accept |
| D5 | Lanes from 2026-10-25 as in "Lanes and files" | accept; one main lane again if quota runs short |
| D6 | The photo layer (SPIKE-32 step 4, asset cache ADR): in M8 after criterion 1 is met, or in M9 | M9; M8 keeps the procedural look extensible |
| D7 | How the viewer gets chunks | **decided (owner, 2026-10-23): at runtime** (generated on demand, prefetched, cached on disk); pre-baking an area offline is added later if criterion 7 shows a gap worth filling |

## Risks
- The look: A-net's hillshades beat droplets and noise, but no shaded engine render exists yet; review 1 comes early for that reason.
- D8 rills along the grid axes and straight coarse grooves (SPIKE-33); S35 and M8-3 address them.
- Detail arriving late when flying fast and low on a first visit; criterion 7 measures it.
- Chunk latency of 6 s at 0.25 s per km2 assumed; A-net measured 0.06 s per km2 at 4 m.
