# 0112 The erosion solver: regions, rounds and catchments
Status: proposed (spikes session for opus-planner, 2026-11-08, inbox #100); the owner accepts after the external review and the D2-D10 pass; a production decision follows the prototype's gates (D3)
Date: 2026-11-08
Source: `docs/design/terrain-target.md` §6; SPIKE-105 F2, F4, F7; ADR 0088 point 4, 0099

## Context
- A catchment carved with its own routing differs from the same catchment in a larger carve (F2: rms 5 m; F4: 12.8 m at best); the carve is chaotic (72 m rms between jitter seeds).
- With one routing per round, the breach and sweeps per D8 catchment are bit-identical to the whole-window carve (F7).
- MFD shares and the flood's tie order couple catchments (F7).
- Routing is 85-90 % of a round.
- The smallest erosion unit today is a whole face (10 GB, ADR 0099); it cannot run in the editor (S105 X7, X8).

## Decision
1. The scope of one routing is a **region**: a connected landmass, bounded by the sea. It is routed whole, resident or partitioned (tiles flooded from the neighbours' offers, cross-tile accumulation), with the same bits. A split at H0 divides is a tested fallback only.
2. A **round** is one region-wide routing and the steps that follow; rounds continue the surface. The round count is fixed in the `SimulationProfile`.
3. A **catchment** is a D8 tree of a round's routing. The breach and sweeps run per catchment, any order, on the shared routing.
4. MFD is clipped to the catchment (a lower neighbour in another catchment is dropped from the weights and from their sum: the area at an outlet is the sum of its members' true areas, A3 section 6.4); the flood orders visits by (level, global cell id).
5. A re-solve is a full region re-route executed with memoised work: only tiles whose round-start surface changed are re-flooded, the tile borders are solved for the whole region (sweeps to a fixed point), and receivers and area are recomputed where their inputs changed. No dirty region is predicted (F9).
6. A catchment-round is keyed by its member cells, outlet and base level, the receivers and diffuse area on its members, their round-start surface, drivers, the profile and the kernel versions; the same function makes a fresh build.

## Consequences
- Supersedes ADR 0109 and ADR 0105 point 1 on the gate; amends ADR 0088 point 4.1 and ADR 0099 points 1 and 2.
- Gates G-ROUND, G-LOCAL, G-EQ, G-INC, G-SCALE, G-PART. A failure of G-LOCAL or G-EQ is switch criterion S1 or S2 of S105 §13.1.
