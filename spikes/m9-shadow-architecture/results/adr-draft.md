# 00NN Shadows from a height clipmap march, objects by cascades
Status: proposed (drafted by the spike session for opus-planner, 2026-10-29; owner, 2026-10-29: the horizon-map shadows are redone)
Date: 2026-10-29
Source: SPIKE-56 (`spikes/m9-shadow-architecture/results/results.md`); ADR 0083; CON-11, CON-14

## Context
- The horizon-map shadows (GEN-044/045, REND-038/041/043, STRM-048/060/064) are baked per chunk, take seconds to follow an edit, and need a separate far-field map with a distance fade.
- A sun ray can only meet relief within `sqrt(2 R dh)` of the receiver: 252 km for 5 km of relief, 339 km for 9 km; under 50 km for a sun above 5 degrees.
- The resident set R holds tiles in all directions around the camera at their distance level (STRM-015); tiles sit in storage-buffer slots, not textures (REND-016).
- Shadows are lighting: presentation data with no CPU twin (CON-14 v1.3); CON-11 v1.4 allows presentation shading a planet-scale constant under a tested bound.
- Objects (towers, trees, vehicles) arrive from M10 and need shadows onto terrain and each other.

## Decision
1. **Layer 1, terrain occluders.** Every pixel with a depth (terrain, water, any object) gets its sun visibility from a ray march over terrain heights toward the sun; the planet's own shadow (sun below the geometric horizon) is analytic.
2. **Height clipmap.** The march reads a camera-centred clipmap of heights above the sphere: nested levels of square `R32Float` layers, texel size doubling per level, out to at least 350 km, each with a max-height pyramid. A compute pass gathers it from the resident tiles of R through per-tile mappings built in f64 and handed over tile-local (CON-03, CON-11); it rewrites only regions whose tiles changed or that scrolled.
3. **Coarse layer.** Rays leaving the clipmap continue in a global height cube from the floor tiles (levels 3-4), which also serves the orbit view.
4. **The march.** Half resolution, max-mip skipping, a penumbra from the closest approach to the sun's disc, depth-aware upsample and temporal accumulation that rejects history where the clipmap changed.
5. **Integration.** The terrain pass writes the unshadowed direct-sun term to its own target; `sky-composite` applies the shadow before the aerial perspective; water reads the same mask.
6. **Retirement.** The horizon maps and their bakes, jobs, pack records and textures are removed once layer 1 is live; no far-field horizon cache is kept.
7. **Layer 2, object occluders (M10+).** Cascaded shadow maps of objects only, near the camera, combined with layer 1 by `min`; large static objects (buildings, forest canopy) also splat a max height into the clipmap gather. Hardware ray tracing (wgpu `EXPERIMENTAL_RAY_QUERY`) is an optional quality path only, never the CI path.
8. **Determinism.** Layer 1 uses no atomics and no order-dependent reduction: goldens per adapter (ADR 0061); the owner's cross-machine exception is not used for layer 1.

## Consequences
- ADR 0083 gains shadow shares (layer 1 and a reserved layer 2) in the share table spec; REND-044's 1.5 ms is split between the shadow and the material map.
- A terrain edit shadows correctly in the frame its refreshed tile lands; a preview stamp's shadow lags until then.
- Overhangs cannot be a heightfield: far shadows of bridges and arches are a stated limit until layer 2 reaches them.
- The forward terrain pass gains a second colour target (15-30 MB at 1440p).
- R's horizon cut may miss an occluder just below the camera's horizon at a grazing sun; the coarse layer covers it at coarse resolution.

## Licences of adopted code or techniques
None (published techniques: heightfield ray marching, clipmaps, max-mip skipping, cascaded shadow maps; no code adopted).
