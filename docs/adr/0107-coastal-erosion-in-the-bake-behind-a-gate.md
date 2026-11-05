# 0107 Coasts from wave erosion and sediment supply in the bake, behind a switch and a gate
Status: accepted (owner, 2026-11-05: SPIKE-102 D6, option B "with a gate and this fallback option"); point 5 amended 2026-11-05 (owner): the fallback after a failed gate is a rule-based coast type (SPIKE-102 F7 and P2b: cliffed where wave exposure, hard rock and near-shore relief are high, a coast step there, taper, fade and band only on depositional coasts), not today's taper everywhere, which erases cliff heights
Date: 2026-11-05
Source: owner, 2026-11-05; SPIKE-102 (F7, P2b, 4.3 D6); SPIKE-94 (cliffs survey); Sunamura 1992 (cliff erosion rate against wave force and rock strength); ADR 0089; ADR 0091; ADR 0102; ADR 0104; ADR 0105

## Context
- Every coast is a 0-60 m taper plus a fade and a shore band; about 80 % of real coasts are rocky or cliffed (Emery and Kuhn 1982).
- A coast type chosen by rule and drawn per type contradicts ADR 0089's drivers-not-types principle.
- The taper lowers coastal land in the carve; a cliff added after the bake cannot restore it (a pass-2 re-run of about 40 % of land tiles).
- Wave power (wind field and fetch, ADR 0104), rock strength (ADR 0102) and sediment supply (ADR 0105) all exist in M9.
- The heightfield carries faces up to 45 degrees; steeper faces are a separate technology (ADR 0091).

## Decision
1. The bake erodes coasts by waves: a rate per coastal cell from wave power against rock strength (Sunamura-type), cutting a platform near sea level and retreating the land behind it; deterministic, fixed-iteration, within the carve steps.
2. Sediment delivered to the coast by pass-1 deposition builds beaches and deltas where supply exceeds wave removal; alongshore transport (spits, barriers) stays out.
3. Cliffs, platforms, beaches, headlands and bays are results; a coast class is derived from the result for materials and the cliff technology, never an input that selects a recipe.
4. The heightfield carries a cliff as a ramp of at most 45 degrees between its top and bottom heights; no later stage (coast fade, ground detail) smooths such a ramp; the cliff technology derives where and why at runtime from the stored heights and the analytic fields; the bake stores no cliff mask.
5. It is behind a switch. A time-boxed gate compares coastal cross-sections and coastline roughness against a cliffed and a depositional GLO-30 tile; if it fails, the planet bake runs with the switch off and today's taper, fade and band.

## Consequences
- Extends ADR 0088 point 4; bake kernel versions change; the bake date moves by the design's estimate.
- The switch off reproduces today's coasts.
- Steep-face shading and cliff meshes (SPIKE-94) need no re-bake; each enters by its own ADR (ADR 0091).
