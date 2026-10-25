## ADDED Requirements

### Requirement: REND-038 The horizon shadow term
The lit terrain fragment stage SHALL multiply the direct sun term by a shadow factor from the horizon (GEN-046): the sun's elevation against the horizon of the fragment's point at the sun's azimuth, with a soft edge from -0.35 to +0.9 degrees around the horizon, taking the horizon textures of the fragment's chunk and the coarse horizon. The position used to sample the maps SHALL be tile-local or camera-relative f32 (CON-11). With no horizon resident, or with shadows off, the term SHALL be 1 and the output of every existing view SHALL be unchanged. The term is presentation shading (CON-14): it has no CPU twin beyond the GEN-046 reference, and its declared bound against that reference, measured at texel centres on the software adapter, is: chunk horizon 2e-5 rad, coarse horizon 3e-4 rad, lit factor 8e-3 with a chunk map and 2e-2 with a coarse map (the adapter's `asin` and texture weights on a 1.25 degree soft edge). The maps SHALL be read with bilinear filtering: between texel centres the factor blends neighbouring cells (the reference takes the cell), and across a cube-face edge the clamped sampler stops the blend, leaving a step of one cell's horizon difference along the edge in the far-field mask (a stated limit). Both maps SHALL hold exactly 16 azimuths. The shadow term and the material map together SHALL take at most 1.5 ms of terrain-pass GPU time at 1440p (M8 criterion 5; measured in the architect's window, never in a test).

Verify: C
Status: planned
Source: opus-planner (2026-10-24, M8-5), SPIKE-32 step 2, CON-11, CON-18

#### Scenario: Shader against the reference
- **WHEN** the shader's shadow factor is read back at sampled texels and sun directions
- **THEN** it equals the CPU function of GEN-046 within the stated tolerance, and with shadows off the frame equals the frame of a build without the term

#### Scenario: The view
- **WHEN** the horizon view is drawn at the fixed poses at sunset and midday
- **THEN** the horizon elevation and the shadow mask read as the ridges that cast them, and the stills match their goldens

### Requirement: REND-041 The frame's horizon data
`planet_frame::horizonview` SHALL decide, GPU-free, what the renderer takes for the horizon shadow: a chunk's horizon map (GEN-044: `n x n` cells of up to 16 azimuths) packed as the GPU reads it, four layers of `256 x 256` texels of four azimuth bytes (the azimuths `4 L .. 4 L + 3` in layer `L`, a missing one 0), each texel the mean of the map's cells under it and at least one; the rectangle of a tile inside the chunk it lies in (origin and size in the chunk's `[0, 1]^2`, `None` for a tile coarser than the chunk grid); the sun's azimuth and elevation at a chunk's centre in the chunk's raster frame (GEN-046, `+j` north, `+i` east); and the slots of the renderer's array: a fixed number, one per chunk in view that has a map, a re-bake uploaded again into the same slot, the least recently wanted slot given up when none is free, the nearest chunks served first, a released chunk's slot free at once. A map that does not fit is refused, never truncated. For the plan's nodes in order it SHALL build one entry each (`HorizonFrame`): the slot of the node's chunk (`-1` for a tile coarser than the chunk grid or a chunk with no slot, the other numbers then 0), the sun's azimuth and elevation at the chunk's centre, the chunk fade's weight (1 until REND-039) and the tile's rectangle, and the flat table of two `vec4` a node that the shader reads.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-5), decisions 1 and 2 (a texture array, one layer-group per resident chunk slot), CON-03

#### Scenario: Packing and slots
- **WHEN** maps of 8, 512 and 614 cells are packed, tiles at several levels are placed in their chunks, and slots are assigned over a flight with more chunks wanted than slots
- **THEN** a small map's texel is the cell under it, a larger one the mean of the cells (a constant map stays constant), a bad map is refused, a tile's rectangle is its place in the chunk, the sun's azimuth is a quarter turn toward `+i` and 0 toward `+j`, a re-bake uploads once into its slot, and the stalest slot goes to the nearest new chunk
