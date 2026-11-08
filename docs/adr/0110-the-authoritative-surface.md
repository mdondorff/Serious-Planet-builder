# 0110 The authoritative surface: one composition, one base read, one elevation source
Status: proposed (spikes session for opus-planner, 2026-11-08, inbox #100); the owner accepts after the external review and the D2-D10 pass
Date: 2026-11-08
Source: `docs/design/terrain-target.md` §3; SPIKE-104 §3; SPIKE-105 §6, §11 (points 9A), §13

## Context
- The bake and the runtime read different bases: pass 2's window takes the base every 4 cells (407 m), the runtime the store's 611 m lattice (SPIKE-104: rms 12 m, max 209 m).
- A C1 read of a D8 channel network re-dams it at the local grid (13.3 % of land in pits, SPIKE-104).
- The fill depth, the channels and the regional change are each candidates for separate runtime terms, each a further elevation source.
- The runtime adds the fill depth back until lake water exists (ADR 0101 point 3).

## Decision
1. The surface is `S = B + L + C + A`, composed in f64 in that order: the base `B`, the local change `L` (faded at the coast), the creases `C`, the above-erosion items `A`.
2. `B` is the store lattice's C1 read (GEN-107), plus below-erosion height items; it is the same read for every consumer (K4, K5, K7, tiles, CPU systems). Pass 2's stride-4 base is retired.
3. `L` is the only baked term the runtime reads. The regional change, channels and basins are inputs of the bake.
4. Channels are burned in at the local grid (K7 lowers cells along a segment's level profile, then breaches, incises and settles); they are not a runtime height term. A later bank detail may lower the ground only, within a declared bound.
5. The runtime no longer adds the fill depth; basins show their floor and the water field draws lakes.
6. Every derived layer folds the base's identity into its key; a mismatch is refused.

## Consequences
- Amends ADR 0088 points 2 and 5, ADR 0099 point 3, ADR 0100; replaces ADR 0101 point 3.
- Gate G-SURF, G-CON.
