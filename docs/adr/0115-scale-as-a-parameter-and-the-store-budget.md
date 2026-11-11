# 0115 Scale as a parameter and the store budget
Status: accepted for the prototype (owner, 2026-11-11: "approve with the conditions mentioned", after the external review of 2026-11-11), with the conditions below; point 7's budget set by the owner (2026-11-08, 2026-11-09, 2026-11-11). Production: G-STORE and G-SCALE measured at the A/B gate
Date: 2026-11-08
Source: `docs/design/terrain-target.md` §4, §10, §12; SPIKE-105 §3 (X11-X13), §13.5, §16, §18; ADR 0045 point 2, ADR 0108

## Context
- Grids are fixed counts per cube-face edge (`BakeShape::production`); the planet's scale is implicit.
- R5a: moon to larger than Earth in the engine; R7: about 40 GB at Earth, larger planets may use simpler features.
- At Earth the store is about 36 GB before hydrology (field bake 11.7 GB, layer and channels about 24 GB).

## Decision
1. Grid counts are derived from one integer per body, `k`, chosen from the target local cell size: base lattice `2^k`, local `6 x 2^k`, regional `3 x 2^(k-2)`, H0 `3 x 2^(k-4)`. Earth at `k` = 14 is today's set.
2. All are named parameters of the world definition (ADR 0108). The body owns its physical parameters (radius, gravity, rotation, tilt, atmosphere, sea level); a geology preset owns K, U, lithology and class rules; hydrology and climate own runoff and evaporation. A preset supplies defaults. Radius sets no geology.
3. K4 to K7 run when the body's hydrology switch is on, skip when it is off. Hydrology on needs an outlet: in the MVP the sea. A body with runoff and no sea is refused with an error (closed basins as outlets are post-MVP); a body with a sea and no runoff switches hydrology off.
4. The 40 GB limit is a hard gate at Earth on real serialized data, including indexes, keys, graph metadata and edit overrides; peak temporary storage, replacement-commit space and retained versions are reported separately.
5. Field narrowing comes first; a coarser local grid (about 200 m) is a fallback only.
6. Larger planets take coarser cells at the same counts (2x Earth at `k` = 14, 204 m), unless given a store budget of their own. A scaling model in `k` and radius estimates cells, store and build memory (A3 §4.3).
7. A world build has its own memory budget, separate from CON-21's runtime 8 GB: a hard limit and a lower working target that keeps headroom (owner, 2026-11-08 and 2026-11-11), both read as peak commit charge of the complete build on the reference machine; the values are in BUDG-016. The compact resident route is the normal path (owner, 2026-11-09); the partitioned route stays a tested equal. A tile-wise route is prototyped only if the unavoidable working set exceeds the budget after retention and caches are removed.
8. Caches are evictable; eviction never changes a result or breaks undo and replay (owner, 2026-11-09; A3 §6.6).

## Consequences
- Supersedes ADR 0045 point 2 (a ceil on the bake cell) with point 1's rule (the largest `k` whose local cell is at least the target; Earth 101.8 m at `k` = 14). Gates G-STORE, G-SCALE (one large and one small body).
- Point 7 is a new budget.
- The resident rounds of the largest landmass are measured (7.7 GiB, 55 bytes a cell, on a compact grid of the landmass, A3 §6.6); the partitioned route measured more memory and time as built (8,854 MiB, 2,974 s against 7,858 MiB, 1,716 s), so the 2026-11-08 choice of it as the normal path, made on the assumption that it bounds memory, is replaced.
- Since measured: the complete resident build at k=4, 11,387 MiB worst of two runs (GEN-158; ledger G-SCALE).

## Conditions (external review, 2026-11-11)
1. The store budget stays a hard constraint; smaller and larger bodies are validated, since an Earth-only result does not show planetary scalability. Numerical scaling and computational scaling are reported apart; a phenomenon may use its own resolution where justified.
2. The compaction peak and temporary disk use are measured, not only the steady-state store. Replacement is atomic and crash-safe, with tests.
3. Memory headroom below point 7's hard limit is kept; optimisation comes before a coarser grid, which needs evidence that it is necessary.
