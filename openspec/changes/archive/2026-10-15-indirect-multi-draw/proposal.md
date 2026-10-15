## Why

M4 item 7, review item C (`docs/background/optimization-brainstorm-review.md`): the terrain renderer issues one `draw_indexed` per node with a vertex-buffer rebind and a dynamic-offset bind-group switch each (about 2,900 at ground-1m). The CPU cost of encoding them is the part of the frame the renderer owns. The plan already says everything that is drawn, so the argument records can be built on the CPU from it and submitted as one `multi_draw_indexed_indirect` per buffer run (CON-03 holds: the CPU decides, the GPU culls nothing).

## What Changes

- frame: `planet_frame::draw` (GPU-free): `DrawRecord` (a `DrawIndexedIndirect`), `build_draw_list` (one record per node in the plan's order, `first_instance` = node index, `base_vertex` = the slot's first vertex inside its buffer, runs of consecutive nodes in one buffer), and the per-frame byte counts.
- render: node records move from a dynamic-offset uniform (256 B stride) to a read-only storage buffer indexed by the instance index (192 B stride); the vertex shader reads `nodes[instance_index]` and passes the index to the fragment shader as a flat varying. Per-frame globals, node records and argument records live in a ring of two sets. `DrawMode::{PerDraw, Indirect}`: per-draw stays as the reference path and the fallback (now `draw_indexed(.., i..i+1)` so both paths run the same shader); indirect is the default where the adapter has `INDIRECT_FIRST_INSTANCE` and the downlevel `INDIRECT_EXECUTION` flag.
- app: `test-render --draw per-draw|indirect`.
- spec: REND-015.

## Declared addition (TEST-011)

Bytes written per frame on the indirect path, per standard view, are reported and bounded (nodes x 212 B: 192 B node record + 20 B argument record; ground-1m 2,897 nodes = 614,164 B). No existing pin changes; MESH_VERSION unchanged; no golden changes.

Allocation: the share's node-record bytes (16 MiB at `MAX_PLAN_NODES` x 512) now bound the ring: the most nodes a plan may hold is (16 MiB - 2 x 96 B) / (2 x 212 B) = 39,6xx, above `MAX_PLAN_NODES` (32,768); the old code allowed 65,536 by its 256 B stride (the largest standard view has 2,897).

Not in this change: the compact tile format (later change; the slots here are the vertex-format tiles), GPU culling, reordering nodes by buffer.
