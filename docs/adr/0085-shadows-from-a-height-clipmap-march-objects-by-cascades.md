# 0085 Shadows from a height clipmap march, objects by cascades
Status: accepted (owner, 2026-10-30; proposed by opus-planner, 2026-10-29, from SPIKE-56); points 3 and 9 amended by ADR 0090 (proposed)
Date: 2026-10-29
Source: SPIKE-56 phases 1 and 2 (`spikes/m9-shadow-architecture/results/results.md`); owner's Plateau 6 report (2026-10-29); ADR 0083; CON-11, CON-14

## Context
- The horizon-map shadows (GEN-044/045, REND-038/041/043, STRM-048/060/064) are baked per chunk, take seconds to follow an edit, show chunk-aligned patches, and ignore the preview stamp (owner's Plateau 6 report).
- A sun ray can only meet relief within `sqrt(2 R dh)` of the receiver: 252 km for 5 km of relief, 339 km for 9 km; under 50 km for a sun above 5 degrees.
- The resident set R holds tiles in all directions around the camera at their distance level (STRM-015); tiles sit in storage-buffer slots, not textures (REND-016).
- Prototype on GLO-30 terrain, 1440p, RTX 3080 Ti Laptop: the half-resolution march costs 0.5-1.2 ms with the sun at 15-25 degrees and 1.6-2.2 ms at 2 degrees; a full clipmap rebuild costs 1.2-1.3 ms.
- Shadows are lighting: presentation data with no CPU twin (CON-14 v1.3).

## Decision
1. **Layer 1, terrain occluders.** Every pixel with a depth (terrain, water, any object) gets its sun visibility from a ray march over terrain heights toward the sun; the planet's own shadow (sun below the geometric horizon) is analytic.
2. **Height clipmap.** The march reads a camera-centred clipmap: nested `R32Float` levels of 1024^2 texels, texel size doubling per level, out to at least 350 km, each with a max-height pyramid. A compute pass gathers it from the resident tiles of R and rewrites only regions whose tiles changed, that scrolled, or that a preview stamp touches.
3. **Curvature baked by the gather.** Each texel stores its height in the anchor's tangent frame; the sphere's drop comes from per-tile coefficients built in f64 on the CPU (CON-03, CON-11). The march contains no planet-radius term and needs no declared-bound exception.
4. **Coarse layer.** Texels outside R and rays leaving the clipmap read a global cube built from the floor tiles (levels 3-4) holding the maximum height per texel. It over-shadows slightly at a grazing sun; a mean height misses far shadows.
5. **The march.** Half resolution, max-mip skipping, a penumbra from the closest approach to the sun's disc, depth-aware upsample, and temporal accumulation that rejects history where the clipmap changed. Below about 5 degrees of sun the half-resolution rays are spread over two frames.
6. **Integration.** The terrain pass writes the unshadowed direct-sun term to its own target; `sky-composite` applies the shadow before the aerial perspective; water reads the same mask.
7. **Retirement.** The horizon maps and their bakes, jobs, pack records and textures are removed once layer 1 is live; no far-field horizon cache is kept.
8. **Layer 2, object occluders (M10+).** Cascaded shadow maps of objects only, near the camera, combined with layer 1 by `min`; large static objects (buildings, forest canopy) also splat a max height into the clipmap gather. Hardware ray tracing (wgpu `EXPERIMENTAL_RAY_QUERY`) is an optional quality path only, never the CI path.
9. **Determinism.** Layer 1 uses no atomics and no order-dependent reduction: goldens per adapter (ADR 0061); the owner's cross-machine exception is not used.

## Consequences
- ADR 0083's share table gains layer 1 at 1.5 ms of GPU at 1440p (march, upsample, temporal, clipmap upkeep) and a reserved layer 2. REND-044's 1.5 ms no longer covers shadows. The implementation's perf window confirms the 1.5 ms with the two-frame spread at a low sun.
- A terrain edit, and a dragged preview stamp, shadow correctly in the frame the clipmap is gathered.
- Overhangs cannot be a heightfield: far shadows of bridges and arches are a stated limit until layer 2 reaches them.
- The forward terrain pass gains a second colour target (15-30 MB at 1440p).
- Clipmap and coarse layer take about 75 MB of VRAM, replacing the horizon textures (56 MB of coarse horizon plus the chunk maps).

## Licences of adopted code or techniques
None (published techniques: heightfield ray marching, clipmaps, max-mip skipping, cascaded shadow maps; no code adopted).
