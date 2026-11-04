# 0099 Erosion layer: pass 1 at 12,288 cells, MFD area, C1 read of the pass-1 change
Status: accepted (owner, 2026-11-03: "12,288 it is"; planner inbox #55 and #56)
Date: 2026-11-04
Source: owner, 2026-11-03; ADR 0088 point 4; GEN-100, GEN-101; stair and block diagnosis (explorer, planner inbox #51/#52; `target/review/erosion-diag/` of the explorer worktree); SPIKE-98 cost results (`spikes/erosion-bake-cost/results/`); change `pass1-mfd` (#1145)

## Context
- ADR 0088 point 4 reads the pass-1 change bilinearly in pass 2 and routes pass 1 on a D8 area at a coarse grid.
- A bilinear read leaves strips along every pass-1 cell line; a D8 area imprints the grid's eight directions as staircase valleys and flat blocks at the pass-1 lattice scale.
- Grid alignment of the change (low-passed gradients within 5 degrees of the axes; isotropic 11.1 %), mountains tile: 18.4 % as built, 14.0 % with the MFD area, 11.2 % at 12,288 cells with the MFD area and the C1 read; remedies in pass 2 alone left it at 18.3-18.6 %.
- Relief check at 12,288 (D8 against MFD, mountains and hills): result relief p50 -5.1 % and -2.6 %, p90 -14 % and -8 %; added median relief over the base kept 73 % and 94 %.

## Decision
1. Pass 1 runs at 12,288 cells a cube-face edge (about 814 m), not the coarser grid of ADR 0088 point 4.1.
2. The drainage area of pass 1 is shared among the lower neighbours (multiple flow direction, Freeman, exponent 1.1, highest cells first, ties by cell index; outlets pass nothing on). The receivers stay the jittered D8 of ADR 0088 for the implicit carve.
3. Pass 2 reads the pass-1 change by Catmull-Rom (C1), clamped to the range of the four surrounding cells (a spline overshoots 7.4 % of a step: levees, and the sea floor). This replaces "bilinear" in ADR 0088 point 4.2.
4. The kernel versions rise (bake 2, pass 2 2), so the keys follow (CON-07).
5. Uplift (U/K) is not recalibrated by this decision; it follows the owner's decision on SPIKE-99.

## Consequences
- Amends ADR 0088 point 4 (points 4.1 and 4.2; the rest stands). Point 5 (the runtime adds the layer bilinearly) is unchanged: the pass-2 tiles are the stored layer.
- Pass 1 takes 17-23 min a face at B = 10 (local, indicative); the cone carve test compares against the carve without uplift because the shared area out-incises the default uplift.
- The deposition capacity can fall along a path where the flow diverges (a fan); the memory of the area build is about 1.6e8 cells.
- Requirements: GEN-100 (area shared), GEN-101 (C1 read).
