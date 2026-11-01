## ADDED Requirements

### Requirement: GEN-086 The horizon march has an AVX2 kernel with the bytes of the scalar one
`generators::horizon` SHALL march four azimuths of a texel at once on a CPU with AVX2, chosen at run time, and SHALL give for every raster, block, parameter set and thread count the bytes of the scalar march (GEN-085), which every other machine and every remainder azimuth runs.

Verify: A
Status: active
Source: opus-planner (2026-10-27, owner: no deferred optimisation), GEN-044, GEN-085, CON-14

Detail: The kernel SHALL do the scalar march's IEEE operations on each lane in the same order, with no fused multiply-add, and SHALL treat a sample outside the raster as the scalar march does (the lane stops). Both kernels SHALL be tested against each other on CI wherever AVX2 is present, with azimuth counts of 16, 7, 4 and 1, with texels whose rays leave the raster, and with a raster of NaN heights.

#### Scenario: Both kernels
- **WHEN** a rough raster is baked on the scalar kernel and on the AVX2 kernel
- **THEN** every byte is equal, for texels near the raster edge as well
