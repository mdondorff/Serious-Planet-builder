# 0030 Erosion strategy: local filter in M3, a coarse global stream-power stage after it
Status: accepted (owner, 2026-10-12; decided in the opus-planner session after SPIKE-03 and SPIKE-04)
Date: 2026-10-12
Source: SPIKE-03 (hydrology, #93) and SPIKE-04 (erosion filter variants, #91) in `docs/spikes.md`; architecture report §5 (erosion strategy: "global coarse + local filter"; the MVP excludes global hydrology), M3 row; ADR 0022 (option 1), ADR 0028; constitution CON-08, CON-14, CON-21

## Context
M3 needs an erosion filter and a decision on hydrology. SPIKE-04 rendered three local filter variants; SPIKE-03 compared a global stream-power chain on the sphere with the fallback (rivers routed over the base heights). Findings: stream power gives dendritic valleys and crosses face edges and the cube corner without a visible step, but it is feasible only on a coarse grid (about 21 s at 10 km cells, about 93 s at 5 km, single-threaded; at 1 km hours and tens of GB, beyond CON-21), it shows lattice-axis hatching with single-direction routing, and its across-edge gradients were 1.6x (height) and 2.7x (drainage area) those inside a face at 1024 cells, not yet explained. The fallback has no valleys. Neither needed same-stage reads (ADR 0022 option 1 holds; no CON-08 ruling needed).

## Decision
1. **M3: the local erosion filter is SPIKE-04 variant B** (slope-incised ridged creases): a pure function of the direction over the terrain field (ADR 0028), using the bake's slope, with interval-arithmetic height bounds as tight as the terrain field's, under a new implementation id. Global hydrology is not part of M3 (it was a SHOULD; the report's MVP excludes it).
2. **After the M3 gate: a global stream-power stage on a coarse grid** (4 to 10 km cells), one field per ADR 0022 option 1, cached like the global bake, read by regional or local carving as an earlier stage's output. Before it ships: a routing that removes the lattice hatching (multiple-flow-direction or D-infinity, measured against single-direction), and the across-edge gradient check at the production grid with the cause of the 1.6x / 2.7x ratios explained.
3. Variant C (meanders, about 3.7x the baseline cost, looser bounds) is not adopted; it may return as a new version if its look is worth the cost.

## Consequences
- M3 acceptance does not wait for hydrology; the cells 32 / ratio 240 planning check must pass with variant B's real bounds and the production bake.
- The global stage after M3 needs its own keys, perf entry (time and memory under CON-21) and seam tests; local carving that reads it is a new version of the filter.

## Licences of adopted code or techniques
None (stream power and priority-flood routing are published methods; no code adopted).
