# 0123 River architecture: drainage that survives each scale, rivers routed at L
Status: proposed (lead, 2026-11-14; owner acceptance pending: architecture and a store format)
Date: 2026-11-14
Source: SPIKE-114 (#1487), SPIKE-115 (#1495), SPIKE-116 (#1498), the rivers literature review (lanes digest 2026-11-14); owner, 2026-11-14: "double down on rivers"; fix at the earlier, more global stage where the evidence points

## Context
- At L (101.8 m cells) the start surface held closed basins on 73-75 % of the river length. Rivers through them were either canyons (10-16 % of river cells cut over 50 m) or lakes (up to 57 % of river cells).
- The cause is the 814 m to L hand-over, not the base B. The solved regional surface drains at 814 m (5.9 % of river length on basins), but read bilinearly at L it has 74.5 %. A D8 step to a diagonal neighbour between two higher cells becomes a saddle in any interpolant.
- One round of the regional erosion kernel at L over the region meets the bars on R3a: basins 8.6 % of river length, canyons 0.72 %, lakes 3.2 %, relief 111-113 %. It costs 73 s and 6.2 GB.
- River lines from the 814 m network cross ridges that exist only at L. Lines rebuilt from B alone at read time agree on 8 % of channel cells.
- Exact incremental re-routing after an edit: Barnes 2016's tile scheme, 0.11 s per touched tile plus a graph over the tile perimeters.

## Decision
1. The regional solve drains through cell edges only (4-neighbour receivers, or a 4-neighbour breach after the rounds), so its drainage survives a bilinear read at L. The base's residual below 814 m is damped to 0 near the 814 m channels (over about 400 m).
2. One round of the regional kernel runs at L over the whole region, after the regional rounds and before K7. Its count is a parameter of the spec, measured on R3a, R1h and R3b.
3. Rivers inside a region are routed at L on that surface. The stored graph holds the L line geometry and the network levels (GEN-185). That is a store format change.
4. Depressions at L: breach when the cut needed is at most D, otherwise a lake at its spill level; D is set by the inland-lakes ADR.
5. Edits re-route at L by Barnes 2016: the touched tiles are flooded alone, then the perimeter graph is solved. The result is exact and independent of the partition (CON-30).
6. Canyons through ridges appear only where drainage forces them: overflow of a basin at its sill, or an antecedent river from the coarse network. They are never the repair for un-eroded relief.

## Consequences
- Amends the river chain of ADR 0119 (K7 reads the L-eroded surface; its burn lines come from the L route) and STRM-084 (the graph's geometry and levels).
- New requirement IDs in the implementing changes: the 4-neighbour regional drain, the L round, the L route with stored geometry, the Barnes edit path.
- Risks to measure in the implementation: axis-aligned grooves from 4-neighbour drainage (seen at 40 km on R3b); edge agreement with H1 on flat ground (71 of 122 and 135 of 200 crossings); K7's canonical flood refuses flats of the eroded surface (route.rs:236).
- The look of the starting surface (long ridges, against the DEM reference) stays a separate, parked question.
