# 0073 Normals from the tile's height lattice
Status: proposed (sonnet-spikes, 2026-10-20, at opus-planner's request, who chose it after SPIKE-23; the owner decides). Refines the canonical normal named in ADR 0033 point 4; touches GEN-019, GEN-010.
Date: 2026-10-20
Source: SPIKE-23 (`spikes/edit-refresh/results/results.md`); evidence `spikes/lattice-normals/results/results.md` (SPIKE-24); ADR 0003 and ADR 0028 point 4 (the stencil as a function of the direction); GEN-019, GEN-020, COORD-007, WDEF-007

## Context
- A tile's vertex normals come from four extra height evaluations per vertex (`normal_stencil`, a tangent frame of the vertex, one cell of angle): 80 % of the evaluations of a 32-cell tile.
- The evaluations are the whole cost of a tile job, so of every refresh, dive and start-up load (SPIKE-23: the stamp stage is under 1 %).
- The lattice already holds the heights one cell either side of every vertex; the stencil measures about the same two-cell baseline along other axes.
- Neighbouring tiles must agree on a shared vertex's normal (no lighting seam); today they do exactly, because the stencil is a function of the direction alone.
- On the real worlds the lattice normal differs from the stencil normal by a mean of at most 0.05 degrees, p99 at most 0.39 and at most 1.03 at the steepest tiles (two estimators compared, neither against the truth); under a low sun 1 degree moves a vertex's Lambert shade by up to about five 8-bit levels, while the lit stills (plains and haze) differ by under 1 % of full scale.

## Decision
1. **The normal of a vertex is the cross product of the centred differences of its neighbours' positions along the lattice axes**, `n = (q(i+1,j) - q(i-1,j)) x (q(i,j+1) - q(i,j-1))` normalised, `q = d * (R + h)` in f64 on the tile's own lattice (COORD-007). Every face has `U x V` outward, so `n` is outward without a flip; a flip is an assertion failure. One definition for the CPU mesh, the compact tile and every test oracle (the canonical normal of GEN-019). The f64 form is accurate to 1.3e-6 degrees at level 22 and 6.5e-5 at level 28 (the unit direction's rounding is the floor); no radius-first form gains.
2. **One batch per tile**: the lattice, its one-sample ring without the four ring corners, and the origin (GEN-020); about 4.5 times fewer evaluations than the stencil.
3. **Seams.** Within a face, two tiles sharing a vertex read the same directions (the same lattice integers) and compute bit-identical normals. A ring point beyond a face edge is the neighbour tile's own lattice direction one cell inside it, on the row that continues the shared edge, found by the integer axis table (which axis, which sign, the along-edge index kept or mirrored), never by projecting the extended cube plane back (that lands off the vertex's row); both tiles of an edge then read the same four points. At a cube corner the three tiles read the same three edge points; each orders them canonically (by the face pair of the edge they lie on) and forms `(p1 - p0) x (p2 - p0)` with the same operands, then orients it outward: bit-identical normals. Tiles of different levels are as today (skirts, morph).
4. **The read set is unchanged**: the ring lies within one lattice cell of the tile, inside the footprint halo of WDEF-007; no read record, footprint, key composition or refresh set changes.
5. **`MESH_VERSION` 9** (GEN-010): every tile key, persistent mesh key, pre-filled pack key and committed known answer is made again once; repro bundles of version 8 are refused with the versions named (TEST-008).

## Consequences
- A tile job costs up to about 4 times less (the block gathering does not shrink with the evaluations); the drawn-exact time of the edit-latency case at 200 m with a 150 m stamp falls from 508 ms to about 180 ms by the SPIKE-23 model (an estimate, to be measured in a perf window).
- Normals differ from before by the figures above; fine detail is shaded with about the same two-cell filter, neither smoother nor sharper. At edge and corner vertices the neighbour-lattice ring bends the stencil across the edge: first-order there; the accuracy test sets its own bound for them.
- Goldens: the debug and shaded goldens that show normals or lighting are declared changes (AUTO-E, ADR 0017 with ADR 0061 point 3; named in the change's Expected golden changes with their bound tests); every lit, sky and water golden that moves goes to the owner's `/bless` with the hillshades as evidence.
- The spec delta, the new tests (including exact equality of the shared vertices' normals at edges and the eight corners, at one and three threads) and the golden list are the change's proposal; the evidence names them.
- Easy to reverse: the version bump re-keys everything.

## Licences of adopted code or techniques
None (the centred difference of a height lattice is textbook).
