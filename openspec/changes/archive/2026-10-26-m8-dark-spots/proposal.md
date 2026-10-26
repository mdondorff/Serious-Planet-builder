## Why
The owner (2026-10-25, RTX, rebuilt store; `target/review/frame-4841.bundle`) saw blue-black spots over a bright highland at 3,904 km near the day/night line. The diagnosis (crusader, 2026-10-25): they are the coarse horizon shadows of the hill band at low sun. The coarse map has 512 observer cells a face (19.6 km a cell), 16 azimuths, a byte of 0.353 degrees; of the land cell-azimuths 4.5 % have a horizon above a sun at 0.9 degrees, 2.1 % at 1.25, 0.5 % at 2 and none from 3 degrees (the neighbour step p50 0, p90 0.35, p99 1.41 degrees). So a few per cent of the cells are in shadow near the terminator, each a 20 km spot, a sample of the terrain's shadow and not its shape.

## What Changes
- **The coarse horizon fades with the distance of the camera** (REND-043): its elevation is multiplied by `coarse_fade(d)`, 1 up to 1,000 km from the surface point, a smoothstep to 0 at 3,000 km (`planet_generators::horizon::coarse_fade`, the shader's `horizon_coarse_fade`). The chunk horizon is not faded. At the owner's 3,904 km the coarse shadows are gone.
- **Mipmaps of the coarse array are not built.** The first plan had them for the minification of a 512 cells map; with the fade at 3,000 km a pixel is under 2.4 km there against a cell of 19.6 km, and a pixel reaches the cell (minification) only beyond about 24,000 km, where the strength is 0 anyway.

## Not Done (deferred to look review 2)
A sub-cell coverage plane (the share of a cell in shadow, C): the numbers above are its input. The fade is a presentation choice in the shader, not a look-file value.

## Impact
- Spec: REND-043 (tier A for the function, C for the shader against it). Goldens: the lit orbit views with a coarse map, if any, move (see the PR).
- Code: `generators/horizon.rs`, `render/horizon_wgsl.rs`, `render/terrain.rs` and `atmosphere.rs` (the two lit fragment stages), `render/horizon.rs` (the probe takes the distance).
