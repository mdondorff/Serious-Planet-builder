## Why
The planner's critique of round 1 (inbox #40, 2026-11-02): thin straight white streaks (needles, high-ground width p10 39 km), one uniform straight polar wall, mountains 5.5 % of land while land above 2 km is 11.3 %, and the step of `k` to be checked again.

## What Changes
- **Class source (planet-generators `classes`):** belts bend (the centreline moves sideways by up to 120 km, low-frequency noise); a segment tapers in width and height to its ends and its width varies along the strike (never under 120 km half-width); segments cut by noise of frequency 2.8; the mountains' membership is `min(1, 2 core)`; belt amplitude 7500 m (from 7000 m after the taper lowered the highs), plateau uplift 800 m (from 1200 m). `CLASS_SOURCE_VERSION` 3.
- **Spec text:** GEN-094 to GEN-096 (planned, in `m9-bake`) rewritten to match.
- Not in this change: the continents (round 2), the step limit of `k` (reported again), wiring into the bake.

## Capabilities
### Modified Capabilities
- `generator-pipeline`: GEN-094 to GEN-096 (planned, in `m9-bake`)
