# 0112 The erosion solver: regions, rounds and catchments
Status: proposed (spikes session for opus-planner, 2026-11-08, inbox #100; amended after the ADR review, inbox #102). Acceptable as the prototype's architecture after the owner's D2-D10 pass; not approved for production before G-LOCAL, G-ROUND, G-PART, G-EQ and G-INC pass on the actual kernels (A/B gate, D3)
Date: 2026-11-08
Source: `docs/design/terrain-target.md` §6; SPIKE-105 F2, F4, F7, §18; ADR 0088 point 4, 0099

## Context
- A catchment carved with its own routing differs from the same catchment in a larger carve (F2: rms 5 m; F4: 12.8 m at best); the carve is chaotic (72 m rms between jitter seeds).
- With one routing per round, the breach and sweeps per D8 catchment are bit-identical to the whole-window carve (F7).
- MFD shares and the flood's tie order couple catchments (F7).
- Routing is 85-90 % of a round.
- The smallest erosion unit today is a whole face (10 GB, ADR 0099); it cannot run in the editor (S105 X7, X8).

## Decision
1. The scope of one routing is a **region**: a connected component of the land mask at the regional grid, bounded by the sea, re-derived at every commit. It is routed whole, resident or partitioned. A split at H0 divides is a tested fallback only.
2. The routing's reference is a definition (A3 §6.8): the flood's unique fixed point, the receiver rule, the area's donor order. Resident and partitioned routes must compute it bit for bit; G-PART proves it.
3. A **round** is one region-wide routing and the steps that follow; rounds continue the surface. The round count is fixed in the `SimulationProfile`.
4. A **catchment** is a D8 tree of a round's routing. The breach and the sweeps run per catchment, any order, on the shared routing. Thermal and pit spreading cross divides and run after the last round: per tile with a halo is the target; today they run on a whole grid and their spatial cut is untested. A later coupling is added to the contract; "independent" is not a permanent limit.
5. MFD is clipped to the catchment (a lower neighbour in another catchment is dropped from the weights and from their sum; the D8 receiver always remains, so no fallback is needed; `A_d` at an outlet is the sum of `a w`). The flood orders visits by (level, global cell id).
6. A re-solve's result is by definition a full region re-route; a full recomputation is always an allowed fallback. Reuse is an optimisation: each step is reused only if all its inputs are bit-equal, including a tile's incoming offers (dependency table, A3 §6.8). No dirty region is predicted (F9).
7. Every value a catchment-round reads is in its key, directly or through an immutable upstream key, in canonical form (A3 §6.5); a catchment's members are visited and hashed in `Routing.order` restricted to the catchment (D31). The same function makes a fresh build (CON-29).

## Consequences
- On the gate, replaces ADR 0109 and ADR 0105 point 1: deposition is not dropped but moves to the post-round sediment pass (A3 §6.7; the basin infill is replaced through ADR 0111 point 5). Amends ADR 0088 point 4.1 and ADR 0099 points 1 and 2.
- CON-08 v1.7 (ADR 0116) allows the regional solver and its rounds; CON-30 is G-PART's basis.
- Interim, until the A/B gate: the accepted ADRs named here govern the shipped bake; this ADR governs the prototype path behind its switches (architect D24, 2026-11-10).
- Gates G-ROUND, G-LOCAL, G-EQ, G-INC, G-SCALE, G-PART. A failure of G-LOCAL or G-EQ is switch criterion S1 or S2 of S105 §13.1. F7 did not test these kernels; passing on prototype fixtures is not passing a gate at scale.
