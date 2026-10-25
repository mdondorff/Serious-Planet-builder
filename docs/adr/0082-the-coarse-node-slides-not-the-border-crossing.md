# 0082 The coarse node slides, not the border crossing
Status: proposed (drafted by opus-planner, 2026-10-25; built in #692 before acceptance, easy to reverse)
Date: 2026-10-25
Source: #692 (sonnet-explorer); ADR 0081 point 1; GEN-066, GEN-067

## Context
- ADR 0081 point 1 lets the point where a river crosses a chunk border slide along that border, chosen by a rule both neighbours evaluate on their two edge rows.
- A chunk covers one coarse cell; the coarse network's node is that cell's centre.
- A node moved by a function of its own cell alone is read identically by every chunk whose halo holds it; no edge-row rule is needed, and confluences agree by the same argument.
- The segment between two moved nodes is a least-cost path over a 32 m raster within 1 km of the coarse guide; neighbours, halos and worker counts agree on it (GEN-067, tested).
- Measured on the 16384 store (#692): climbing links 0 at the lowland site, site 2 and hills B; 7 of 756 at hills A; 519 of 1344 at the steepest river (40.8 deg).

## Decision
1. **Amends ADR 0081 point 1.** The coarse node slides, to the lowest ground within the border tolerance of ADR 0081 (0.5 km, 1 km on steep chunks); the border crossing is where the routed segment meets the border, not a separately chosen point.
2. A free head (ADR 0081 point 2) slides the same way, to the highest ground within its radius.
3. Everything else in ADR 0081 stands: the coarse network fixes which chunks a river crosses, re-routing across chunk borders stays excluded.

## Consequences
- One rule (a function of the coarse cell) replaces the edge-row rule; border agreement stays tested (GEN-067).
- The crossing can lie farther along the border than the tolerance when the routed segment bends; it stays inside the 1 km band around the guide.
- Easy to reverse: a zero slide gives the fixed nodes of the coarse network.

## Licences of adopted code or techniques
None.
