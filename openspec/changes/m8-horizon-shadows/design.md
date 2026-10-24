# Design: m8-horizon-shadows

Status: draft for the planner; the open questions need a decision or a measurement before the step that depends on them. Numbers are starting values from SPIKE-32 (`spikes/realism/results/results.md`) and become spec numbers when each step lands.

## The product
- A horizon map holds, per texel and per azimuth `k` of `A`, the elevation angle of the highest terrain along that azimuth out to the range. The shader compares the sun's elevation with the horizon interpolated between the two nearest azimuths, with a soft edge of fixed width.
- It does not depend on the sun, so any time of day uses the same map (baked once per chunk version).
- Two maps: the chunk's (4 m texels, range 5 km, from the chunk raster plus halo) and the coarse one (global lattice, range about 50 km). The horizon used at a point is the larger elevation of the two at the azimuth.

## Starting numbers (SPIKE-32)
| Quantity | Value |
|---|---|
| texel | 4 m |
| azimuths `A` | 16 (22.5 degree steps; fans near occluders) |
| range | 5 km (chunk), about 50 km (coarse) |
| step law | steps growing 4.5 % + 2 m |
| storage | 1 MB per km2 (RGBA8, four array layers) |
| bake | 0.7 s per km2, 6 threads |
| GPU | +0.06 ms at 1440p (two texture reads per fragment) |
| soft edge | -0.35 to +0.9 degrees around the horizon |

## Decisions (opus-planner, 2026-10-24)
1. **Texture plumbing:** a texture array with one layer per resident chunk slot (chunks are one size); a tile carries the slot index. The material map (M8-6) uses the same slot index in a parallel array. No atlas.
2. **Bake cost:** the horizon map is a second job that lands after the chunk's heights. Criterion 3 is about heights. Until the horizon lands, the coarse horizon from the global run shades the tile and the chunk horizon fades in (the stand-in rule, CON-22). Texel size and azimuth count are parameters (default 4 m, 16); the change reports the bake time per chunk.
3. **Debug view:** a lit `ShadeView` (horizon, shadow), no new key.
4. **Edits:** shadows lag an edit by design. An edit re-queues the horizon job of every chunk whose halo it touches. Precise edit handling belongs to M8-7 (to be noted in its change).

## Measured (indicative, CPU only, not a gated perf run)
| Quantity | Value |
|---|---|
| bake, 16 azimuths, 5 km, 4 m texels, release build, one thread (2026-10-24, the GEN-044 bake) | 21 us per texel |
| chunk of 4.9 km edge (1225 x 1225 texels, 24 km2) | about 32 core-seconds, about 5 s on 6 threads |
| SPIKE-32 for comparison | 0.7 s per km2 on 6 threads (17 s for 24 km2) |

The measured figure is lower than the spike's, so the bake as a second job lands in seconds after the heights; a per-chunk figure on the real chunk raster is reported when step 4 runs the job.

## Questions (1, 2, 3 and 7 are decided above; kept for the reasoning)
1. **Texture plumbing.** The terrain bind group is buffers only today. A texture array of resident chunk horizons (layer per chunk, fixed texel count per chunk) against one atlas. The material map (M8-6) needs the same. Decide with the render owner before step 5; whichever change lands first creates it.
2. **Bake cost against the chunk landing target.** About 17 s for a 24 km2 chunk at 4 m texels and 16 azimuths exceeds the 10 s landing target (criterion 3) when added to the A-net job. Options: horizon texels at 8 m (4 times cheaper, softer shadows), horizon as a second job that lands after the heights (the chunk draws without shadows meanwhile, like the stand-in), 8 azimuths, or a coarser far part of the march. Measure on the real chunk raster in step 1 (CPU timing only, indicative, not a gated perf run).
3. **Azimuth count.** 16 gives fans near occluders; 32 doubles storage and bake. SPIKE-32 suggests 32 or a two-component fit. Decide from the stills of step 6.
4. **Debug view.** CON-18 needs one. Candidate: a new lit shade view (`horizon`: the horizon elevation at the sun's azimuth as grey, and `shadow`: the mask), next to temperature and precipitation (`ShadeView`), so no new key. If a key is wanted, one viewer key in the K pattern.
5. **Determinism and the CPU twin.** SPIKE-32 called the map presentation data without a CPU twin. CON-07 asks for a canonical CPU implementation of any field a GPU implementation matches. The bake is CPU; the shader only reads and interpolates. The interpolation gets a CPU twin (a function with the shader's arithmetic) and a tolerance test; the bake itself is canonical CPU code, bit-exact across threads (CON-14).
6. **Face edges and poles.** The march crosses chunk borders and face edges; the halo is taken through directions on the sphere like the coupled run's neighbours (GEN-040), not through face coordinates. Untested territory as for chunks (the chunk-field proposal's open risk).
7. **Edits.** An edit changes heights, so its chunk and every chunk within the horizon range could change their maps. The proposal follows the chunk invalidation rule (reach plus halo) and treats the range beyond the halo as stale until the next bake (shadows lag an edit; documented, not fixed here). Decision with R3 and M8-7.

## Risks
- The sunset look needs shadows from terrain 20 km away; the coarse horizon at a lattice step of 2.4 km (2048 cells) can show a blocky outline near the combination. Check on stills.
- The texture array limits (layers per array, 2048 on common hardware) cap resident chunks; eviction is already by need.
- Seams: two adjacent chunks must agree at their border to the texel; the shared halo makes the values identical by construction (tier A).
