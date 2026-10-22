Opened: 2026-10-22
Question: What quality can this engine reach on ONE 5 km alpine region (relief at 50 m to 2 km, real shadows, snow and rock mixed at metre scale, grass floors, photo detail without banding), what does each step cost, and what must change in the architecture to do it planet-wide?
Why it matters: M8 "Natural landscape" planning input (opus-planner, the owner's bar: an Outerra alpine valley at sunset).
Time box: about 5 h of agent work, indicative on the RTX 3080 Ti Laptop (other sessions running), no perf window.
Success criteria: stills at sunset and midday from 300 m, 30 m and 1.8 m, main as it is and with each of landforms, shadows, material map and photo sets switched on in turn; a cost table per step; the architecture changes each step needs.
Fallback if it fails: a written list of what blocks each step.
Code location: branch `spike/realism` (never merged); `realism-prototype.patch` in this folder is its diff against main at e374a938.

# SPIKE-32 results: the realism spike (realism)

Machine: Rasierklinge, RTX 3080 Ti Laptop (Vulkan), release build, 2560 x 1440, the lit path with the atmosphere, other sessions running (GPU times are indicative: median of 60 frames per still, GPU timestamps of the terrain pass).

## Verdict
- **The engine's lit path can carry all four steps in one pass with small changes; the result reads as a mountain landscape at 300 m and 30 m, but it does not reach the Outerra bar.** What is missing is in the content, not in the renderer: the landform generator of the prototype is a stop-gap, the material rules are first guesses and the photo sets are ungraded.
- **What works** (`images/sheet_*`): real peaks and ridges at 0.5 to 2 km (step 1); long ridge shadows at 3.5 degrees sun elevation for +0.06 ms (step 2); five materials mixed at metre scale by height-based blending of photo sets, scree under cliffs, grass floors (steps 3 and 4); the photo layer fades to the sets' mean colour by footprint (2 to 6 m per pixel), so no octave bands appear.
- **What does not reach the bar:** pinnacle-shaped peaks (thermal erosion makes cones); no glacial valley shapes; snow is scarce and blotchy ("leopard" patches from the noise terms of the rules); the photo grass reads yellow-olive (sets need colour grading); radial streaks at grazing angles in the de-tiled grass near the horizon; shadows come only from terrain inside the 5 km region and fan out at 22.5 degree azimuth steps; the 1.8 m ground is a lawn without clumps (instancing, SPIKE-28).
- **Cost** at 1440p on the RTX, terrain pass, median over 12 stills: landforms alone 0.92 ms (main's own shading), +0.06 ms shadows, +0.85 ms material map, +2.27 ms photo sets (total 4.1 ms, range 3.0 to 4.8 ms; main as it is: 1.2 to 1.5 ms on the base world). The photo layer is the expensive step.
- **The decisive architecture finding:** every step needs data that spans kilometres (erosion halo, horizon range 5 km, material inputs at fixed physical scales), so they are **block-level stages, not per-tile stages**: the same conclusion as SPIKE-31 for hydrology. Material weights computed per tile at the tile's own vertex spacing would change with the LOD; they have to come from a fixed-scale product (here 4 m heights, 2 m texels) that tiles sample.

## Method
- **Site:** the highest land flat over 3 km with a base height 500 to 1,400 m above the sea (lat -23.882, lon -15.284 of the seed-1, 512-cell in-memory bake, base 1,400 m, rim change 4 m). A 5,120 m square, 4 m height grid (1280 x 1280), a 700 m skirt that blends to the base height; `RegionStage` is a `HeightField` over the world's field (bicubic over the grid plus three octaves of sub-grid ridged detail, 5 m to 1.2 m, scaled by slope). Tiles, LOD, normals and the lit path are the engine's own.
- **Shading:** the terrain fragment stage (`fs_lit_atmo`) with a realism block in group 0 (bindings 4 to 10) and a per-frame uniform; flag bit 1 shadows, 2 material map, 4 photo sets. Region coordinates in the shader are the camera's region-frame position (f64 on the CPU) plus the fragment's camera-relative position, so no planet-radius value is formed (CON-11; the inverse radius scales a gnomonic correction of under 1 m).
- **Stills:** `REALISM=1 planet test-render --scene capture-flights --allow-memory-bake --global-res 512 --adapter hardware`, poses in the region frame (valley floor at (-1500, -1700) m and an upper slope at (300, -900) m, 300 m, 30 m and 1.8 m above the ground), sun at 3.5 degrees elevation (azimuth 262, sunset) and 58 degrees (noon). Steps: 0 main as it is (the same site without the region), 1 landforms (main's shading), 2 plus shadows, 3 plus material map (flat tints with procedural variation), 4 plus photo sets. Raw timings: `raw-region-timings.txt`, `raw-main-timings.txt`.

## (1) Landforms (`images/landforms_variants.jpg`, hillshades of the 5 km grid)
| Variant | Result | Cost (1280 x 1280 grid, one thread) |
|---|---|---|
| A. ridged multifractal only (1,500 m, 10 octaves, warp 500 m) | ridge and valley structure at all scales but lumpy and uncarved | 0.4 s |
| B. A + stream-power incision (implicit, n = 1, m = 0.5, 100 steps) | dendritic networks, but D8 routing lines the valleys up with the grid axes: parallel 8 m corduroy on every planar slope | 25 to 136 s (machine load) |
| C. B + diffusion + area threshold 3,000 m2 | fewer rills, the axis alignment stays | 40 s for 30 steps |
| D. droplet erosion (2 M droplets, life 300) + thermal 38 degrees | smooth blobs: thermal erosion at 38 degrees removes the relief (2,015 to 1,078 m) | 65 s |
| E. ridged 2,400 m (dome 0) + thermal 40 steps at 50 degrees | real ridgelines; peaks collapse to cones | 1.0 s |
| F (used). E + droplets 600 k (life 200) + thermal 12 steps at 56 degrees | ridgelines with 10 to 50 m gullies on the faces; pinnacles remain | 6.6 s |
What makes it read as mountains, in order of effect: (a) the **amplitude and sharpness before erosion**: 2,400 m of ridged multifractal over a 2.5 km radius (mean slopes near 35 degrees) reads as mountains, 1,100 to 1,500 m reads as hills; (b) a **talus angle of 50 to 56 degrees** for rock (38 degrees erases the relief, and the scree material then needs another source: here the proximity of cliffs, since thermal deposits at 56 degrees are negligible); (c) a **second-scale erosion pass** that is not grid-aligned (droplets; stream power needs multiple-flow routing or jittered receivers to be usable at 4 m); (d) sub-grid ridged octaves to 1 m. Not reached: glacial U valleys, cirques, scree fans as geometry.

## (2) Shadows: a terrain horizon map
- Technique: per 4 m texel, the horizon elevation along 16 azimuths up to 5 km (steps growing 4.5 % + 2 m), stored as four RGBA8 array layers (26 MB per 26 km2); the shader interpolates the two nearest azimuths and compares with the sun's elevation (soft edge: -0.35 to +0.9 degrees). Independent of the sun: the map is baked once, any time of day uses it.
- Look (`sheet_*` column 2 against 3): the long shadows of ridges across the valley at sunset and the dark cirque floors; with it the scene gains depth that no albedo change gives. Limits: 22.5 degree azimuth steps give fan-shaped soft edges near the occluder, a constant penumbra, no shadow from terrain outside the region, none from clouds or trees.
- Cost: GPU +0.06 ms (two texture reads per fragment); bake 18.2 s on 6 threads for 26 km2 (0.7 s per km2, about 4 core-seconds per km2); 1 MB per km2 at 4 m texels and 16 azimuths. The alternative (a shadow map rendered from the sun: one more depth pass of the same tiles, 2 to 3 k texels per side per cascade, bias and aliasing at grazing sun) was not built.

## (3) Material map
- Per 2 m texel (2560 x 2560, two RGBA8 layers, 8 B): weights of rock, scree, snow, grass, soil; wetness, convexity, snow depth. Inputs: slope and curvature from the 4 m grid, aspect, flow area (from the stream-power run when present; here the proxy is the droplet/diffusion height field, so wetness is weak), altitude relative to the massif, three noise octaves (90 m, 14 m, 3.5 m), cliff proximity. Rules: snow above a noisy line, sticking on slopes under 34 to 62 degrees, deeper in hollows (convexity) and on the north side; rock on steep faces and convex ridges; scree in the 24 to 47 degree band near cliffs; grass below the grass line on gentle wet ground; soil in between.
- **Height-based blending** (the part that makes snow settle in hollows and rock show through at metre scale): per material weight plus 0.7 times the photo set's displacement (the snow's height raised by the snow depth), the blend keeps the materials within a depth of 0.45 of the highest. Rock shows through the snow at convex bumps, grass tufts through scree.
- Cost: bake 1.4 s on 6 threads (0.2 us per texel); 52 MB per 26 km2 (70 MB with mips) at 2 m texels; GPU +0.85 ms with flat tints and procedural variation (five weights read, up to five materials evaluated).
- Weakness: the noise terms of the rules at 3.5 and 14 m produce blotchy leopard patterns where two materials compete; snow is scarce because the steep faces take the rock weight first. Tuning work, not an architecture question.

## (4) Photo textures
| Layer | Set | URL | Licence | Download SHA-256 (1K-PNG zip) |
|---|---|---|---|---|
| rock | Rock030 | https://ambientcg.com/view?id=Rock030 | CC0 1.0 | d3e0dc55fc46b093631f4d0009c934003c601e69df9aa4ba41a43db3807056ee |
| scree | Gravel026 | https://ambientcg.com/view?id=Gravel026 | CC0 1.0 | 78849426f9e5b4da0c93ea21c464c8af5996d2be2fe562df945f35f0241dd264 |
| snow | Snow006 | https://ambientcg.com/view?id=Snow006 | CC0 1.0 | f993019a7e2a59bfdf3ddeb9b4e692bc2a734a5fb71f17581234f24065dbdac5 |
| grass | Grass004 | https://ambientcg.com/view?id=Grass004 | CC0 1.0 | 9bdb8a28536f437e910824096dd8a3d8276d5186a27ad067dea4e2e762a6981b |
| soil | Ground037 | https://ambientcg.com/view?id=Ground037 | CC0 1.0 | cbd75f0660870b3299a68c4fe7fd54efb3951cf992d4295961209619eb284c47 |
Downloaded from `https://ambientcg.com/get?file=<id>_1K-PNG.zip` (about 17 to 20 MB each, 93 MB in all) into a cache outside the repository (this session's scratchpad; nothing is committed). Colour, NormalGL, Roughness and Displacement are used; ambient occlusion is not.
- **Use:** a texture array per kind (albedo sRGB with the displacement in alpha; normal xy, roughness), 1,024 pixels, eleven mips (box filter in linear light on the CPU), 5 layers, 56 MB uncompressed (about 14 MB as BC7 and BC5, estimated). Triplanar projection in the region frame (weights |n|^5), each plane sampled with a two-tap de-tiling (a random offset per cell of a low-frequency noise, blended where the cells meet); the rocky sets again at 8 times the scale so faces carry structure over 3 to 25 m; a macro colour variation over 9 m and 47 m; beyond 2 to 6 m per pixel the set fades to its mean colour (the last mip), which is what removes the bands of an octave fade.
- **Banding:** the main's ground detail fades octave by octave while a pixel grows past 0.06 to 0.25 of a cell; the photo layer has no octaves to fade and the mip chain gives a continuous fade. At 30 m and 1.8 m (`sheet_*_noon`, column 5) no band appears. Not solved: radial streaks at grazing angles in the grass, and the yellow cast.
- Cost: GPU +2.27 ms (up to 18 samples per material per fragment: 3 planes, 2 taps, two maps, two scales for rocky sets; up to three materials per fragment). Lower at 1.8 m on the slope (3.0 ms total) where fewer far fragments exist.

## (5) Cost table
GPU: terrain pass at 1440p, median of 12 stills (6 poses x 2 suns; one outlier at the first pose excluded from step 3), increments over the step before. CPU: bake time for the 26 km2 region (6 threads where marked). Memory: per 26 km2 region (per km2 in brackets).
| Step | GPU ms (total) | GPU increment | CPU | Memory |
|---|---|---|---|---|
| 0 main as it is (base world, same poses) | 1.2 to 1.5 | | tile 0.58 ms (mesh build and upload, 1,543 meshes in 0.9 s) | |
| 1 landforms (RegionStage, main shading) | 0.92 | | bake 6.6 s (0.25 s per km2); tile 0.63 ms (1,116 meshes in 0.7 s): bicubic and three octaves cost under 0.1 ms per tile | 6.5 MB heights (0.25 MB per km2) |
| 2 + horizon shadows | 0.98 | +0.06 | bake 18.2 s on 6 threads (0.7 s per km2) | 26 MB (1.0 MB per km2) |
| 3 + material map | 1.83 | +0.85 | bake 1.4 s on 6 threads (0.05 s per km2); about 1 ms per 65 x 65 texel tile | 52 MB, 70 MB with mips (2.7 MB per km2); 34 KB per 65 x 65 texel tile, 133 KB per 129 x 129 |
| 4 + photo sets | 4.10 (3.0 to 4.8) | +2.27 | load and mip 5 sets: not measured | 56 MB uncompressed, about 14 MB BC7 (shared by all tiles) |
Total over step 1: +3.2 ms of the 16.6 ms budget for the terrain pass at 1440p on the RTX, before the cost of the planet-wide stages below.

## What reaches the bar
| Item of the bar | Reached? |
|---|---|
| Alpine valley at sunset from a few hundred metres | Partly: valley floor, massif, long shadows, orange light, atmosphere haze are there (`sheet_valley-300m_sunset`); the peaks are cartoonish pinnacles |
| Ridges and peaks of 0.5 to 3 km casting real shadows | Yes inside the region (horizon map); no from outside it |
| Snow in hollows and on gentle slopes, rock on ridges and steep faces, mixed at metre scale | Mechanism yes (height blending, convexity, slope, aspect); snow coverage and the blotchy patterns are tuning |
| Grassy valley floors and haze | Grass yes (yellow-olive until graded); haze is the engine's atmosphere |
| Base layer natural without instanced objects | No: the ground at 1.8 m is a lawn; clumps, stones and shrubs need instances |

## Architecture changes each step needs
1. **Landforms:** a regional height stage between the global bake and the tile, at 2 to 10 m cells with a halo (SPIKE-31's scheme). CON-08: the stage reads the global bake and is computed per block, not per tile; droplet erosion depends on the order of droplets, so for partition invariance the block grid is part of the field's definition (seeded per block, a fixed halo) and the CPU reference needs `libm` and fixed reduction order (CON-14). Cost scale: the prototype's recipe is 0.25 s per km2, one thread: planet-wide is impossible, so it runs for the player's neighbourhood and the stored tiles of authored regions, with the coarse field elsewhere. `height_bounds` must widen by the region's relief (done in `RegionStage`). Tile slot size: unchanged for heights.
2. **Shadows:** a horizon map per block, 1 MB per km2 at 4 m texels and 16 azimuths; terrain beyond the block needs a second, coarse horizon from the global bake (shadows from peaks 20 km away at sunset) and a combination of the two. 32 azimuths or a two-component fit would remove the fans. It is presentation data (no CPU twin, CON-14) unless gameplay reads it. The shader needs the block's texture: the terrain bind group grows textures (a texture array of resident blocks, or one atlas), so group 0 is no longer buffers only.
3. **Material map:** per-tile weights at the tile's own vertex spacing change with the LOD (a 18 m tile cannot see a 4 m curvature), so the weights are a fixed-scale product (4 m heights, 2 m texels) sampled by the tiles; storage 8 B per texel, 34 KB for 65 x 65 texels, or a texture per block next to the horizon map. The inputs come from driver maps (ADR 0076: altitude, temperature, wetness from the flow accumulation, soil depth); SPIKE-31's two-scale hydrology gives the wetness inside the same halo. If instancing reads the same weights as its suitability (SPIKE-28, SPIKE-30) the weights become a field with a CPU reference and an ADR (CON-07, CON-14).
4. **Photo sets:** an asset cache ADR (location outside the repository, fetch with pinned URL and checksum, licence table, the 1 MB rule for assets; the procedural look of step 3 is the offline and CI fallback so goldens stay deterministic); 5 to 8 layers at 1K as BC7 and BC5, 14 to 22 MB; a per-set colour grade and roughness authoring step. The GPU cost (2.3 ms) is the budget question: evaluate only the two dominant materials per fragment (a sort of five weights), drop the second scale for distant fragments, bake the de-tiling into a larger tiled texture, or move the material evaluation to a deferred pass so overdrawn fragments cost nothing. Nothing in the tile slot changes.
5. **New passes:** none needed for the prototype (everything is in the terrain pass); a sun shadow map would add a depth pass of the visible tiles. **CON-18:** each step needs a debug view (the prototype has weights, shadow and normals views in the uniform, not exercised here) and a tier-C still per material; the photo layer's goldens need the assets, so tests use the procedural fallback.

## Limits
- One site, one seed, one 5 km region, a 512-cell in-memory world (the owner's 16384 store was not used: sonnet-m4's copy is 11 GB and a read of it could write to its tile pack; the "before" is the 512-cell world at the same site, a desert-coloured plateau, so before and after differ in climate as well as in landform).
- The terrain recipe was tuned by eye in a few iterations; the material rules and the photo layer are first versions; no colour grading.
- Timings: GPU timestamps, median of 60 frames, other sessions running, one outlier (4.1 ms at the first pose of step 3, probably the GPU clock). The mesh timings include upload and are one run each. CPU costs per tile are not isolated from the upload. The 1K sets were not compressed; the BC7 figure is an estimate.
- Spike code only: platform maths, statics for the resources, group 0 widened for every pipeline (the water pass shares the layout).

## Visuals
Sheets: each is the five steps of one pose and sun (top row: main as it is, landforms, shadows; bottom row: material map, photo sets), 2,400 x 900 JPEG.
![valley floor, 300 m, sunset](images/sheet_valley-300m_sunset.jpg)
![valley floor, 300 m, noon](images/sheet_valley-300m_noon.jpg)
![valley floor, 30 m, sunset](images/sheet_valley-30m_sunset.jpg)
![valley floor, 30 m, noon](images/sheet_valley-30m_noon.jpg)
![valley floor, 1.8 m, sunset](images/sheet_valley-1.8m_sunset.jpg)
![valley floor, 1.8 m, noon](images/sheet_valley-1.8m_noon.jpg)
![upper slope, 300 m, sunset](images/sheet_slope-300m_sunset.jpg)
![upper slope, 300 m, noon](images/sheet_slope-300m_noon.jpg)
![upper slope, 30 m, sunset](images/sheet_slope-30m_sunset.jpg)
![upper slope, 30 m, noon](images/sheet_slope-30m_noon.jpg)
![upper slope, 1.8 m, sunset](images/sheet_slope-1.8m_sunset.jpg)
![upper slope, 1.8 m, noon](images/sheet_slope-1.8m_noon.jpg)
Landform variants (hillshades of the 5 km grid):
![landform variants A to F](images/landforms_variants.jpg)
Single stills at 1,920 x 1,080 (RTX, Vulkan): main as it is and all four steps, valley 300 m at noon and sunset:
![main, valley 300 m, noon](images/valley-300m_noon_main.jpg)
![all steps, valley 300 m, noon](images/valley-300m_noon_s7.jpg)
![main, valley 300 m, sunset](images/valley-300m_sunset_main.jpg)
![all steps, valley 300 m, sunset](images/valley-300m_sunset_s7.jpg)
![main, valley 1.8 m, noon](images/valley-1.8m_noon_main.jpg)
![all steps, valley 1.8 m, noon](images/valley-1.8m_noon_s7.jpg)
![main, slope 30 m, noon](images/slope-30m_noon_main.jpg)
![all steps, slope 30 m, noon](images/slope-30m_noon_s7.jpg)
![main, slope 1.8 m, sunset](images/slope-1.8m_sunset_main.jpg)
![all steps, slope 1.8 m, sunset](images/slope-1.8m_sunset_s7.jpg)
