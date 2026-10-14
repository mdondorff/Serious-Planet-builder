# Performance Optimization options

Status: Unreviewed
Date: 2026-10-14
Source: Copilot

## Are there potential optimizations for runtime behavior and performance?

Yes — there are well-known, practical algorithms and techniques that match this repo’s flow and can materially reduce CPU and GPU cost (or hide it behind latency). Below I list the most relevant approaches, what complexity they improve (informally), how they map onto the code you already have, and concrete, prioritized suggestions for implementation with trade-offs.

Top candidate approaches (what they do and why they help)
1) GPU-driven culling + multi-draw-indirect (MDI)
   - Idea: move per-node culling / draw submission off the CPU and onto the GPU. A compute shader culls nodes (frustum, horizon, LOD, HZB occlusion) and writes a compact indirect draw buffer. One GPU submit executes many draws via MultiDrawIndirect.
   - Complexity effect: reduces CPU O(T) submission/loop costs to O(1) (one submit) + GPU compute cost proportional to visited nodes; CPU cost becomes constant per-frame. GPU work still proportional to drawable triangles but CPU overhead is much smaller.
   - Map to code: replace the for-loop in TerrainRenderer::render that iterates plan.nodes and calls pass.draw_indexed for each node (render.rs lines ~537–543) with an indirect draw path. Use a compute shader to read the cpu-generated plan (or upload a coarse node list) and emit draws.
   - Trade-offs: increased shader/driver complexity; must ensure node data fits GPU buffers and Plan is encoded for GPU; harder to keep deterministic cross-adapter timing. Good win for large T.

2) GPU occlusion with Hierarchical Z Buffer (HZB) or coarse occlusion and GPU compute
   - Idea: build a hierarchical Z (mipmapped depth) or use rasterized coarse depth from previous frames, then run occlusion tests (on GPU) for node bounding volumes or coarse proxy geometry; only visible nodes go to draw list.
   - Complexity effect: reduces number of submitted/fragmented draws and fragment work; can reduce both CPU draw count and GPU fragment cost.
   - Map to code: after you have node bounding spheres from frame plan, upload them and run a compute shader against HZB to filter nodes before draws (or use HZB occlusion within the compute culling stage described above).
   - Trade-offs: additional memory and pre-pass cost; works well if many nodes are occluded (e.g., valley/hilly scenes).

3) Geometry Clipmaps / Geo-mipmapping (regular grid streaming)
   - Idea: use a ring/clipmap structure that stores geometry for concentric LOD bands centered on the camera. Only a fixed-band set of mesh regions is kept resident and re-used as the camera moves; mesh resolution varies by band rather than unique tiles per quadtree cell.
   - Complexity effect: GPU vertex and generator cost becomes bounded by clipmap band sizes, not by the total visible tile count; effective per-frame complexity can become O(1) in terms of tiles, O(band_size * resolution) overall.
   - Map to code: instead of generating a separate mesh per TileId with independent V, generate a small set of mesh blocks per clipmap ring and reuse them across faces. Requires rethinking tile addressing and blending at seams.
   - Trade-offs: better for continuous height-fields and local, camera-centered rendering (flight focus). Harder to support arbitrary cube-sphere quadtree semantics and perfect adjacency across faces without careful stitching.

4) Precomputed multires height-textures + GPU tessellation / compute mesh generation
   - Idea: generate (or cache) multires height textures (mip levels) and let GPU compute/tessellate meshes on the fly (compute shader or tessellation) using a much smaller CPU footprint. Alternatively, store coarse-to-fine height mipmaps so you avoid full CPU-sided mesh generation for many tiles.
   - Complexity effect: moves O(cells^2) generation work into GPU (parallel), or replaces it with O(mip_samples) lookups; reduces CPU generation cost and upload volume.
   - Map to code: have generators produce compact height texture tiles (with mipmaps) and upload them; render stage fetches height maps and expands to vertex positions in a compute stage or in-vertex shader (with procedural sampling).
   - Trade-offs: requires GPU compute/path for mesh generation or use of shader-based displacement; increases GPU memory use; recomputing on GPU each frame can cost GPU time but can be massively parallel and hide latency.

5) Coarse-to-fine / adaptive sampling and mesh simplification
   - Idea: instead of uniformly V ∝ cells^2 per tile, produce simplified meshes where geometric detail is not needed (mesh decimation, planar patch fitting, or variable-resolution meshes). Use screen-space error metrics to reduce vertex count.
   - Complexity effect: reduces triangles and vertex upload / generation costs from O(cells^2) to O(needed_vertices) which can be considerably smaller.
   - Map to code: adapt planet_generators to emit lower-vertex-count LOD meshes or implement runtime simplification from high-res heights to fewer vertices where curvature is low.
   - Trade-offs: more sophisticated generator; must guarantee stitching correctness (morph intervals help).

6) Persistent streaming + predictive prefetch (async workers)
   - Idea: generate tiles asynchronously on worker threads ahead of time using camera motion prediction, and keep a persistent on-disk content-addressed cache (already partly present). Prioritize generation of tiles likely to be needed next.
   - Complexity effect: hides generation cost latency, smoothing spikes; no asymptotic change, but practical latency and frame-time variance improves.
   - Map to code: extend crates/streaming to have a thread pool or async executor that accepts prefetch requests derived from camera velocity; keep cache and upload pipeline decoupled from frame.
   - Trade-offs: correctness unaffected; more moving parts.

7) Instancing & mesh sharing
   - Idea: if many tiles share identical mesh keys (same seed/params), draw them as instanced draws rather than per-tile unique vertex buffers; reduce GPU memory and draw calls.
   - Complexity effect: reduces draw count and memory duplication; submission becomes O(#unique_meshes) rather than O(T).
   - Map to code: build an instance buffer of per-tile transform / node uniforms and use a single vertex buffer + instanced draw. Terrain tiling variation may limit applicability but nearby tiles can sometimes share geometry (especially in test scenes or repeated biomes).
   - Trade-offs: requires that meshes are identical and transforms are simple; not always applicable for procedurally unique tiles.

8) Compressed vertex formats and streaming compression
   - Idea: compress vertex/index data, use half precision or quantized positions, compress over-the-wire or on-disk cache.
   - Complexity effect: reduces memory bandwidth and upload cost (effective lowering of constants in O(V) uploads).
   - Map to code: change vertex serialization in render::vertex_bytes and upload to use packed formats or Draco-like compression for meshes.
   - Trade-offs: decompression cost (CPU or GPU) and precision loss needing validation.

How these map to complexity improvements (summary)
- Move culling/submission to GPU (MDI + compute cull + HZB): reduces CPU work from O(T) to near O(1) per frame; GPU still has work proportional to visible geometry but can cull earlier.
- Clipmaps / geometry clipmapping: bounds the number of vertices/triangles processed by mapping visibility to a fixed band => reduces asymptotic growth with T for many situations (camera-centered).
- GPU mesh generation / height mipmaps: move O(cells^2) generator cost from CPU to GPU parallel compute; can drastically reduce CPU spikes and improve throughput.
- Instancing & mesh sharing: reduces draw-call and memory duplication costs when meshes repeat.
- Prefetching + async generation: no asymptotic win but greatly improves frame-time stability by smoothing generation workload.

Concrete, prioritized suggestions for your codebase
1) Low-friction, high-value:
   - Implement a GPU-side culling pass that takes the CPU FramePlan bounding spheres and runs a compute shader to filter nodes; produce an indirect draw buffer and use MultiDrawIndirect. (Mapping: add a module to encode plan nodes into a GPU buffer; implement small compute shader to run frustum + morph culling + HZB occlusion; change render path to indirect draws.)
   - Add a background prefetcher in crates/streaming that uses camera velocity to kick off generator tasks for nearby tiles (async worker pool); keep current synchronous fallback.
   - Reuse SelectionCache across frames by default and expose a cached select path (your code already supports select_nodes_in_view_cached — ensure render path uses it).

2) Medium-effort:
   - Add HZB occlusion (build HZB from previous-frame depth or coarse depth pre-pass) and use it from the GPU culling pass.
   - Make mesh uploads asynchronous and rate-limit uploads per frame; do not block the render thread waiting for all uploads — instead, skip drawing missing tiles that aren’t uploaded and draw lower LOD alternatives.

3) Longer-term / higher complexity:
   - Evaluate a clipmap or geo-mipmapping renderer for the flight use-case: this eliminates large T spikes when flying over continuous terrain.
   - Move procedural generation partially or fully to GPU compute (e.g., a compute shader that consumes seeds and writes vertex buffers or height textures).
   - Implement mesh simplification / variable-resolution mesh generation to lower triangle counts where possible.

Practical notes and trade-offs
- Determinism & tests: your repo values deterministic outputs and golden tests. GPU-driven compute, different precision (f32 vs f64), or compression can break bit-identical outputs. If determinism is required, adopt deterministic paths for CI/golden tests (e.g., CPU generator + reference path), and allow fast GPU paths for interactive runs only.
- Work scheduling: prefer to smooth spikes (async generation + bounded upload per frame) rather than trying to make everything instantaneous — this preserves interactivity.
- Complexity vs benefit: GPU-driven solutions have higher implementation cost and testing burden but give the best reductions in CPU draw submission overhead and enable handling huge T.
