## Why
After GEN-085 the march is 1.5 s of the 2.3 s steady-state bake of one chunk (global16384, 6 threads, idle). The owner's line: no deferred optimisation; the result stays byte-identical, the scalar fallback is tested on CI.

## What Changes
- `horizon::Kernel` (`Scalar`, `Avx2`); `bake_horizon` picks `Kernel::detect()` at run time, `bake_horizon_with` takes one. The AVX2 kernel marches four azimuths of a texel at once with the IEEE operations of the scalar march in the same order (no fused multiply-add, floor by `roundpd`, the gather for the four bilinear corners, a lane mask for rays that leave the raster, `max` with the candidate first so a NaN is ignored as `f64::max` does).
- Sizing offline (single thread, 100 x 100 block of the real raster): 0.53 s scalar, 0.27 s AVX2, byte-identical.
- Machines without AVX2, other architectures, and azimuth counts that are not a multiple of four (the remainder) run the scalar march.

## Impact
- Spec: GEN-086 (new). Code: `generators/src/horizon.rs`. Maps are unchanged (GEN-044, GEN-085), no golden moves.
