## Why
ADR 0081 (accepted, owner 2026-10-25): the corridor follows the coupled run's 4.9 km network cell by cell over a base that holds km-scale relief, so the river climbs (on the rebuilt store 205 to 1,049 climbing links at the sites below) and its bed lies hundreds of metres over the ground (anchor minus base median +679 m at the steepest site). The bed rule of H1 (a running maximum from downstream) turns every rise on the river's way into a dam.

## What Changes
1. `generators::route::Router` (GEN-066): the node of a river's coarse cell slides to the lowest base ground within 0.5 km (1 km on steep ground by the relief factor), a head to the highest ground within 1 km; the segment from node to node is the least-cost path (rise plus 0.02 per metre of length, integer millimetres) over the raster cells (every 4th) within 1 km of the coarse guide. Every segment meeting at a cell shares its node, so a chunk border or a confluence needs no rule of its own. `route_band_m` 0 with no slide is the coarse path of before.
2. The bed rule (GEN-059, amended in place in `m8-anet-chunk`'s delta, which is not archived yet): the running minimum from upstream along the main stem (the donor of the largest area) for 16 segments, the least fall under the cell above, at most 200 m under the ground (`cut_cap_m`); a tributary ends no lower than the stem's bed at the confluence. Samples every polyline point (40 m).
3. `ANET_VERSION` 6; the routing parameters join the definition hash; `window` 16 (it counts upstream segments now); `spike()` turns the routing on.
4. `chunk_incision` reports the network build time and takes `--route-off` for the comparison.

## Departure from ADR 0081 point 1
The ADR slides the point where the path crosses a chunk border. Here the node (the coarse cell's centre in the path) slides, whatever the chunk borders: a chunk is a whole coarse cell (level-11 tile), so the border points are the nodes' neighbours and need no rule about two chunks' edge rows; both neighbours read the same node because it is a function of the coarse cell alone. Same tolerance (0.5 km, 1 km steep).

## Tests
- Tier A: neighbouring chunks find the same routed corridor, beds and links; the routed path goes round a ridge the coarse path crosses; all segments through a cell share its node, which lies on ground no higher than the centre within the radius; the bed lies under the ground, never climbs downstream, and stays within the cut cap under a forcing ridge.
- Known answer of the synthetic chunk re-pinned (declared).

## Limits
- Tributaries use the same routing as the stem (every segment is routed node to node); the ADR's measured second step is the measure of the tributaries per km2 (not made here).
- A coarse network that climbs more than the nodes can slide still climbs or dams: the steepest river of the run (34.158, 140.835) keeps 519 of 1,344 links climbing and 3,491 cells at the 300 m bound.
- Cost: see the PR.
