# 0102 Lithology: an analytic erodibility K(x, z) drives the erosion carve
Status: accepted (owner, 2026-11-05: SPIKE-102 D2, "yes, in M9, with the preview"); point 3 read 2026-11-06 (planner, #1216 spec-guardian): K multiplies the class carve factor rather than replacing it (class factors such as the sand sea stay), and K carries a fixed incision-matched scale of the default preset (GEN-127; ADR 0108 point 3)
Date: 2026-11-05
Source: owner, 2026-11-05; SPIKE-102 (`spikes/generator-archetypes/results/results.md` sections 2, 3, 4.1 rows 3 and 4); SPIKE-81 (plate provinces); ADR 0088 point 4; GEN-116; change `lithology-k`

## Context
- The carve uses one erodibility for all rock; only the class carve factor varies it (0.5 for sand sea, 1 elsewhere).
- Mesas, buttes, cuestas, badlands, hard ridges against soft valleys, karst and the cliff-or-beach choice all need erodibility that varies in space and with depth.
- A map that varies only across the surface cannot make a hard cap over soft rock: the rock that erosion meets must depend on the height it has cut to.
- Adding erodibility after the planet bake means a full re-bake and a second tuning of the class amplitudes, the scorecard and the materials.

## Decision
1. Lithology is an analytic field, CPU-canonical (`libm`, integer hashing), evaluated on demand and never stored.
2. It holds hardness per plate province times noise, a stack of strata whose erodibility varies with height (tilted or folded by a smooth offset, with a caprock flag), and a soluble flag.
3. Both passes of the erosion layer read the erodibility at a cell's current height in each step, in place of the class carve factor.
4. Whole-planet preview maps of hardness, strata and the soluble flag come before any bake that uses it; values come from that preview round.

## Consequences
- Extends ADR 0088 point 4 (with ADRs 0099 and 0101): the carve's erodibility becomes a field.
- Relief changes everywhere; the class amplitudes and the realism scorecard are re-tuned on the result.
- Strata thinner than the bake resolves belong to the runtime's detail and rock colour, which can evaluate the same field without storage.
- Faults and folds can extend the field later without a change to the bake's structure; truly 3D bodies (intrusions, caves) are out of scope.
