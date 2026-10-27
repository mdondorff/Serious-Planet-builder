## Why
The owner test of 2026-10-27 (frame-102263; `docs/milestones/M8/owner-test-2026-10-27/b6/README.md`): a smooth dark blob 20 to 30 km wide hid lit crests under a sun below about 1.8 degrees. It is the coarse horizon map itself: one horizon per 19.5 km observer cell, the highest angle of the run's heights seen from the cell's mean ground, applied to every point of the cell. Where the mean sits below a plateau sunward the whole cell is shadowed, crests above the plateau included (8.0 % of the pixels differ with the map off at 0.9 degrees).

## What Changes
- The stored map holds per observer cell two horizons per azimuth (the observer at the cell's lowest and at its highest ground) and the two heights in 40 m steps: `2 a + 2` bytes a cell (`record_len`), version 2 of the march. 25 MB becomes 56 MB of texture at 512 cells a face.
- The shader blends the two by the fragment's height above the sea (`blend_elevation` is the reference); the coarse map takes three texture reads of the cell instead of one. `shadow_factor`, `horizon_at` and `horizon_view_colour` take the height; the measurement variant of REND-044 is adjusted to the new call.
- A stored coarse map of version 1 is not found under the new key: `generate --coarse-horizon` makes the new one from the stored coupled run.

## Impact
- Spec: GEN-045, STRM-045, REND-038. Code: `generators/coarsehorizon.rs`, `cache/horizon_store.rs`, `frame/horizonview.rs`, `render/{horizon,horizon_wgsl,terrain,atmosphere}.rs`, `app/horizonworld.rs`.
- Goldens: the lit views whose frames read a coarse map differ where the map shadowed a point of a cell whose mean was below the plateau; the horizon-view goldens use a synthetic map (identical low and high horizons) and do not move.
- Cost: terrain-pass GPU time for three more reads; criterion 5's margin is 5 to 9 % at river-r1 300 m: re-measure in the acceptance window.
