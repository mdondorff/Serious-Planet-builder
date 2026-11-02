## Why
PR 6 of `shadow-clipmap-march` (ADR 0085 point 7). Since PR 5 the lit fragments carry no horizon factor; the sun shadow comes from the march (REND-049, REND-051). The horizon textures, their bind group, the horizon table, the shader's horizon functions, J's horizon view and the viewer's feeding of the renderer have no consumer left.

## What Changes
- `render`: `horizon`, `horizon_wgsl` and the `probe_horizon` exports deleted; the terrain pass's bindings 4 to 7, `horizon_nodes`, `horizon_frame`, `set_horizon`, `set_horizon_coarse`, `clear_horizon`, `horizon_bytes`, `read_horizon_slot` and the horizon table write removed; the lit shaders lose `horizon_decls!`, `horizon_fns!`, the coarse fade and the `vis` argument of the lighting functions; `MeasureVariant` keeps `Product` and `NoMaterial` (`NoShadow` was the product source and `Neither` equalled `NoMaterial`).
- `frame`: `horizonview` deleted; `ShadeView::Horizon` removed and the views after it move down one index (shadow 5, soil depth 6, moisture 7, insolation 8); J cycles normal, shadow, soil depth, moisture, insolation.
- `app`: the viewer starts no horizon jobs, runs no horizon step and uploads no horizon maps; the title no longer says "no horizon shadows"; the stills wait for no maps and no longer read the `chunk_timeout_s`, `horizon_cell_m` and `coarse_horizon` keys of the poses file (an old file still parses); `shadow-cost` times the material map alone.
- Tests deleted with their requirements: `render/tests/gpu_horizon.rs`, `app/tests/gpu_horizon_rig.rs`, the `HorizonRig` unit tests, the pose-wait message test, the coarse-horizon poses-key test; goldens `lit_ground_horizon_synthetic_ev15` and `lit_ground_shadow_synthetic_ev15` of the three adapters.
- Kept for PRs 7 and 8: the horizon bake path (`generators::{horizon, horizon_lookup, coarsehorizon}`, `cache::horizon_store`, `streaming::horizonjobs`, the bakes and the coarse read of `app::horizonworld`, `--horizon-threads`, `--no-horizon-jobs`, the horizon rows of the perf scenes).
- Specs: REND-038, REND-041, REND-043 removed; REND-044 (material map only), VIEW-054 and TEST-048 reworded.

## Capabilities
### Modified Capabilities
- `renderer`: REND-044; REND-038, REND-041, REND-043 removed
- `viewer`: VIEW-054
- `testing`: TEST-048

## Impact
- No pixel of any lit, sky, water or debug frame moves: the lit fragments already carried no horizon factor (REND-051) and the lighting functions only lose a factor of 1.0.
- `stills.txt` loses its two horizon columns; the `shadow-cost` JSON loses `no_shadow_ms`, `neither_ms` and `shadow_ms` (`compiled_out_ms` holds one value, `together_ms` equals `material_ms`).
- The archive of `shadow-clipmap-march` repeats the removals and rewordings of this change; whichever archives second finds them already applied.
