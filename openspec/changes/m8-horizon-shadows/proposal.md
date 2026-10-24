# Change: m8-horizon-shadows (proposal, drafted 2026-10-24; not started)

## Why
- M8-5 (`docs/briefings/m8-plan.md`; exit criterion 5: shadows and the material map together at most 1.5 ms of terrain-pass GPU time at 1440p). SPIKE-32 step 2 measured a terrain horizon map at +0.06 ms GPU for long ridge shadows at 3.5 degrees sun elevation (`spikes/realism/results/results.md`, section 2), and concluded the map is a block-level product with a kilometre range, not a per-tile one.
- Terrain beyond the block needs a second, coarse horizon from the global run (shadows from peaks 20 km away at sunset) and a way to combine the two (SPIKE-32 architecture finding 2).
- Chunks are the block of M8 (ADR 0077): the chunk stage already supplies the 4 to 10 m heights and a halo in metres; this change adds a second product per chunk beside the heights.

## What changes (in order; each step one PR of at most a day)
1. **Horizon bake (CPU, GPU-free)**: per chunk, per texel of the chunk's height raster, the horizon elevation along `A` azimuths out to the range `Rh`, from the chunk's raster plus its halo (`generators`). Pure, deterministic (CON-14), tier-A tests on synthetic terrain. Numbers (texel size, `A`, `Rh`, step law) in the spec; starting values from SPIKE-32: 4 m texels, 16 azimuths, 5 km, steps growing 4.5 % + 2 m.
2. **Coarse horizon from the global run (CPU)**: per direction on the global lattice (the coupled run's lattice, stored heights, GEN-040), the horizon elevation along `A` azimuths to the far range (about 50 km), stored beside the other global channels (`TileChannel`, STRM-043's pattern, a new kind); a pure function of the stored heights.
3. **Combination**: the horizon of a point is the larger of the chunk's and the coarse one at the azimuth; defined once as a CPU function with the shader's interpolation as its twin (CON-07: the CPU function is the reference; tolerance stated in the spec).
4. **Storage and residency**: the horizon map is a chunk output (`ChunkField`), stored and evicted with its chunk (STRM-038 to 041), cached on disk under the chunk key plus the horizon spec; a tile whose chunk is not ready uses the coarse horizon only (stand-in, CON-22).
5. **Render**: the terrain bind group gains the horizon textures (a texture array of resident chunks, or one atlas: design question 1) and the lit fragment stage a shadow term (`renderer`); the sun's elevation and azimuth already travel in the globals. Shadow off is the identity: no golden of an existing view changes.
6. **Debug view and tier-C**: a horizon view (the horizon elevation at the sun's azimuth, and the shadow mask) and stills at sunset and midday on the engine render of the fixed poses (CON-18); goldens through `golden-review` only.
7. **Perf**: shadows' share of the 1.5 ms budget measured in the architect's window (criterion 5), not in a test.

## Impact
- Specs: new GEN-044 to GEN-046 (horizon bake, coarse horizon, combination), STRM-045 (the global channel) and STRM-048 (the horizon job) and STRM-049 (the world wiring), REND-038 (the shadow term), VIEW-054 is not needed if the debug view is a lit shade view (design question 4). All status planned until each lands (deltas in this change).
- Code: `generators` (new module beside `coupled.rs`/`chunkfield`), `cache/global_store.rs` (a channel; my lane), `render` (terrain bind group and lit shader; my lane), `frame` (shade view; small). Not touched: `generators/height*.rs`, `frame/lod.rs`, `streaming`, `app/worldfield.rs` (explorer's), except one line each where the chunk output is registered, in a small PR agreed with the planner.
- Depends on M8-2 (chunk field, done through 3d) and for step 2 on the stored coupled run (done). The material map (M8-6) shares the texture plumbing of step 5: whichever lands first creates it.
- Cost: bake 0.7 s per km2 at 4 m texels and 16 azimuths in SPIKE-32; a chunk of about 24 km2 plus halo would add about 17 s to a chunk job, against the chunk's 10 s landing target (criterion 3): see design question 2.
- Out of scope: shadows from clouds or objects, a sun shadow map, shadows on instances (M9), soft penumbra from the sun's disc beyond the fixed soft edge, the photo layer.

## Tests
- Tier A: the bake on a synthetic ridge (known horizon angles, the lit side and the shaded side), determinism over threads (CON-14), the halo (a ridge outside the chunk but inside the halo shadows the chunk's edge texels; identical values at chunk borders), the coarse horizon on a small lattice, the combination (max), the interpolation twin of the shader.
- Tier B: the chunk output through the cache (round trip, key changes with the horizon spec), the stand-in (coarse only) and the landing refresh.
- Tier C: the shader against the CPU twin on a few texels (tolerance), the horizon debug view and sunset/midday stills (goldens via `golden-review`, first goldens of a new class go to the owner's /bless).
- Perf (window): GPU time of the shadow term at 1440p at the fixed poses; chunk job time with the horizon bake.

Expected golden changes: none for existing views (the term is off unless the lit view asks for shadows and a horizon is resident); new goldens for the horizon view and the stills.
