# Compact tiles in a storage buffer instead of an atlas texture (exploration, 2026-10-17, sonnet-spikes)

Question from opus-planner (the owner's: why do the 8-byte texels sit in a texture and not in buffer memory read by index, "vertex pulling"?). Prototype on branch `explore/compact-storage-buffer` (never merged): a runtime switch `set_buffer_atlas(true)` makes compact renderers built afterwards keep each block as a 128 MiB storage buffer (slot after slot, row-major texels of 8 bytes) read by the vertex stage with `array<vec2<u32>>` and an index `slot_start + j * (cells + 1) + i`, instead of an `Rgba16Uint` atlas read with `textureLoad`. Everything else is unchanged: the slot accounting, the 128 B records, the tile references, the draw list, the morph and the fragment stage (the climate texel read changes to `base + idx`). Production code is untouched. GPU timings are indicative only (see the label below), no perf window.

## Short answer
1. No document records why a texture: ADR 0033 weighs the atlas against "large vertex buffers" (the vertex format's pool) and never against a storage buffer read by index; SPIKE-06 built the compact shader around `textureLoad` without comparing. It is not forced by a wgpu limit: the atlas block (4096 x 4096 x 8 B) is exactly 128 MiB, which is exactly `max_storage_buffer_binding_size`, so a buffer block needs the same 15 (mid share) or 23 (high share) bindings as the atlases do.
2. The prototype draws **exactly the same pixels** (byte equality on WARP and on the RTX), costs **the same GPU time** on the RTX (ratio buffer / atlas 1.001 to 1.011 over four views), and **writes cheaper**: per-slot writes 33 % cheaper per tile, batched writes equal but in 2 calls instead of 21.
3. It does **not** fix the WARP device removal: the smoke with tiles in buffers still removes the device (0 of 5 completed). This corrects my learning of 2026-10-17 ("atlases written by `write_texture`"): the trigger is not the texture path.
4. On WARP only, a `write_buffer` executes at about 13.7 MB/s whatever the buffer, against about 100 MB/s for `write_texture`: filling 2,378 tiles takes 1.6 s instead of 0.25 s. A cost for software tests, not for the product.
5. By itself the switch shrinks the duality only a little (render side, about 60-80 lines); its value is a free slot layout (no square atlas, no spare-channel trick, any stride), which is what the per-tile data of item 7 and the layout v2 of the retirement plan want. Recommendation below.

## Q1. Why the atlas
- ADR 0033 point 3 and the doc comment in `render/src/compact.rs` decide how a draw *names* its slot (a run per atlas, a bind group per atlas) and list atlas textures against "large vertex buffers (the vertex format; the default storage binding limit of 128 MiB holds about 2,400 slots of 54 KB)". Neither SPIKE-06 (`R32Uint` texel per vertex "in a pooled atlas") nor the brainstorm review compares a storage buffer. So the texture is SPIKE-06's inheritance, not a decision against buffers.
- Limits (wgpu 30 defaults, which ADR 0047 requires): `max_storage_buffer_binding_size` 128 MiB, `max_buffer_size` 256 MiB, `max_storage_buffers_per_shader_stage` 8 (the compact vertex stage uses 4: nodes, records, refs, one block). A compact slot at cells 32 is 33 x 33 x 8 = 8,712 B, so a 128 MiB binding holds 15,405 slots (the atlas holds 124 x 124 = 15,376: the square costs 0.2 %). Share arithmetic is identical: 1.875 GiB of blocks at the mid share = 15 blocks = 231,075 slots (atlas 230,640), 2.875 GiB at the high share = 23 blocks. One bind group per block, as now; the draw list already groups runs per block. A single binding is too small for the share, so blocks stay separate bindings with or without a texture; binding arrays are not needed.
- Neighbour reads for the exact morph are plain index arithmetic in both; the texture's 2D cache locality does not show: the GPU times are equal.

## Q2. Prototype
Correctness (`crates/render/tests/probe_buffer.rs`, with an assertion that the switch really changes the shader): one-shot frames of the ten scripted views in all 9 debug views (90 frames, 16,542 compact nodes drawn), persistent renderers with batched and per-slot uploads at four views, and the lit frame with the atmosphere at four views: **byte-identical on WARP and on the RTX (Vulkan)**.

WARP smoke (`planet editor --smoke --tile-format compact`, 160 x 90, software adapter), with the window's lit path and atmosphere: buffer variant **removed the device in 5 of 5 runs** (`readback map failed`), the atlas baseline 3 of 3. The removal needs the compact pipeline with the atmosphere, not texture uploads.

GPU time, RTX 3080 Ti Laptop (Vulkan), ground-1m view of the `Reference` field, 2,378 compact nodes at 2560 x 1440, relief view, GPU time of the render pass (timestamp queries), both renderers on one device, frames alternating A B / B A:

| view | atlas median ms | buffer median ms | ratio |
|---|---|---|---|
| ground-1m, 300 frames | 1.888 | 1.898 | 1.005 |
| ground-1m, alternating blocks of 60 frames | 1.930 | 1.952 | 1.011 |
| ground-1m, 400 frames, run 2 | 1.942 | 1.951 | 1.005 |
| ground-1m, 400 frames, run 3 | 1.951 | 1.963 | 1.006 |
| aerial-200km (630 nodes) | 2.167 | 2.170 | 1.001 |
| face-edge-1km (1,574 nodes) | 2.312 | 2.322 | 1.004 |

CPU time to record and submit is equal (1.09-1.11 ms at ground-1m). **Label: indicative, taken 2026-10-17 09:20:25 to 09:21:09, Rasierklinge, RTX on Vulkan, GPU clock state not controlled; no other GPU work of mine was running and the coder session's real-GPU run had finished and was holding; not a perf window; the owner may have had a remote desktop session.** The pass is only about 2 ms here (the `Reference` field, debug relief view, not the lit path), so a 1 % difference is 0.02 ms, inside the p10-p90 spread (about 0.15 ms): "equal" is the honest reading. The lit and atmosphere path was not timed.

Upload cost of 2,378 compact tiles (CPU time in the calls and wall time until the queue is idle, median of 5 fresh renderers; `warm` = the same slots written again; RTX):

| mode | atlas | buffer |
|---|---|---|
| per slot, contiguous slots | 10.05 us/tile (wall 23.9 ms), 4,756 write calls | 6.73 us/tile (16.0 ms) |
| batched, contiguous slots | 4.74 us/tile (11.3 ms), 21 write calls | 4.79 us/tile (11.4 ms), 2 write calls |
| batched, every 3rd slot | 9.68 us/tile (23.0 ms), 4,756 calls | 7.49 us/tile (17.8 ms) |

(Per-slot first upload into fresh blocks: 24.2 ms against 16.3 ms.) The batched atlas path needs the rows of a run interleaved on the CPU and breaks a run at the end of every atlas row; a buffer run is one `memcpy` and spans anything adjacent. The batched path is already the product's default, so the product gain is the simpler code and the cheaper scattered and per-slot cases: up to about 0.3-0.4 ms per frame at the 1 MiB upload cap (about 118 slots).

WARP (CPU renderer, `write-buffer-vs-texture-warp.txt`): one `write_buffer` into a 128 MiB, 32 MiB or 24 MiB buffer, with or without STORAGE/COPY_SRC, runs at a constant 13.7 MB/s from 256 KB to 21 MB (21 MB: 1.54 s); the same bytes through `write_texture`: 0.20 s (about 100 MB/s). It is WARP's buffer-to-buffer copy, not the buffer size or first-touch initialisation. A software test that uploads thousands of tiles gets slower; the tests upload tens.

## Q3. Could the vertex format share the pool?
Today both formats already use "blocks with a slot pool": `PoolConfig { slot_bytes, block_slots, main_blocks, vertex_slot_bytes, vertex_slots, ... }`, `SlotPool`, `pools.acquire`, `make_pool_buffer` (vertex blocks: `VERTEX | COPY_DST`, at most 64 MiB, 256 slots of 53.7 KB), `block_buffer` / `atlas`. With the compact blocks as plain buffers the allocator side becomes literally one thing: a block is a buffer of `block_slots * slot_bytes`, the stride is the format's. The vertex format (levels 0-2, fallbacks, the vertex-only reference mode) could move to storage buffers read by index as well (`array<f32>` or `vec4` reads of its 12 floats per vertex instead of vertex attributes), which would:
- merge `DrawBuffer::{VertexPool, Block, Atlas}` into one run kind (one bind group per block, no `set_vertex_buffer`), and drop the mid-pass pipeline switch and `vertex_bound` bookkeeping if the vertex stage decodes both layouts (a per-node format flag in the tile reference, a uniform branch per draw) in one pipeline: three pipelines (debug, lit, lit+atmosphere) instead of six;
- let the climate/water data be one per-slot block read the same way by both.
What it would **not** shrink: the tile builders (`tile_mesh_field` and `compact_tile_field`), the two pack payload forms, `PoolKind::{Main, Vertex}` and the level/relief rule, `Place::{Vertex, Atlas, Fallback}`, the parametrized tests and the CPU mirrors. I estimate the render-side saving at 150-200 of the roughly 900 vertex-only-plus-compact-only lines in `render` (the pipelines, the run kinds and the switching), nothing in the other crates. The risk is the one SPIKE-06 found: the vertex stage is the GPU cost, and one shader with both decoders raises register pressure; unmeasured here. As a step towards retiring the vertex format it is optional: the retirement deletes the second decoder instead of merging it.

## Recommendation (for the M5 gate, together with the duality decision)
1. **Move the compact blocks to storage buffers**, as a small change of its own (about 1-1.5 coder days: shader accessor, block creation, the two upload paths, the share arithmetic without squares, the one-shot path without the 8192-texel limit, tier-B differential against the atlas version kept until it merges) or as the first step of the layout v2 in the retirement plan (the texel layout changes there anyway). Evidence: identical images, equal GPU time (1.001-1.011, indicative), cheaper and simpler writes, no wgpu limit in the way, no golden change (the images are byte-identical, the pack payload is the same texel bytes), and a free slot layout: the 2-spare-bit squeeze of the 32-bit height plus two 15-bit normals in one 8 B texel (earlier study) is no longer forced, a texel can be 12 or 16 B, and per-tile data (the 324 B climate block, item 7's water mask) can follow the texels in the same slot without a second texture or a spare channel.
2. **Do not expect it to fix the WARP removal** and do not count on it for the duality: by itself it removes the atlas/block distinction and little else. The duality decision (retire the vertex format, about 5-7 days) stands on its own numbers.
3. Keep the WARP cost in mind: `write_buffer` on WARP is 7x slower than `write_texture` (13.7 against about 100 MB/s); fine for tests that upload tens of tiles, not for a software test that fills thousands.
4. If the retirement is approved, build the buffer layout inside it; if not, the buffer change is still worth doing before item 7 adds more per-tile data.

## Limits
- One view family (the `Reference` field, relief and debug views); the lit and atmosphere path was compared for pixels, not timed. The GPU time is 1.9-2.3 ms per frame, so differences under about 0.05 ms are noise.
- WARP smoke counts are small (0 of 5 and 0 of 3 completed).
- Line-count savings are estimates from function sizes; nothing was removed from the product to measure them.
