# Change: m8-look-review-1-hills

## Why
- Look review 1 judged one site, a river over a plain; the owner's read of incision depth and hillslope diffusion needs a second site in hilly ground (opus-planner, 2026-10-25).
- The anchor must come from a query over the store, not from the eye.

## What changes
1. `[place]` takes `relief_radius_m`, `relief_min_m` and optionally `relief_max_m`: the anchor is then the largest land river among the cells whose coupled-run heights within the radius span `relief_min_m` to `relief_max_m`.
2. `Lattice::radius_m()`.
3. `docs/milestones/M8/look-review-1-hills/`: poses, six stills, README.

## Tests
- Tier A: the relief of a cell on a small lattice, the keys parsed, refused when only one of the pair is given.

Expected golden changes: none (review stills).
