## Why

The rest of M2's scope: an adaptive camera, the render graph declaration and the instance-path stub (CPU reference for GPU culling), plus the two M2 spikes and the render-graph and LOD ADRs.

## What Changes

- `frame::controller` (adaptive camera), `frame::instances` (culling reference, indirect arguments, camera-relative instance positions), `render::graph` (declaration and compile checks).
- SPIKE-01 (wgpu indirect/binding features: available on WARP and the RTX 3080 Ti) and SPIKE-02 (CDLOD morph quality) recorded and closed; ADR 0013 (render graph, GPU-driven baseline) and ADR 0014 (terrain LOD) proposed.
- REND-010, 011, 012 added as active; REND-008 (GPU culling differential) stays planned.

## Capabilities

### Modified Capabilities
- `renderer`: new REND-010, REND-011, REND-012.

## Impact

`crates/frame`, `crates/render`, `docs/` only. No GPU pass is added; the terrain executor is unchanged.
