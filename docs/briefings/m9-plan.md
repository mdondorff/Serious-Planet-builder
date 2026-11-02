# M9 Realistic terrain: plan and iterations
Status: active (started 2026-11-01 with the owner's M9 decisions; plan written by opus-planner, 2026-11-02). Updated at every iteration boundary; an iteration closes with a line per item: done, carried or dropped.
Sources: `docs/briefings/m9-decisions.md`; ADR 0088, 0089, 0091, 0092 (proposed); `docs/roadmap.md`.

## Exit (milestone acceptance, `cargo xtask accept M9`)
1. The baked world (ADR 0088) of the owner's seed passes the terrain scorecard (TEST-081 to TEST-084) and the generator bands (TEST-082, TEST-087, TEST-088).
2. The owner accepts the look at the scorecard sites and at walking height (stills and a live session).
3. No-degradation gate and the speed envelope hold with the bake on.

## Iterations
| # | Iteration | Exit | Lane | State (2026-11-02) |
|---|---|---|---|---|
| I1 | Continents (generator round 2) | S1-S3, S9-S11 on seeds 1-5; the owner's seed | explorer | #1078: seed 14 picked; the test field's climate aligned before the bless (delegated to the planner) |
| I2 | Relief to Earth's bands (round 3) | TEST-082 gate (median in band +-10 %, each seed within one width); GEN-097 p99 at most 2.0 | explorer | #1086 draft; owner: p99 form; class bands proposed, owner yes pending (not blocking) |
| I3 | Belts at convergent margins only (round 4) | belt land 13-23 %; south polar land under 2 km mostly; seed 4 in band | explorer | next |
| I4 | Engine preview approval | the owner approves seed 14's preview (generator only) | owner | after I3 |
| I5 | Bake code (ADR 0088) | channels, runtime base behind a switch, pass 1 carve, pass 2 incise, thermal and deposition, min/max pyramid, checkpoints; tier A/B on regions | explorer | PR 1 (channels and stored form) done locally; PR 2 next |
| I5b | Pipeline snapshot before the bake (owner, 2026-11-02) | the full pipeline with erosion, fast: the first pass over the planet, the second pass on 30-40 sample tiles at full resolution and optionally the planet at reduced resolution (estimate under an hour); preview maps and stills; the owner approves before I6. Built as a non-editor workflow (an app mode or `generate` modes `--planet-res`, `--region`) reusable for global authoring | explorer | bake code takes resolution and region as parameters (planned) |
| I6 | Pre-run and planet bake | one face at full resolution timed; then the overnight planet bake (6-7.5 h) with checkpoints | explorer | after I4 and I5 |
| I7 | Channels as per-point primitives (R5) | rises 0, gate, stills; behind a switch | tbd | open |
| I8 | Ground detail at walking height (R6) | 1.8 m stills at fixed sites, GPU time; behind a switch | tbd | open |
| I9 | Look against the reference (R7) | scorecard on the bake; owner judgement | explorer + owner | open |

## Beside M9 in the same weeks (not M9 exit items)
| Item | Lane | State |
|---|---|---|
| Shadows ADR 0085: 5b-2 apply, PRs 6-9, the 2° perf window | crusader | #1084 blessed, #1085 acne fix |
| B4 rest: real-store pair, daily package | crusader | after 5b-2 |
| `node-buffer-delta` | crusader | after B4 |
| B9 perf-window process (SPIKE-95) | spikes | started 2026-11-02 |
| Relative compute shares (ADR 0092) | owner decision, then 4 PRs | proposed |

## Later, not M9 exit
- A runtime view of a pre-baked world from the snapshot (owner, 2026-11-02: interesting, a second step after I5b).
- The snapshot workflow is the base of the dormant "global authoring preview" item (status.md).

## Dates (estimates, not commitments)
- I1-I3 and I5: this week (owner away until about 2026-11-08).
- I4 and I6: after the owner's preview approval; the bake runs the following night.
- I7-I9: after the owner's return; acceptance not before 2026-11-16.
