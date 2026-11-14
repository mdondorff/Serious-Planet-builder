# 0123 River architecture: drainage that survives each scale, rivers routed at L
Status: proposed (lead, 2026-11-14; reviewed by crusader and spikes; owner acceptance pending after an external review: architecture and a store format). Precondition for acceptance: SPIKE-117 (the combination) meets the bars on R1h, R3a and R3b; SPIKE-118 (Schott 2024, done fairly) is compared.
Date: 2026-11-14
Source: SPIKE-114 (#1487), SPIKE-115 (#1495), SPIKE-116 (#1498), the rivers literature review (lanes digest 2026-11-14); owner, 2026-11-14: "double down on rivers"; fix at the earlier, more global stage where the evidence points

## Context
- At L (101.8 m cells) the start surface held closed basins on 73-75 % of the river length. Rivers through them were either canyons (10-16 % of river cells cut over 50 m) or lakes (up to 57 % of river cells).
- The cause is the 814 m to L hand-over, not the base B. The regional surface drains at 814 m (5.9 % of river length on basins), but read bilinearly at L it has 74.5 %. A D8 step to a diagonal neighbour between two higher cells becomes a saddle in any interpolant.
- One round of the regional kernel at L over the region meets the bars on R3a (basins 8.6 %, canyons 0.72 %, lakes 3.2 %, relief 111-113 %), at 73 s and 6.2 GB. It misses the basin bar on two flat windows (29.1 %, 13.7 %).
- The 4-neighbour hand-over removes canyons and lakes on R1h, R3a and R3b (cut 0 %, lakes 0-3 %) but leaves shallow basins on 25-35 % of the river length.
- Lines rebuilt from B alone at read time agree on 8 % of channel cells. Barnes 2016's tile scheme re-routes exactly in 0.11 s per touched tile plus a perimeter graph.

## Decision
1. The regional solve drains through cell edges only. The measured form is a 4-neighbour breach after the regional rounds; 4-neighbour receivers inside the rounds are the alternative to measure. Land is floored at sea level plus 5 mm. B's residual below 814 m is damped to 0 near the channels of the drained surface (75 km2 threshold, over about 400 m), with the same sea-level floor.
2. One round of the regional kernel runs at L over the whole region, after the regional rounds and the hand-over, before K7. Its count is a parameter of the spec, measured on R1h, R3a and R3b.
3. Rivers inside a region are routed once at L on that surface, in the region's front, and stored: the graph holds the L line geometry and the network levels (GEN-185). The reader never re-derives the routes. That is a store format change.
4. Depressions at L: breach when the cut needed is at most D, otherwise a lake at its spill level; D is set by the inland-lakes ADR.
5. Edits: re-run the L round over the dirty catchments, memoised like the regional rounds (SPIKE-112's level 2); then re-route by Barnes 2016 (the touched tiles flooded alone, then the perimeter graph). Both steps keep incremental equal to fresh (CON-29) and are independent of the partition (CON-30).
6. Canyons through ridges appear only where drainage forces them: overflow of a basin at its sill, or an antecedent river from the coarse network. They are never the repair for un-eroded relief.

## Bars (measured on R1h, R3a, R3b)
| Bar | Value |
|---|---|
| Closed basins on river length (pits at least 0.5 m deep and at least N cells; N set by SPIKE-117) | at most 10 % |
| River cells cut over 50 m below the start surface | at most 1 % |
| Relief at 200 m-1 km against the start surface | within ±20 % |

## Consequences
- Amends the river chain of ADR 0119 (K7 reads the L-eroded surface; its burn lines come from the L route) and STRM-084 (the graph's geometry and levels).
- New requirement IDs in the implementing changes: the 4-neighbour regional drain, the L round, the L route with stored geometry, the edit path (dirty-catchment L round, then Barnes).
- Risks to measure in the implementation:
  - axis-aligned grooves from 4-neighbour drainage (seen at 40 km on R3b);
  - the 814 m breach lowers 1.03 % of R1h's land by over 50 m (valleys 814 m wide);
  - edge agreement with H1 on flat ground (71 of 122 and 135 of 200 crossings);
  - K7's canonical flood refuses flats of the eroded surface (route.rs:236): fix GEN-173's grade for them, or K7 uses the plain flood;
  - the cost of the dirty-catchment L round after an edit (unmeasured; the whole region takes 73 s).
- Fallback: if the combination fails the flat windows, Schott 2024's multi-scale erosion with per-scale parameters and elevation retargeting is the next candidate (SPIKE-118 measures it now).
- The look of the starting surface (long ridges, against the DEM reference) stays a separate, parked question.
