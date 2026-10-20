# 0073 Normals from the tile's height lattice
Status: proposed (sonnet-spikes, 2026-10-20, at opus-planner's request, who chose it after SPIKE-23; the owner decides). Refines the canonical normal named in ADR 0033 point 4; touches GEN-019, GEN-010.
Date: 2026-10-20
Source: SPIKE-23 (`spikes/edit-refresh/results/results.md`); evidence `spikes/lattice-normals/results/results.md` (SPIKE-24); ADR 0003 and ADR 0028 point 4 (the stencil as a function of the direction); GEN-019, GEN-020, COORD-007, WDEF-007

## Context
- A tile's vertex normals come from four extra height evaluations per vertex (`normal_stencil`, a tangent frame of the vertex, one cell of angle): 80 % of the evaluations of a 32-cell tile.
- The evaluations are the whole cost of a tile job, so of every refresh, dive and start-up load (SPIKE-23: the stamp stage is under 1 %).
- The lattice already holds the heights one cell either side of every vertex; the stencil measures about the same two-cell baseline along other axes.
- Neighbouring tiles must agree on a shared vertex's normal (no lighting seam); today they do exactly, because the stencil is a function of the direction alone.
- On the real worlds the lattice normal differs from the stencil normal by a mean of at most 0.05 degrees, p99 at most 0.39 and at most 1.03 at the steepest tiles; no pixel of the lit stills differs by more than 1 % of full scale (the stills show plains and haze; relief is shown by hillshades in the evidence).

## Decision
1. **The normal of a vertex is the cross product of the centred differences of its neighbours' positions along the lattice axes**, `n = (q(i+1,j) - q(i-1,j)) x (q(i,j+1) - q(i,j-1))` normalised and oriented outward, with `q = d * (R + h)` in f64 on the tile's own lattice (COORD-007). One definition for the CPU mesh, the compact tile and every test oracle (the canonical normal of GEN-019).
2. **One batch per tile**: the lattice, its one-sample ring without the four ring corners, and the origin (GEN-020); about 4.5 times fewer evaluations than the stencil.
3. **Seams.** Within a face, two tiles sharing a vertex read the same directions and so compute bit-identical normals. Across a cube edge the ring point beyond the face edge is the neighbour tile's own lattice point one cell inside it, on the row that continues the shared edge (not the extended cube plane): the two tiles read the same four points. At a cube corner the same rule gives the three tiles the same three points; their normals agree to f64 rounding, within the 1e-6 degrees of the renderer's seam scenario, with no larger tolerance. Tiles of different levels are as today (skirts, morph).
4. **The read set is unchanged**: the ring lies within one lattice cell of the tile, inside the footprint halo of WDEF-007; no read record, footprint, key composition or refresh set changes.
5. **`MESH_VERSION` 9** (GEN-010): every tile key, persistent mesh key, pre-filled pack key and committed known answer is made again once; repro bundles of version 8 are refused with the versions named (TEST-008).

## Consequences
- A tile job costs about 4 times less; the drawn-exact time of the edit-latency case at 200 m with a 150 m stamp falls from 508 ms to about 180 ms by the SPIKE-23 model (an estimate, to be measured in a perf window).
- Normals differ from before by the figures above; fine detail is shaded with about the same two-cell filter, neither smoother nor sharper.
- Goldens: the debug and shaded goldens that show normals or lighting are declared changes (AUTO-E, ADR 0017 with ADR 0061 point 3; named in the change's Expected golden changes with their bound tests); every lit, sky and water golden that moves goes to the owner's `/bless` with the hillshades as evidence.
- Numerics review (core, generators) of the position differences, as the stencil's own `q = d * (R + h)` form.
- The spec delta, the new tests and the golden list are the change's proposal; the evidence names them.
- Easy to reverse: the version bump re-keys everything.

## Licences of adopted code or techniques
None (the centred difference of a height lattice is textbook).
