# SPIKE-28 results: instancing at walking height (instancing)

Opened: 2026-10-22
Question: What does a scatter of instances (rocks and grass clumps) cost at walking height at 2560x1440 on the RTX, with the visible set found by the CPU (cull and upload each frame) and by a GPU compute pass writing indirect draw arguments, and how do the two agree (the CPU reference, CON-03)?
Why it matters: M7 I1 (opus-planner, 2026-10-21): instancing is a performance risk the owner wants de-risked early (Q10); M8 builds it on the driver maps' suitability (ADR 0076), which gives the placement density per kind.
Time box: about 3 h of agent work (owner/planner, 2026-10-22).
Success criteria: a table of counts against cost (GPU ms of the cull pass and the draw pass, CPU ms and upload bytes of the CPU path, memory per instance and in all) for 10 k, 100 k and 1 M placed instances; the GPU visible set equal to the CPU reference's; the density input as a hook (a function of kind and place) shown to thin the set identically on both; a recommendation.
Fallback if it fails: report what ran, what did not, and the indicative numbers of the part that did.
Code location: spikes/instancing/ (the prototype is `crates/render/examples/instancing_spike.rs` on branch `spike/instancing`; a patch is kept in `results/`; throwaway, never merged as it is).

## Answer in one paragraph
**On the RTX at 2560x1440 a scatter of instances costs about 0.45 to 0.65 ms of GPU time per million visible triangles (0.62 to 0.66 up to 2.3 M triangles, 0.45 from 8 M), and a GPU compute cull over a static instance buffer is nearly free (0.05 ms for 1 M instances, 0.22 ms for 4 M).** Instancing is therefore a budget of triangles, not of instances: 2.3 M visible triangles (16 k grass clumps of 6 triangles and 28 k rocks of 80) cost 1.45 ms; 8.6 M cost 4.0 ms; 17 M cost 7.7 ms. The CPU path does not scale: culling a flat list takes 2.5 ms at 1 M placed and 10 ms at 4 M, culling by cells first 0.5 and 2 ms, and uploading the visible set costs 5 ms at 470 k visible instances (7.5 MB a frame). The GPU path found exactly the CPU reference's set in all seven scenes (0 differences in 4 M placed instances). Recommendation: **the GPU path, with the CPU's cell test deciding which cells are resident and in range, the placement density from the suitability on the CPU (a field, CON-14), and mesh LODs so that the triangle count, not the instance count, is held to the budget (about 1.5 M triangles for 1 ms).**

## Method
- Prototype: `crates/render/examples/instancing_spike.rs` on branch `spike/instancing` (patch: `instancing-prototype.patch`). Raw wgpu with its own device (like `upload_bench`), RTX 3080 Ti Laptop (Vulkan), release build, one process, other sessions running (indicative; no readiness gate, no perf window). GPU times from timestamp queries of waited frames (median of 100 after 10 warm-up frames); an earlier full run of the same scenes agreed within noise (see `run-sort.txt`; the numbers below are the last run, `run-final.txt`).
- Scene: a flat ground plane, the eye 1.8 m up, pitch 6 degrees down, 60 degrees vertical field of view, 2560x1440, an `Rgba8Unorm` target and `Depth32Float`, opaque flat-lit instances (no alpha test, no shadows, no textures). Two kinds: grass clumps (three crossed tapered quads, 6 triangles) and rocks (an icosphere of 80 faces, flat shaded, displaced), 80 % and 20 % of the placed instances, scattered uniformly in a disc (250 m, or 80 m for the dense rows). An instance is 16 bytes: a position (3 x f32, relative to a region origin where the camera stands, the tile-local form CON-11 asks for) and one word (kind, turn, size, a thinning byte).
- The cull of one instance (the same formula on the CPU and in WGSL): the bounding sphere against six frustum planes, and distance thinning: the instance's own byte against a keep probability that falls from 1 at half the kind's range to 0 at the range (grass 45 m, rocks 120 m; 60 and 120 m in the dense rows).
- Paths: (CPU) cull on the CPU every frame, upload the visible instances of both kinds with one `write_buffer`, two instanced draws; (CPU by cells) the same cull with the instances sorted into 16 m cells and the cells tested first (sphere against frustum, nearest distance against the range); (GPU) a compute pass of one thread per instance (workgroups of 64) writing the visible indices of each kind with an atomic counter that is also the instance count of an indirect draw, then two `draw_indirect` calls reading the instance through the index. The ground alone is drawn first and its pass time subtracted, so the draw columns are the instances' cost.
- Oracle (CON-03): after the timed frames the GPU's visible indices and counts are read back and compared, as sets, with the CPU reference (`cull_cpu`; the pure form of `planet_frame::instances::cull_instances`, REND-008).

## Results
GPU ms are medians; "draw" is the instances' share of the pass; "cull, all" is the flat scan over every placed instance.
| Placed (field) | Visible grass | Visible rocks | Triangles | CPU cull, all | CPU cull, by cells | CPU upload (bytes) | Draw, CPU path | GPU cull | Draw, GPU path | Same set |
|---|---|---|---|---|---|---|---|---|---|---|
| 10 k (250 m) | 39 | 59 | 4,954 | 0.019 | 0.004 | 0.004 ms (1,568) | 0.003 | 0.008 | 0.005 | yes |
| 100 k (250 m) | 422 | 707 | 59,092 | 0.197 | 0.025 | 0.009 (18,064) | 0.047 | 0.010 | 0.047 | yes |
| 1 M (250 m) | 4,027 | 7,116 | 593,442 | 2.455 | 0.469 | 0.056 (178,288) | 0.396 | 0.053 | 0.392 | yes |
| 4 M (250 m) | 15,983 | 27,879 | 2,326,218 | 10.598 | 1.982 | 0.385 (701,792) | 3.178 | 0.219 | 1.451 | yes |
| 1 M, patches (250 m) | 2,017 | 3,573 | 297,942 | 1.258 | 0.193 | 0.027 (89,440) | 0.145 | 0.035 | 0.138 | yes |
| 2 M (80 m, dense) | 138,713 | 96,618 | 8,561,718 | 14.317 | 6.967 | 2.307 (3,765,296) | 7.397 | 0.118 | 3.959 | yes |
| 4 M (80 m, dense) | 277,130 | 192,380 | 17,053,180 | 28.830 | 14.496 | 4.953 (7,512,160) | 15.920 | 0.223 | 7.725 | yes |

(The row "1 M, patches" places 500 k instances: see the density hook below.) Per frame the CPU path costs the cull plus the upload and the draw; the GPU path costs the cull and the draw, and on the CPU only the dispatch.

### What the numbers say
- **Cost follows triangles.** The GPU path's draw is 0.39 ms for 0.59 M triangles (0.66 ms per million), 1.45 ms for 2.3 M (0.62), 3.96 ms for 8.6 M (0.46) and 7.7 ms for 17 M (0.45): the cost per triangle falls as the count grows (a fixed part per draw and per instance), to about 0.45 ms per million. Rocks (80 triangles each, 28 k to 190 k visible) are most of the triangles; a grass clump is 6.
- **The GPU cull is cheap enough to scan everything.** 0.05 ms at 1 M and 0.22 ms at 4 M placed instances, whatever is visible. Past about 10 M instances in range it would want the cell test in front (below).
- **The CPU cull needs cells.** The flat scan is 2.5 ms per million placed instances and the by-cell cull 0.45 ms at 1 M; in the dense rows the visible instances (470 k) make the instance tests 7 to 14 ms, which is what the GPU does in 0.1 to 0.2 ms.
- **The upload does not scale past tens of thousands of visible instances**: 18 kB and 0.01 ms at 1 k, 3.8 MB and 2.3 ms at 235 k, 7.5 MB and 5.0 ms at 470 k, on the render thread (the staging copy comes on top on the queue).
- **An unexplained factor in the draw.** For the same visible set the draw of the CPU path (instances uploaded, read directly) costs about 2 times the GPU path's (read through the index) from 2 M triangles up: 3.2 against 1.45 ms at 2.3 M and 15.9 against 7.7 ms at 17 M. It is not the order of the instances (ad-hoc runs with the CPU path sorted near first, far first and shuffled, `run-sort.txt`, gave 9.4, 12.8 and 8.6 ms against 7.4 in cell order and 4.0 for the GPU path at 8.6 M triangles); the data and the draws are otherwise the same. Not chased (time box); the GPU path's numbers are the ones used here, the CPU path's draw column is an upper bound.

## The density hook (ADR 0076)
- The placement is a CPU field: `scatter(n, suitability)` keeps a candidate of kind `k` at `(x, z)` with the probability `suitability.at(k, x, z)`, from a hash of its index (deterministic, the same set on every machine). `Flat` (1 everywhere) and `Patches` (a toy field of soft patches) show the hook: the patches row places 500 k of the 1 M candidates, the visible counts halve, the two paths agree. In M8 the closure becomes the suitability of the kind from the driver vector (ADR 0076 points 3 and 4), evaluated by the CPU when a tile's instances are built; the GPU never evaluates a suitability, it culls and thins by distance.
- The distance thinning is separate from the density: each instance carries its own thinning byte, so the near-to-far fade is the same instances disappearing, deterministic, and the same on the CPU reference and in WGSL.

## Memory
- 16 bytes an instance (3 x f32 and one packed word); 32 bytes unpacked. 1 M placed instances are 16 MB, 4 M 64 MB.
- The GPU path adds an index of 4 bytes per visible instance per kind (allocated at the worst case here, 8 bytes per placed instance, 32 MB at 4 M; a real one is sized per tile) and 32 bytes of indirect arguments.
- The CPU path needs a visible buffer of 16 bytes per visible instance (7.5 MB at 470 k) and the same upload every frame.
- Not every placed instance is resident: a tile's instances are built by the CPU from the placement field when the tile is (as the heights are), so the resident count follows the tiles in range, not the planet.

## Recommendation
1. **Build the GPU path** (REND-008's CPU reference stays as the oracle, and the equality test of this spike becomes a tier-C test): per tile a static instance buffer built from the placement; the CPU tests the tiles or cells (frustum, range) and gives the GPU the list of those to scan; one compute pass writes the visible indices and the indirect arguments; the draws are indirect.
2. **Hold the triangles, not the instances, to the budget**: about 0.45 to 0.65 ms per million visible triangles; for about 1 ms, 1.5 M triangles in view. That means LODs: rocks of 80, 20 and 4 triangles by distance (or a billboard), grass clumps thinned by distance as here and a card at range; at 2.3 M triangles the 28 k visible rocks are the cost.
3. **Placement density = suitability on the CPU** at tile build, a field under CON-14; the distance thinning by the instance's own hash byte on both sides.
4. Do not use the CPU upload path beyond a few tens of thousands of visible instances.

## Limits
- Opaque flat-shaded instances: grass in practice is alpha-tested cards, which costs fragments (overdraw is high in the dense rows, see the pictures); no textures, shadows, wind or LOD meshes; the terrain pass (1.4 to 2.3 ms with the ground detail at 1440p) and the atmosphere come on top.
- One flat plane and the camera at the region origin: positions are small f32, so the f64 to f32 step of a real tile (CON-11) is not measured. Bounding spheres only; no occlusion culling.
- Timings are indicative (no gate, no clock control); the draw of the CPU path is unexplained (above). The GPU cull was checked for set equality in seven scenes; a boundary case (an instance exactly on a plane) differing by a rounding was not seen, in 4 M instances.
- Not measured: instances over a real terrain pass, the cost of building a tile's instances on the CPU, memory of a streamed ring, a hierarchical GPU cull.

## Visuals
- `images/1M-250m.png`: 1 M placed instances in a 250 m field (4,027 grass clumps and 7,116 rocks visible), the GPU path, RTX, 2560x1440, eye 1.8 m: the scatter reads as rocks and grass at walking height; the sky is the clear colour.
- `images/2M-80m-dense.png`: 2 M placed in an 80 m field (139 k clumps and 97 k rocks visible, 8.6 M triangles): the dense case, 4.0 ms for the instances.
