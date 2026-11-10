## Why
Inbox #118 and #119 (architect, 2026-11-10): on R3b the depressions behind the window's land cut deepened round over round (p50 5 m to 46 m, maximum 380 m after four rounds).
The breach loop excavated chains that end in an outlet it never lowers; the sill stayed, the lake behind it deepened.

## What Changes
- `carve::exit_base` (new), `round::catchment_round`, `carve::carve_rock`: under the round rules a breach never cuts below the base level of its chain exit (`base[outlet] = z[outlet]`, `base[c] = base[receiver] + breach_slope dist[c]`), for chains that end above the sea level. Chains that end at the sea have no base.
- GEN-162 added (generator-pipeline).
- `tests/bake_round`: a closed bowl behind a cut outlet does not deepen round over round (fails without the rule); a bowl open to the sea is opened by round 1.

## Impact
- Code: `generators::bake::{carve, round}`, `tests/bake_round`. The default rules keep today's breach (the default-rules digest is unchanged); a sea-outlet build is bit-identical (k=1 phase plan: H1, z after the sediment pass and G-CON equal to main).
- R3b (window 1,020 cells, margin 200): region-proper depressions after four rounds 0.05 % to 0.75 % of land (p50 2.4 m, maximum 9.2 m, falling round over round); the window's land-outlet depressions 30,909 cells (maximum 380 m) to 16,833 (maximum 24 m).
- Hypothesis (not shown): the old excavation behind the cut also drained pits inside the region.
- Not in this change: the growth of the pits with k (open).
- R3b gates with the fix (`region_scale gates 200`, seed 15, face 2): G-ROUND 0 and G-DET 0 cells differ, G-PART 0 receivers and 0 A_d differ, G-CON at most 4.3e-16 confluence and 4.1e-15 outlet error; G-LAND 65 of 343,014 land cells unclassified (0.019 %, bound 1 %); pits of the window 23,726 at the start of the sediment pass, 22,786 (2.588 %) after pit spreading (23,337 of the 23,726 in kept basins).
