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

## SPIKE-04 Erosion filter variants
Status: open
Opened: 2026-10-12
Question: Which pure-function-of-the-direction erosion filter (gradient-damped fBm, slope-incised ridged creases, flow-aligned gullies) gives the most convincing terrain in the relief view over the global bake, at what cost per sample, with what height-bound argument, and does cells 32 / ratio 240 still plan?
Why it matters: M3's local erosion filter (the report's fallback to hydrology, SPIKE-03); the owner picks the look.
Time box: one agent session.
Success criteria: three variants rendered in the relief view at 4 cameras (coast, mountain, 1 m, 200 m) into `target/review/erosion-spike/` with a contact sheet; per variant: ns per sample, the bound argument, plan verdict at cells 32 / ratio 240.
Fallback if it fails: keep the plain fBm detail of `cpu-terrain-v1` (GEN-016).
Code location: spikes/erosion-variants/ (throwaway; branch `spike/erosion-variants`, never merged; it makes `layer_noise` and `layer_noise_with_gradient` public on that branch only).
Result (2026-10-12, one session, bake of 512 cells, seed 7, WARP, release build, one thread; images in `target/review/erosion-spike/`: `contact-sheet.png` = relief view, 4 columns 0-baseline / A-damped / B-incised / C-meander, 4 rows coast-1500m / mountain-2km / ground-1m / hills-400m; `contact-sheet-hillshade.png` = top-down hillshade patches (coast 40 km, mountain flank 8 km, mountain 600 m), same columns, which shows the character far better than the relief view, whose mild lighting makes all variants look flat at flight scale):
- 0 baseline (`cpu-terrain-v1`: fBm of 14 octaves from 64 per radius, persistence 0.45, at most 300 m): 735-760 ns per sample; smooth, no visible erosion character at any of the patches. Measured Lipschitz constant 7.1e5 m/rad (x3 safety).
- A damped fBm (octaves from 256 per radius, persistence 0.55, up to 900 m on land; each octave damped by 1/(1 + (|accumulated gradient, bake slope included| / 1.27e6 m per rad)^2)): about 1.2 us per sample (the bake slope costs 4 extra bake reads). Looks like fine granular rock, rough everywhere, no drainage pattern. Lipschitz 3.0e6 m/rad. Bound argument: weights in (0, 1], so |detail| <= a0 sum(amp)/norm; over a cap the per-octave noise bounds times the weight range, the weight range from the variation of the accumulated gradient (Hessian bound times reach plus the Lipschitz constant of the bake's smoothed gradient): tight on small caps, loose on large ones.
- B incised creases (ridged creases (1 - |n|)^3 per octave, amplitude (80 + 900 land) m scaled by 0.15 + 0.85 smoothstep(slope 0.01 to 0.2) of the bake slope): about 1.2-1.3 us. Branching drainage-like crease network, deeper on steep ground. Lipschitz 1.17e6 m/rad. Bound argument: interval arithmetic: `layer_noise_bounds` per octave, exact monotone map of |n| through (1 - |n|)^3 (the kink of |n| is not a problem for a range), the slope factor from a monotone function of the bake gradient bounded over the cap by cell differences, product of intervals: sound and as tight as the terrain field's.
- C meander (B with the lattice position displaced by 0.8 cells of smooth vector noise at 0.35 of the octave frequency): about 2.7-2.9 us. Meandering, river-like creases. Lipschitz 1.15e6 m/rad. Bound argument: B's, with the noise evaluated over the box the displaced positions can reach (the displacement is Lipschitz, about 2.5 times the box of B): looser at mid octaves, equal for tiny caps.
- A flow-aligned variant was tried first (lattice stretched along the downhill direction) and dropped: the stretch needs a local reference point, and a stretch about the point itself is the identity (the direction is tangent, so the position has no component along it); a real one needs a per-cell reference, which seams at cell borders.
- Planning at cells 32 and viewport / tau 240 (640x360, tau 1.5 px), four cameras, `max_height_m` 10,000: all four fields plan at all four cameras (2.5-9.5 thousand nodes, B and C within 1 % of the baseline, A within 3 %). CAVEAT: the spike's `height_bounds` are Lipschitz bounds about the centre value with the measured constant times 3, not proved, and the bake is 512 cells (not 16384): the ADR 0016 check must be repeated with the real bounds and the production bake.
Recommendation: not made; the owner picks the look from the sheets. Costs: B is about the cost of the baseline plus the bake slope; C doubles it; A and B need the bake's gradient (4 to 8 extra bake reads per sample), which a regional field with a halo would read once per tile instead.

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
