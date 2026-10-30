## Why
ADR 0085 (accepted, owner, 2026-10-30) replaces the horizon-map shadows with a per-pixel sun march over a camera-centred height clipmap. The horizon maps follow an edit in seconds, show chunk-aligned patches, ignore the preview stamp (owner's Plateau 6 report, 2026-10-29) and hold the last key of the retired chunk lattice. SPIKE-56 measured the march at 0.8-2.2 ms at half resolution on Rasierklinge (1440p) and picked the drop baked by the gather (no planet radius in the shader). This change is the build plan; it is implemented as the PRs of `tasks.md`, each green on its own.

## What Changes
- **Frame (GPU-free, CON-03):** a clipmap plan per frame: 10 to 12 levels of 1024^2 texels around the camera's ground point, the anchor's tangent frame, per-level tile lists from R, per-tile f64 coefficients with the sphere's drop handed over tile-local (CON-11), dirty regions (landed, evicted, scrolled, previewed) (REND-046).
- **Render:** a compute gather from the tile slots into `R32Float` array levels with max mips, applying the frame's preview stamps (REND-047); a coarse max-height cube from the floor tiles (REND-048); the march at half resolution with max-mip skipping, a penumbra and the analytic planet shadow (REND-049); a depth-aware upsample and temporal accumulation, with the rays spread over two frames below about 5 degrees of sun (REND-050); the terrain pass writes the unshadowed sun term to its own target, `sky-composite` applies the shadow before the aerial perspective, water reads the same mask (REND-051).
- **Budgets:** layer 1 at most 1.5 ms of GPU at 1440p (REND-052, BUDG-002); REND-044 keeps the material map only.
- **Determinism:** no atomics, no order-dependent reduction; goldens per adapter (REND-053, ADR 0061).
- **Retired** once layer 1 draws: the chunk and coarse horizon maps, their bakes, jobs, pack records, textures and views (GEN-044, GEN-045, GEN-046, GEN-085, REND-038, REND-041, REND-043, STRM-045, STRM-048, STRM-049, STRM-050, STRM-057, STRM-059, STRM-064, TEST-065, TEST-066); the open changes `m8-horizon-share` (STRM-061) and `m8-horizon-march-simd` (GEN-086) are archived without syncing.
- **Viewer:** J's horizon view goes; its shadow view shows the march's visibility (VIEW-054).
- Not in this change: layer 2 (object cascades, M10+), hardware ray tracing, occluder splats of objects into the gather.

## Capabilities
### New Capabilities
- none (the requirements join `renderer`)

### Modified Capabilities
- `renderer`: REND-046 to REND-053 added; REND-044 reworded; REND-038, REND-041, REND-043 removed
- `compute-budgets`: BUDG-002 names layer 1 and a reserved layer 2
- `viewer`: VIEW-054 without the horizon view
- `generator-pipeline`: GEN-044, GEN-045, GEN-046, GEN-085 removed
- `streaming-cache`: STRM-060 without its horizon clause; STRM-045, STRM-048, STRM-049, STRM-050, STRM-057, STRM-059, STRM-064 removed
- `testing`: TEST-054 without horizon jobs; TEST-065, TEST-066 removed

## Impact
- Code: new `frame::clipmap` (plan), `render::shadow` (gather, coarse cube, march, upsample, temporal; WGSL), the terrain pass's second target, `sky-composite` and water bindings; removed `generators::{horizon, horizon_lookup, coarsehorizon}`, `frame::horizonview`, `render::{horizon, horizon_wgsl}`, `app::horizonworld`, the horizon half of `streaming::chunks`, `cache::horizon_store`, the horizon parts of `stills`, `editperf`, `gate`, `envelopeperf`, `litperf`.
- The chunk lattice (GEN-034/035) loses its last user; its removal is B6's or a follow-up's (design.md, open point 3).
- VRAM: about 75 MB (clipmap and coarse cube) replaces 56 MB of coarse horizon plus the chunk maps. Terrain pass: one more colour target, 15-30 MB at 1440p.
- Goldens: every lit golden changes once (shadow source); the debug views do not. Per adapter, through `golden-review`; first lit goldens of the new shadow go to the owner's `/bless` (ADR 0017).
- Stores: the coarse horizon channel in `C:/DEV/SeriousPlanetData/global16384` is no longer read; the store is not rebuilt (its definition hash is unchanged).
- Owner: a look review of the shadows (stills and a short drag video) before the horizon maps are deleted (tasks.md, PR 6).
