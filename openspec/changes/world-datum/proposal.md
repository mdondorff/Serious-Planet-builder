## Why
GEN-166 stores the round surface as f32; with the flood epsilon fixed at 1e-3 m, a world with a 40 km mountain would collapse the carved epsilon chains near its summits.

## What changes
- The f32 rounds stay in the frame of the sea level (the finest spacing at coasts, outlets, lakes and the K7 floor); the cost goes to the summits.
- A per-world flood epsilon `epsilon_w = max(epsilon_profile, spacing(R) x (1 + 2^-20))`, `R = max |h|` (the carve measures heights from its sea level, 0), and a named `epsilon_max` above which the rounds run in f64.

## Impact
generators (bake::precision, new). Earth's span keeps epsilon 1e-3 and the keys of today.
