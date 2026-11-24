## Why
- The viewer mesh splits every cell along `a-c` (`planet_generators::mesh::grid_indices`), and the lit paths shade with the vertex normal interpolated over each triangle. A smooth crease across the split diagonal renders as a sawtooth of cell size; along it, it stays straight (spikes #184g; G3-9's candidate-neutral item; the design note `lanes/digest/proposals/viewer-mesh-split-2026-11-24.md`, option c1; lead #200, 2026-11-24).
- Measured through the real compact renderer (TEST-153, WARP): a scarp at 135 degrees turned back by a quarter turn differs from the same scarp at 45 degrees by a mean of 1.94 and a p99 of 22.6 luminance steps (0-255), all of it along the scarp's edges in a per-cell pattern.

## What changes
- REND-068: the lit fragment stages of the compact renderer shade with the bilinear interpolation of the cell's 4 stored texel normals at the fragment's tile coordinates (`tile_normal`, normalised).
  - A previewed node keeps the vertex normal.
  - The debug views keep the vertex normal, and their pictures stay bit-identical.
  - The reference renderer, which has no texels in its fragment stage, keeps the vertex normal as its stand-in, like its other tile reads.
- No geometry, mesh, slot, key, pool or morph change: the triangulation and REND-009's morph targets are unchanged.
- TEST-153: the 45 against 135 degree scarp through the real renderer. With this change: mean 0.044, p99 0.93.

## Expected golden changes:
- (filled from the tier-C run: the lit views of the compact path only)

## Impact
- Spec deltas: REND-068 (renderer), TEST-153 (testing).
- Code: `crates/render/src/compact.rs` (`tile_normal`), `reference.rs` (the stand-in), `terrain.rs` and `atmosphere.rs` (the two lit fragment stages); the test `crates/render/tests/gpu_quad_split.rs`.
- The look of B and RG alike: the lit goldens go to the owner's `/bless` through golden-review. The evidence is `lanes/digest/proposals/rend-068-quad-split-before-after.png`. The PR waits for the lead's review.
