# 0115 Scale as a parameter and the store budget
Status: proposed (spikes session for opus-planner, 2026-11-08, inbox #100; amended after the ADR review, inbox #102). Acceptance blocked by the owner's D2-D10 pass; point 7's budget is set by the owner. The budgets are hard gates, not facts: production needs G-STORE and G-SCALE measured (A/B gate)
Date: 2026-11-08
Source: `docs/design/terrain-target.md` §4, §10, §12; SPIKE-105 §3 (X11-X13), §13.5 (D6, D7), §16, §18; ADR 0045 point 2, ADR 0108

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
7. A world build has its own memory budget, separate from CON-21's runtime 8 GB: 12 GB peak on the reference machine (owner, 2026-11-08), measured as the process's peak. The partitioned route is the normal path for large landmasses and must pass the same gates; the resident route is an optimisation for a landmass that fits the budget, its threshold measured (G-SCALE).

## Consequences
- ADR 0045 point 2 becomes normative. Gates G-STORE, G-SCALE (one large and one small body).
- Point 7 is a new budget; the owner decides it with this ADR.
- The resident rounds of the largest landmass are measured (7.7 GiB, 55 bytes a cell, on a compact grid of the landmass, A3 §6.6); the H1 pass is extrapolated above 12 GB (87 bytes a cell at a quarter of the cells); the partitioned route's non-resident form is not built.
