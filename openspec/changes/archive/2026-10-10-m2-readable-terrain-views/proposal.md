## Why

The owner reviewed the six terrain golden candidates: orbit tile-id and aerial level are readable, but the 1 m depth, 200 m height, cube-corner normals and 2 km morph images are smooth, nearly uniform ramps. A correct and a broken render would look alike, so blessing them would guard little. The cause is the data: the height field has features of 100 km and larger, so near-ground views see a plane.

## What Changes

- Depth view: log2 distance bands (dark line at every doubling), so perspective-correct depth reads as stripes.
- Height view: grey ramp with a dark contour line every 100 m of absolute height.
- Morph view: blue (fine) to red (fully morphed to the coarser grid) in R/B, so the red channel is still the morph value; blue is its complement.
- Normals golden: rendered from a camera 3,000 km above the cube corner looking down, where the normal colour changes across all three faces and any seam would show as a step.
- Height golden uses the aerial-200km camera (renamed `terrain_aerial200km_height`): at 200 m the height field is flat to within a few metres, so contours do not show; contours coarsen by decades so they never turn to moire. Depth golden stays on ground-1m.

## Capabilities

### Modified Capabilities
- `renderer`: REND-006 (views may carry contour lines and bands).

## Impact

`crates/render/src/terrain.rs` (shader), `crates/render/tests/gpu_terrain.rs` (colour rules, normals golden camera). Four candidate goldens change; their names stay in `tests/goldens/pending.txt`.
