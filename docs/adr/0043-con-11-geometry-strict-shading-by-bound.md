# 0043 CON-11: geometry stays strict, presentation shading is judged by a tested bound
Status: accepted (owner, 2026-10-15, opus-planner session; amends constitution CON-11, v1.4; amends ADR 0035 point 3 in part)
Date: 2026-10-15
Source: owner's question of 2026-10-15 on atmosphere shading and CON-11; ADR 0035 (fields and presentation data); ADR 0040 (atmosphere) and its numerics review of 2026-10-15; constitution CON-11, CON-13, CON-14

## Context
CON-11 (v1.3) said "No shader computes values of planet-radius magnitude". Its purpose is precision: f32 cannot hold planet-scale positions finely enough, and errors in geometry become visible jitter, cracks and depth fighting (CON-13). The owner's reading: values like the planet radius are acceptable in shaders when there is no risk of visible inaccuracy and the purpose is purely visual. For the atmosphere, R's own f32 rounding (about 0.5 m of 6,371 km) is invisible; the real risk is cancellation between planet-sized values (r - R, r^2 - R^2), which is about 33 % wrong at 1 m altitude in naive f32 and can make sky and haze values step from frame to frame as the camera moves, while being harmless from 2 km up. ADR 0035 point 3 had kept CON-11 in full for presentation data, which would have forced special formulations even where they are not needed.

## Options (with trade-offs)
1. Keep CON-11 literal: no planet-sized values in any shader. Safe; forces audited formulations or CPU-side work for every shading feature that needs planet geometry.
2. **Split by purpose** (chosen): geometry strict; presentation shading allowed planet-sized values when its declared bound, including temporal stability under camera motion, is tested at the worst case.
3. Drop the rule for all shaders. Loses the protection where errors are visible geometry.

## Decision
1. **Geometry stays strict:** vertex positions, depth, and anything that places a pixel reach the GPU only as tile-local or camera-relative f32 values, and no shader computes geometry from values of planet-radius magnitude (unchanged intent of CON-11, with CON-13).
2. **Presentation shading** (ADR 0035's presentation data: sky, haze, lighting, water appearance, and so on) may use values of planet-radius magnitude when its declared bound is tested at the worst case, **including temporal stability under camera motion at 1 m altitude** (a slow camera move must not make the output step or shimmer beyond the bound). The known failure to test for is cancellation between planet-sized values, not their mere presence.
3. Constitution text (v1.4): CON-11 as recorded in `docs/constitution.md`. ADR 0035 point 3 ("CON-11 and CON-13 still apply" to presentation data) now reads with CON-11's v1.4 text, which already contains this carve-out.

## Consequences
- ADR 0040 needs no constitution exception: SPIKE-09 measures whether the simple f32 form meets the bound (static and in a slow walk at 1 m) and the simplest passing form is used; altitude-relative primitives are the fix only where it fails.
- Every presentation-shading spec that uses planet-scale values includes a temporal-stability scenario in its bound test.
- Water surface geometry and terrain stay tile-local (ADR 0041 point 4).

## Licences of adopted code or techniques
None.
