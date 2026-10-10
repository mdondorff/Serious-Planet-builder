# Spike register

Time-boxed experiments that answer a question before a decision. Opened and closed with `/spike`. Spike code lives in `spikes/<name>/` and is never merged into the main crates as-is.

Planned spikes from the background report (open them when their milestone starts):
- GPU-driven indirect terrain/instance path in wgpu. Milestone: M2. (SPIKE-01, closed 2026-10-10.)
- CDLOD morph quality on steep terrain. Milestone: M2. (SPIKE-02, closed 2026-10-10 for smooth terrain; steep terrain remains open for M3.)
- Global hydrology on the sphere (stream-power on a 1–4 km grid + halo-based local carving). Milestone: M3. Fallback: local erosion filter + rivers on a coarse flow grid.
- City HLOD with 10^5 buildings. Milestone: M7.

## SPIKE-01 GPU-driven indirect path in wgpu
Status: closed
Opened: 2026-10-10
Closed: 2026-10-10
Question: Does wgpu 30 expose what a GPU-driven instance path needs (indirect indexed draws, first-instance, multi-draw count, binding arrays, immediates, timestamps)?
Why it matters: ADR 0013 (render graph and GPU-driven baseline), report §8, §14 ("wgpu feature gaps").
Time box: 30 minutes.
Success criteria: the features are advertised by the adapters we test on (WARP, lavapipe via CI, RTX 3080 Ti Laptop).
Fallback if it fails: raw Vulkan/D3D12 backend behind the render-HAL seam.
Code location: none (a throw-away query of `Adapter::features()`; not kept).
Result: `INDIRECT_FIRST_INSTANCE`, `MULTI_DRAW_INDIRECT_COUNT`, `TEXTURE_BINDING_ARRAY`, `STORAGE_RESOURCE_BINDING_ARRAY`, `IMMEDIATES`, `TIMESTAMP_QUERY`, `DEPTH32FLOAT_STENCIL8` and `FLOAT32_FILTERABLE` are advertised by `Dx12 Microsoft Basic Render Driver` (WARP) and by `Vulkan NVIDIA GeForce RTX 3080 Ti Laptop GPU`. Default device limits: 128 MiB storage buffer binding, 256 MiB buffer, 8192 texture dimension, 4 bind groups. Linux lavapipe was not queried (CI has no feature log); the Intel Iris Xe iGPU on this machine lacks the binding-array features (reported by the verifier). The query code was not kept. Features are available but must be requested when the device is created (the context currently requests none); no GPU culling pass was built or timed.
Recommendation: no fallback needed for the instance path; request `INDIRECT_FIRST_INSTANCE` and `MULTI_DRAW_INDIRECT_COUNT` when the first GPU culling pass is added (REND-008), behind its CPU reference (`frame::instances`). Timings need Rasierklinge.

## SPIKE-02 CDLOD morph quality
Status: closed
Opened: 2026-10-10
Closed: 2026-10-10
Question: Does per-vertex geomorphing with parent-grid targets and skirts give crack-free level transitions on the cube-sphere quadtree (on the smooth test generator; steep terrain not covered)?
Why it matters: ADR 0014 (terrain LOD), LOD-004, LOD-005, REND-009.
Time box: part of the m2-terrain-render change.
Success criteria: zero hole pixels in ten scripted views; border vertices fully morphed next to coarser leaves and unmorphed next to finer ones; fully morphed borders coincide with the coarser neighbour's border across faces.
Fallback if it fails: stitching meshes or a larger skirt.
Code location: `crates/frame/src/lod.rs`, `crates/generators/src/mesh.rs`, `crates/render/src/terrain.rs` (not throw-away: implemented through OpenSpec changes).
Result: all three criteria hold in tests (`morph.rs`, `mesh.rs`, `gpu_terrain.rs`) on WARP. Two defects were found and fixed on the way: morph distance measured to height-including positions (now a height-free reference position) and a morph start earlier than the split distance plus bounding radius (now `LodParams::morph_interval`, validated). Visual quality of the morph on the real GPU and with realistic terrain is unreviewed (six goldens await the owner).
Recommendation: adopt CDLOD with vertex morph and skirts (ADR 0014); revisit meshlets after the M2 gate if node counts (772 draws at a 1 m view with the small test parameters, 3,003 with the production parameters) cost too much. Steep terrain was not tested: the test generator is smooth.
