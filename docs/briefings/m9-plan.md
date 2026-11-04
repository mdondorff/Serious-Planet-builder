# M9 Realistic terrain: plan and iterations
Status: active (started 2026-11-01 with the owner's M9 decisions; plan written by opus-planner, 2026-11-02; re-planned 2026-11-04 after the erosion preview). Updated at every iteration boundary; an iteration closes with a line per item: done, carried or dropped.
Sources: `docs/briefings/m9-decisions.md`; ADR 0088, 0089, 0091, 0092 (proposed); `docs/roadmap.md`.

## Exit (milestone acceptance, `cargo xtask accept M9`)
1. The baked world (ADR 0088) of the owner's seed passes the terrain scorecard (TEST-081 to TEST-084) and the generator bands (TEST-082, TEST-087, TEST-088).
2. The owner accepts the look at the scorecard sites and at walking height (stills and a live session).
3. No-degradation gate and the speed envelope hold with the bake on.

## Iterations
| # | Iteration | Exit | Lane | State (2026-11-04) |
|---|---|---|---|---|
| I1-I3 | Generator rounds 2-4 (continents, relief bands, belts) | as before | explorer | done to the preview level; high mountains reach 0.21 of GLO-30 3 km relief (SPIKE-99): a carried generator item for I9 |
| I5 | Bake code (ADR 0088, 0098, 0099) | passes, kinds 12/13, pass 1 at 12,288 with MFD | explorer | merged (#1104-#1145) |
| I5a | Bake fixes before the preview (owner, 2026-11-04) | (a) SPIKE-98 settings: halo 128, base every 4 cells, thermal 300 passes; (b) no one-cell pits or pillars: the depression fill capped by the receiver, what remains spread over neighbours, variety at 200 m-1 km kept (measured before and after); (c) two baked channels kept for ground detail: drainage area (pass 1, MFD) and erosion/deposition depth (pass 2) | explorer | next |
| I5c | Runtime read of the layer (PR 6) | the field reads the layer C1, the crease band on (ADR 0098), the coast fade within 3 m; previews render through it | crusader | next |
| I8a | Ground detail, first step (R6, pulled forward) | shader detail with variety: more than one scale, varied by slope and by the channels of I5a(c); 1.8 m, 10 m, 50 m stills; GPU time; behind a switch | crusader after I5c; design by spikes | design now |
| I4b | Integrated preview (owner, 2026-11-04) | through the viewer's path, everything the product shows (labelled omissions only): mountains incl. the owner's circled wall, hills, a plains site, a coast; 2 km, 300 m, 50 m, 10 m, 1.8 m; low sun | explorer + crusader | after I5a, I5c, I8a |
| I6 | Planet bake | the owner's go after I4b; about 6-7 h projected (SPIKE-98; single-thread figures until the machine is restarted) | explorer | after I4b |
| I7 | Channels as per-point primitives (R5) | rises 0, gate, stills; behind a switch | tbd | open |
| I8b | Ground materials from the channels (owner, 2026-11-04: in M9, so the channels are used) | material mix (rock, scree, soil, grass, wet ground) from slope, drainage area and deposition depth; ground-detail variety by material; owner's look | crusader | after I6 |
| I9 | Look against the reference (R7) | scorecard on the bake; high-mountain relief; owner judgement | explorer + owner | open |
| B | k renamed to a per-class band amplitude; kinds 10/11 and `m + k (e - m)` retired (SPIKE-99, owner yes 2026-11-04) | spec and code; no look change | tbd | after I4b |

Ground-detail noise and materials are runtime only: they iterate after the bake without a rebuild. The baked channels are not: they go into the bake now.

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
- I5a, I5c, I8a design: 2026-11-04 to 11-06.
- I4b: about 2026-11-06; I6 the night after the owner's go (the machine restart is not before Sat 2026-11-07).
- I8b, I7, I9: from 2026-11-09; acceptance not before 2026-11-16.
