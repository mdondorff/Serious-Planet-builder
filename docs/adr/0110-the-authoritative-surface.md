# 0110 The authoritative surface: one composition, one base read, one elevation source
Status: proposed (spikes session for opus-planner, 2026-11-08, inbox #100; amended after the ADR review, inbox #102). Acceptance blocked only by the owner's D2-D10 pass. Production: G-SURF, G-PROFILE, G-BASIN (A/B gate)
Date: 2026-11-08
Source: `docs/design/terrain-target.md` §3; SPIKE-104 §3; SPIKE-105 §6, §11 (points 9A), §13, §18

## Context
- The bake and the runtime read different bases: pass 2's window takes the base every 4 cells (407 m), the runtime the store's 611 m lattice (SPIKE-104: rms 12 m, max 209 m).
- A C1 read of a D8 channel network re-dams it at the local grid (13.3 % of land in pits, SPIKE-104).
- The fill depth, the channels and the regional change are each candidates for separate runtime terms, each a further elevation source.
- The runtime adds the fill depth back until lake water exists (ADR 0101 point 3).

## Decision
1. The surface is `S = B + L + C + A`, composed in f64 in that order: the base `B`, the local change `L` (faded at the coast), the creases `C`, the above-erosion items `A`.
2. `B` is the store lattice's C1 read (GEN-107), plus below-erosion height items; it is the same read for every consumer (K4, K5, K7, tiles, CPU systems). Pass 2's stride-4 base is retired.
3. `L` is the only baked elevation term the runtime reads. The regional change, channels and basins are inputs of the bake. `C` is evaluated at read time on the CPU and stores nothing of its own; with the relief switch on, its amplitude reads the baked driver `k` (GEN-099).
4. Channels are burned in at the local grid (K7 lowers cells along a segment's level profile, then breaches, incises and settles); they are not a runtime height term. A later bank detail may lower the ground only, within a declared bound.
5. The runtime no longer adds the fill depth; basins show their floor and the water field draws lakes. This takes effect when G-BASIN passes and the lake producer exists.
6. Every derived layer folds `base_id` into its key; a mismatch is refused. `base_id` covers the lattice key, the C1 read's version and parameters, the mapping, radius and `k`, the sea level, the below-erosion items at the tile, and the stored forms (A3 §3.2).

## Consequences
- Amends ADR 0088 points 2 and 5, ADR 0099 point 3, ADR 0100; replaces ADR 0101 point 3.
- Gates G-SURF, G-CON, G-PROFILE (channels through the complete read).
