# 0081 Local main-stem routing inside a chunk
Status: proposed (opus-planner, 2026-10-25; the owner's acceptance pending; departs from an accepted ADR, so no merge of code built on it before acceptance)
Date: 2026-10-25
Source: SPIKE-42 (`spikes/local-routing/results/results.md`, #676); SPIKE-40 and 41; `openspec/changes/m8-anet-chunk/design.md` sections 17 to 20; owner, 2026-10-25 (M8: hydrology right); ADR 0077 point 3

## Context
- The chunk's river corridor follows the coupled run's 4.9 km network cell by cell; the base at 8 m holds km-scale relief (creases, the hill band of ADR 0080) the network does not see.
- On the relief-keeping run the corridor climbs: 178 of 680 links at the lowland site, 88 of 169 at site 2, bed anchors a median 266 m over the ground there.
- A capped running-minimum bed on the same path cuts water gaps instead: 16 m deep and 2.4 km over 5 m on a hilly chunk (SPIKE-42).
- Routing the main stem inside a band of 1 to 2 km around the coarse path, entry and exit cells fixed by the coarse network, cuts 0.3 m on that chunk; rising links fall from 356 to 7 to 26 of about 600; 4 to 50 ms a chunk against about 4 s; identical bits at 1 and 8 workers; neighbours agree on the shared border cell.
- A least-cost route is straight (sinuosity 1.0 to 1.2 at 4 km); lowland rivers need 1.3 to 2 (SPIKE-40).

## Decision
1. **Departure from ADR 0077 point 3.** Inside a chunk the main stem is routed on the chunk's own raster through a band of width `W` around the coarse network's path; the coarse network still fixes which chunks a river crosses and the cells where it enters and leaves each chunk. Re-routing across chunk borders stays excluded.
2. **Free head.** A river with no upstream chunk starts at the highest ground within a stated radius of its coarse head, not in a pit.
3. **Tributaries** above a stated area threshold are routed the same way into the main stem, as a second step with its own measurement; if they do not meet their targets within M8, they move to M9.
4. **Meanders** (lowland sinuosity 1.3 to 2) are not part of this decision; they are M9 (realistic terrain).
5. `W`, the head radius and the tributary threshold are parameters in the chunk stage's definition and key.

## Consequences
- The bed follows the ground: no corridor anchor above it, cuts only where entry and exit force them.
- The chunk stage's version bumps; chunks re-key; border agreement and thread independence stay tested (criterion 2).
- Easy to reverse: `W` = 0 is today's path.

## Licences of adopted code or techniques
None.
