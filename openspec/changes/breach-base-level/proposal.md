## Why
Inbox #118 and #119 (architect, 2026-11-10): on R3b the depressions behind the window's land cut deepened round over round (p50 5 m to 46 m, maximum 380 m after four rounds).
The breach loop excavated chains that end in an outlet it never lowers; the sill stayed, the lake behind it deepened.

## What Changes
- `carve::exit_base` (new), `round::catchment_round`, `carve::carve_rock`: under the round solver's local clip a breach never cuts below the base level of its chain exit (the outlet's height, flat along the chain), for chains that end above the sea level. Chains that end at the sea have no base.
- GEN-162 added (generator-pipeline).
- `tests/bake_round`: a closed bowl behind a cut outlet does not deepen round over round (fails without the rule; round 1 grows 6 % from the sweeps' incision, 4.6 % with no breach at all, then it falls); a bowl open to the sea is opened by round 1.

## Impact
- Code: `generators::bake::{carve, round}`, `tests/bake_round`. The default rules, K7 and the sediment pass keep today's breach (the default-rules digest and the stage manifest are unchanged; a first gate on any non-default rules moved K7's stage `k7-f1-t3-4` and was narrowed to the local clip); a sea-outlet build is bit-identical (k=1 phase plan: H1, z after the sediment pass and G-CON equal to main).
- R3b (window 1,020 cells, margin 200), depressions of more than 1 m under the fill after four rounds: region proper 176 cells (0.05 % of land, as before the change), the window's land-outlet depressions 5,428 cells (maximum 23.6 m; 30,909 cells and 380 m before). A first rule with the base rising by the breach slope along the chain left region-proper pits at 0.75 % (a sill within the slope profile stayed over a pit behind it, found by the numerics review); the flat base does not.
- Not in this change: the growth of the pits with k (open).
- R3b gates with the final rule (`region_scale gates 200`, seed 15, face 2): G-ROUND 0 and G-DET 0 cells differ, G-PART 0 receivers and 0 A_d differ, G-CON at most 4.4e-16 confluence and 5.9e-15 outlet error; G-LAND 17 of 343,014 land cells unclassified (0.005 %, bound 1 %); pits of the window 6,418 (0.729 %) at the start of the sediment pass, 5,616 (0.638 %) after pit spreading (6,313 of the 6,418 in kept basins).
