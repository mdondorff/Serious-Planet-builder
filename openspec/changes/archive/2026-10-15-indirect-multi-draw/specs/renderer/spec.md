## ADDED Requirements

### Requirement: REND-015 Indirect multi-draw
The terrain renderer SHALL be able to submit a plan's draws as indirect multi-draws built from the plan on the CPU: one `DrawIndexedIndirect` record per planned node in the plan's order (`first_instance` = the node's index, `base_vertex` = the first vertex of the node's slot in the buffer the draw names, `instance_count` 1, the index count of the tile mesh), grouped into runs of consecutive nodes that sit in one pooled vertex buffer, one `multi_draw_indexed_indirect` per run, with the per-node records in a storage buffer the shader indexes with the instance index. The per-draw loop SHALL remain, selectable (`--draw per-draw|indirect`), as the fallback where the adapter lacks `INDIRECT_FIRST_INSTANCE` or indirect execution, and as the reference: both paths SHALL draw identical images for the same plan. The records SHALL come from the plan and the slots alone (no GPU-side culling or decisions, CON-03); per-frame globals, node records and argument records SHALL live in a ring so a frame in flight is never overwritten (STRM-016); shaders SHALL read only camera-relative and tile-local values (REND-005). The bytes written per frame for node and argument records SHALL be reported (TEST-011).

Verify: A, C
Status: active
Source: CON-03, CON-11, ADR 0033, M4 item 7 (review item C)

#### Scenario: Records equal the plan
- **WHEN** the draw list of a standard view's plan is built from the slots of its tiles
- **THEN** there is one record per node in the plan's order, `first_instance` equals the node index, `base_vertex` equals the slot's index in its buffer times the vertices per slot, the runs cover the records, and the bytes per frame (192 B node record plus 20 B argument record per node) stay within nodes x 212 B for each of the ten views (a declared addition to the pinned counts)

#### Scenario: Identical images
- **WHEN** the ten scripted views are rendered in the level and relief views, with the tiles placed by the free list and scattered over blocks and the vertex pool, once per-draw and once indirect, on the software adapter and on the discrete GPU
- **THEN** the images are identical and the renderer reports the same draws, triangles and node-record bytes, one argument record per node, and one multi-draw per run (more than one where the nodes sit in several buffers)

#### Scenario: Unsupported adapter
- **WHEN** indirect mode is requested on an adapter without the feature
- **THEN** the renderer keeps the per-draw loop and reports the mode in effect

#### Scenario: Frames in flight
- **WHEN** more frames than the ring is deep are submitted without waiting, alternating two plans
- **THEN** the last frame shows its own plan, and `--stream` and `--pipelined` runs on either path settle on the same image
