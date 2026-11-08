# 0115 Scale as a parameter and the store budget
Status: proposed (spikes session for opus-planner, 2026-11-08, inbox #100); the owner accepts after the external review and the D2-D10 pass
Date: 2026-11-08
Source: `docs/design/terrain-target.md` §4, §10, §12; SPIKE-105 §3 (X11-X13), §13.5 (D6, D7); ADR 0045 point 2, ADR 0108

## Context
- Grids are fixed counts per cube-face edge (`BakeShape::production`); the planet's scale is implicit.
- R5a: moon to larger than Earth in the engine; R7: about 40 GB at Earth, larger planets may use simpler features.
- At Earth the store is about 36 GB before hydrology (field bake 11.7 GB, layer and channels about 24 GB).

## Decision
1. Grid counts are derived from one integer per body, `k`, chosen from the target local cell size: base lattice `2^k`, local `6 x 2^k`, regional `3 x 2^(k-2)`, H0 `3 x 2^(k-4)`. Earth at `k` = 14 is today's set.
2. Physical and procedural parameters live in the preset (ADR 0108), including gravity and rotation; radius sets no geology.
3. A body without a sea or runoff skips K4 to K7.
4. The 40 GB limit is a hard gate at Earth on real serialized data, including indexes, keys, graph metadata and edit overrides; peak temporary storage is reported separately.
5. Field narrowing comes first; a coarser local grid (about 200 m) is a fallback only.
6. Larger planets take coarser cells at the same counts (2x Earth at `k` = 14, 204 m), unless given a store budget of their own.
7. A world build has its own memory budget, separate from CON-21's runtime 8 GB: 12 GB peak on the reference machine (proposed; a resident routing and round of the largest landmass needs about 10 GB, F8), or a partitioned route.

## Consequences
- ADR 0045 point 2 becomes normative. Gate G-STORE, G-SCALE.
- Point 7 is a new budget; the owner decides it with this ADR.
