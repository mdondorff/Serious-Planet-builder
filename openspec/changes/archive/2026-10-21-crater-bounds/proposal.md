# Change: crater-bounds

## Why
- SPIKE-26 (the item sweep of the owner's 77 items): part of the remaining plan refusals sit at the rims of craters (Crater 6, Platonmax). A cap that touched a crater's rim took the range from the bowl to the rim's crest, 50 m of bound for 2 m of relief over a cap of 28 m on a 250 m crater, so a fine tile had a loose extent and its morph interval was empty.
- The refusals at the owner's brush clusters are not bounds: the 44 stacked raise strokes are real 220 m spikes (the bounds are exact to 1 mm); they are the subject of ADR 0074.

## What changes
- GEN-026: a crater's range over a cap is the range of its profile, which rises to the rim's crest at `x = 0.85` and falls beyond it: the end values and, where the cap holds the crest, the crest's. No other stamp kind changes (hills and plateaus were measured tight, ratio 1.02 to 1.05 and 1.00; lines are monotone).
- No output, key or version changes: only the bounds.

## Tests
- `stamp_stage::the_bounds_of_a_crater_are_exact_over_a_small_cap_at_its_rim_and_hold_for_random_caps` (Crater 6: 250 m, 2 km; Platonmax: 672 m, 200 m; a 40 m crater): every sampled height is within the bounds and the growth of the extent is within 1.1 times the sampled relief plus 0.5 m over caps of 4 to 40 m; it failed with 58 m more.
- The existing bounds tests (random caps, the rim of a 1 m crater) pass.

Expected golden changes: none (bounds only; a plan whose tiles had loose crater bounds may select other tiles, no height changes; the gpu_stamps hash is checked by prepush).
