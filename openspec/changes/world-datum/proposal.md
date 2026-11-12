## Why
GEN-166 stores the round surface as f32 about 0, so the f32 spacing is the spacing at the highest height; a world with a 40 km mountain would collapse the carved epsilon chains.

## What changes
- A world datum (the midpoint of the world's base height bounds) that the f32 rounds narrow about.
- A per-world flood epsilon `epsilon_w = max(epsilon_profile, SPACING_FACTOR x spacing(R))`, `R = max |h - datum|`, and a named `epsilon_max` above which the rounds run in f64.
- The datum enters the layer and catchment keys only when non-zero; the choice is recorded in the bake key and the MARK line.

## Impact
generators (bake::precision, carve, round), cache (layer key). Earth's span keeps epsilon 1e-3 and the keys of today.
