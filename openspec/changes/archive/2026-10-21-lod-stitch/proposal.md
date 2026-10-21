# Change: lod-stitch

## Why
- ADR 0074 (accepted), change 2 of 3. A leaf held back from splitting is nearer than its split distance, so a finer neighbour's vertices on the shared edge are not fully morphed to the coarse grid by the camera distance (the crack-free argument of LOD-004 and LOD-008 assumes a leaf with a finer neighbour is at least its split distance away). The border would open next to every capped tile; the skirts hide it only partly.

## What changes
- The plan marks the edges (`terrain::edge_masks`): a capped leaf's edge facing finer leaves is `fine`, the edges of those finer leaves facing it are `stitch`; from the plan's leaves alone, never the camera, so a reused plan stays crack-free. A leaf with no capped neighbour has no mask.
- The node record's level lane carries the masks (256 times: `stitch` in the low four bits, `fine` above); the compact vertex stage morphs a vertex on a stitched edge fully (m = 1) and one on a fine edge not at all (m = 0), skirts included. With both masks 0 the stage computes exactly what it computed (the goldens prove it).
- The test-only reference renderer refuses a masked node (`ExecError::MaskedNode`); the stale-tile overlay reads the state modulo 256.
- LOD-008 (the edge masks, a scenario), REND-016 (the lane, the vertex rule).
- Not in this change: the F3 colour of capped tiles, the log line and the un-ignored sweep test (change 3); cover and stand-in nodes carry no mask (the T-junctions beside a held leaf stay hidden by the skirts there, as stated in ADR 0074).

## Tests
- `lod_stitch.rs` (tier A): the masks on a spike plan are exactly the capped leaves' edges facing finer leaves and the facing edges of those leaves, none on ground with no capped tile; the CPU mesh oracle: the finer leaf's vertices on a stitched edge, morphed to the parent grid (the mesh's `coarse`), lie on the capped leaf's edge to a millimetre.
- `gpu_stitch.rs` (tier C, every adapter): a mask changes the picture along the border and nowhere else (more than 90 % of the pixels unchanged), the stitch alone moves the finer leaf.
- The existing goldens and the node-record byte tests show that a node without masks is unchanged.

Expected golden changes: none.
