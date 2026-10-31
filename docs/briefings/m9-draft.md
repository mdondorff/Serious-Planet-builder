# M9 Realistic terrain: draft plan
Status: draft for the owner's decision after the away week (drafted by the spikes session for opus-planner, 2026-10-30)
Sources: roadmap M9 and its design constraints (2026-10-28); `docs/briefings/week-2026-11-02.md` section 3 (R1 to R8); SPIKE-55, SPIKE-56, SPIKE-57, SPIKE-58, SPIKE-60; `openspec/changes/shadow-clipmap-march` (#941), `openspec/changes/terrain-scorecard` (#942); owner test 2026-10-27 (`docs/milestones/M8/owner-test-2026-10-27/README.md`); ADR 0076 (open), ADR 0084 (not pursued), ADR 0085, ADR 0086, ADR 0087; M8 report, improvement 4 (a stop rule in every criterion that depends on a new subsystem).

## Goal
- Hill and mountain terrain that reads as real at 300 m, 30 m and 1.8 m against the owner's look reference, measured against GLO-30 (scorecard) and judged by the owner.
- Rivers that run downhill in valleys that follow the ground.
- Terrain shadows from the clipmap march (ADR 0085); the horizon maps retired.

## Starting point
- Hills and mountains are 4-9x too gentle in slope and relief; the Badlands match; no generator cell reaches Alpine spread (SPIKE-55).
- The coarse network does not follow the ground: a monotone bed along straight 4.3 km segments cuts 30 m p50 past the valley law (site9: 116 m); lowland valleys come out deeper than the land is high (SPIKE-60).
- A per-point channel is monotone by construction and adds no level step and no held-back tile (SPIKE-60 A1, A2).
- Erosion kernels (thermal, deposition) fit only the bake on the CPU; fluvial erosion per region needs the whole drainage (SPIKE-57).
- Chunk stage removed (ADR 0086); quadtree LOD stays (SPIKE-53).

## Exit criteria (proposal), each with a stop rule
| # | Criterion | Measure | Stop rule |
|---|---|---|---|
| 1 | Hill and mountain relief | scorecard (TEST-079) at Valley and Ridge, Black Forest, Highlands, Alps: generator/real p50 of slope and 1 km relief at least 0.6 at both sites (today 0.11-0.35); Badlands and desert distances not worse than the baseline | after the first full bake and two tuning rounds, below 0.4: stop, report, owner chooses between shipping the gain or a GPU-bake ADR |
| 2 | Rivers run downhill in valleys | 0 rises over 0.5 m at levels 14 and finer (SPIKE-60 metric) on 48 random chunks and the owner sites; cut beyond the valley law p50 at most 10 m (today 30 m) | if the bake's ground-following network leaves cut p50 over 20 m after its time box: channels drawn only where the cut is under 20 m, the rest to M10 |
| 3 | Drainage in hills | scorecard drainage density 0.5-3 km per km2 at the hill tiles | none (reported); a miss goes to criterion 1's tuning |
| 4 | Ground at 1.8 m | owner's look judgement on 1.8 m stills at fixed sites (tier D); GPU within terrain's share (BUDG-002) | two rejected rounds: G3 shader detail ships, G1 geometry stays as today |
| 5 | Shadows layer 1 | REND-052 at most 1.5 ms at every pose incl. a 2-degree sun; a dragged preview shadows in its frame; horizon maps removed | low sun over 1.5 ms after the two-frame spread: SPIKE-56's fallback (per-texel shadow map in the clipmap) |
| 6 | No degradation on the new store | gate (TEST-061 to 063): 0 held-back tiles, no frame over 100 ms, no level step; envelope at least 200 m/s at 300 m | any regression against the M8 store blocks the switch of the default store |
| 7 | Bake within the constraints | full bake at most 10 h, checkpointed, resumable; store at most about 28 GB | R3's pre-run projecting over 12 h or 35 GB: cut scope (x2 not x4, fewer passes) before the full bake |
| 8 | Owner look review | plausible hills and rivers at the look reference (tier D) | none: the owner's verdict closes M9 |

## R items: order, size, lane
| # | Item | Size | Lane | Starts after |
|---|---|---|---|---|
| R0 | Shadows layer 1 (#941; 9 PRs) | 9 PRs, 12-18 h, 2 windows | crusader | B6 PR 5 |
| R1 | Terrain scorecard (#942) and its baseline report | 3 PRs, 5-8 h | crusader | now (parallel to R0 PR 1-2) |
| R3 | Bake pre-run: one face at 100 m with a ground-following network, valley carve capped by local relief and tapered from heads, thermal and deposition passes; time, size, scorecard on that face | spike, 2 h + machine hours | spikes | R1 baseline |
| R3b | CPU thermal and deposition kernels on one region against the scorecard (SPIKE-57 recommendation 2) | spike, 2-3 h | spikes | with R3 |
| R4 | The bake: network routed on the base at 100-600 m (curved, GEN-066 style), carve, thermal, deposition; checkpointed; a new store beside `global16384` | 4-6 PRs, 8-12 h + one overnight bake | explorer | R3, R3b |
| R5 | Channels per point along the baked network; width at least two vertex spacings per level (SPIKE-60) | 3-5 PRs, 6-10 h | crusader | R4 store |
| R6 | Ground detail: G3 shader detail, G1 with a steeper fine-octave law (SPIKE-53, SPIKE-54); behind a switch until the owner's look | 4-6 PRs, 6-10 h | crusader | R0 PR 5 (same lit path) |
| R7 | Hill tuning rounds against the scorecard and the look reference | 2-4 rounds of 1-3 PRs | explorer | R4 |
| R8 | Switch the default store; retire the M8 store's channels nobody reads | 1-2 PRs | explorer | criteria 1, 2, 6 |
- Total: about 30-45 PRs, 50-75 lane-hours, one overnight bake (two if the first fails), 4-6 perf windows.
- Calendar (owner back 2026-11-09): R0 and R1 in the first week; R3, R3b and R4 by the end of the second; R5 to R7 in weeks three and four; acceptance about 2026-12-04, at full quota.
- The week plan's month table hoped for M9's acceptance by about 2026-11-22; 2026-12-04 is the realistic figure. The owner picks.

## What needs the owner's eye, and when
| When | What | Form |
|---|---|---|
| R0 PR 5 | new shadows against the horizon maps; a dragged plateau | stills (river-r1, hills-b, Plateau 6) and a 5-10 s video through `cargo xtask package` |
| R1 done | the scorecard's baseline: which gaps matter most to the owner | `report.md` on the phone |
| R4 bake | first look at baked hills and valleys at the SPIKE-55 sites | stills at 300 m, 30 m, 1.8 m plus the scorecard report |
| R6 | ground at 1.8 m, two amplitude laws | stills at three fixed sites |
| R7 end | the M9 look review (criterion 8) and a live test on the laptop | stills, videos, then the laptop |

## Perf windows under ADR 0087
| Check | Run as |
|---|---|
| REND-052 at a 2-degree sun (R0 PR 4) and the acceptance row (R0 PR 9) | window (close call in SPIKE-56) |
| REND-052 at other poses, clipmap upkeep in a flight | loaded pass at 5 % margin |
| R6 GPU cost of G3 and G1 | loaded pass if at least 20 % under its share, else window |
| Gate and envelope on the new store (criterion 6) | window (frame time with streaming: ADR 0087 point 3) |
| Bake time (criterion 7) | the overnight run itself, machine reserved; not a perf window |
| Scorecard run | correctness run, no window |
- Before the first window: re-derive the calibration limits (ADR 0087 open point, ADR 0065); 2 of 4 quiet preflights were refused on 2026-10-30.

## Decisions for the owner up front (proposal for each)
1. **Edits and erosion (group E, D3).** Proposal: edits sit on top of the baked erosion by default (what you draw is what you get); erosion runs in the bake only; no runtime re-solve (the chunk stage is gone).
2. **Erosion that keeps mass (E 12).** Proposal: the bake's deposition pass keeps mass approximately; an edited hill is not eroded at runtime, so it keeps its mass.
3. **A river across an edit (E 15).** Proposal: the channel follows the baked network and is cut through the edit per point (R5); re-routing a river around an edit is M10 or later.
4. **Scope of groups F and G.** Proposal: F (river form) in M9 through R4 and R5; G (resolution at walking height) in M9 through R6.
5. **The 1 m / 500 m exception.** Proposal: close it; it was the chunk re-solve's 10 s column, removed with the chunk stage.
6. **Criterion 1's bound.** Proposal: 0.6 of real at p50 for slope and 1 km relief at the hill and mountain tiles; another number is the owner's call.
7. **Disk.** Decided (owner, 2026-10-31): R4 first writes the erosion result as a separate layer beside `global16384` (easy to reverse), only if reads meet criterion 6's speed; once the erosion result is accepted, it is folded into one store (no paired reads of two files), and its size is optimised then. SPIKE-61: the layer is about 7-10 GB.
8. **GPU bake.** Proposal: not now; the CPU bake must fit 10 h first (R3 decides). A GPU bake with a stored canonical output needs an ADR (candidate below).
9. **Driver maps (ADR 0076, open).** Proposal: soil depth and wetness become baked channels in R4 if they cost under 10 % of the bake; their use (materials, plants) stays M10.

## Candidate ADRs (not written)
- The M9 bake pipeline: a ground-following network, a carve capped by local relief and tapered from heads, thermal and deposition passes, checkpoints (after R3).
- A GPU bake whose stored output is the canonical field (CON-07, CON-14), only if R3 shows the CPU bake over 10 h.
- ADR 0076 decided again with the bake's channels (decision 9).
