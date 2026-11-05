# 0105 Deposition in pass 1 and endorheic sinks in arid basins
Status: accepted (owner, 2026-11-05: "All four into m9", the independent review's items 1 and 2)
Date: 2026-11-05
Source: owner, 2026-11-05; independent review of the M9 bake plan (2026-11-05); Yuan et al. 2019 (stream power with deposition, FastScape); Davy and Lague 2009; ADR 0088 point 4; ADR 0101; ADR 0104

## Context
- Pass 1 only detaches material; nothing fills a kept depression, so kept pits deepen every step (p50 63 m against 3-7 m in real mountains).
- Flat alluvial valley floors and intermontane or foreland basins need deposition at pass 1's scale.
- Routing every kept basin to the sea overstates drainage area and carve downstream of arid closed basins, and leaves no playas.
- Both change the heights the routing sees, so adding them after the planet bake means a re-bake.

## Decision
1. Pass 1 deposits sediment: eroded volume is routed downstream with a transport-limited deposition term (Yuan et al. 2019 or an equivalent volume routing), filling kept depressions up to their spill level before passing the rest on; the solve stays implicit, fixed-iteration and deterministic.
2. Where aridity is below a threshold, a kept basin is a sink: its water evaporates and does not route to the sea; its sediment stays in it.
3. The fill-depth channel (ADR 0101) keeps its meaning: the depth left between the deposited surface and the spill level.

## Consequences
- Extends ADR 0088 point 4 and ADR 0101; bake kernel versions change.
- Pit depth and share per class are re-measured against GLO-30 (the extended scorecard).
- Playas and later lake water read the sinks without a re-bake.
