## MODIFIED Requirements

### Requirement: REND-016 Compact tile rendering
The terrain renderer SHALL hold every drawn tile, levels 3 and finer (the render floor, LOD-011), in the compact format (GEN-019), the only tile format, in pooled slots of storage-buffer blocks and draw them with a vertex stage that places each vertex from the texels of its tile and of the neighbours the morph needs (the exact morph target). A block of the main pool SHALL be one storage buffer of at most 128 MiB (`block_slots x slot_bytes`: 14,146 slots of 9,488 bytes at cells 32, allocated on first use or by the warm-up of REND-017), its slots following one another and the texels row-major in the slot (the slot of GEN-025: a 128-byte header, the 324-byte climate block and the wide texels, a 32-bit height step count and two 15-bit octahedral normal components; `MESH_VERSION` 10 since ADR 0073 (8 before), so every terrain golden is regenerated through `golden-review`); the vertex stage SHALL read the slot's header and the texel `j x (cells + 1) + i` at the offsets of GEN-025 from the block bound for the run and decode the height as `hr_min + f32(q) * hr_step` and the normal by `extractBits`, and the fragment stage the slot's climate block (GEN-021); there is no separate record buffer; a draw SHALL name its block by being in a run of that block (consecutive nodes in one block form one run, in the plan's order) and its slot through a per-frame 16-byte tile reference (the slot's first word in its block, reserved, the slot index, reserved) indexed by the instance index (`first_instance` = the node index). The per-draw loop SHALL remain the reference: both submission paths SHALL draw identical images for the same plan whatever the slots, and the image SHALL equal the reference renderer's (REND-031: the same plan drawn from the CPU meshes) within the model tolerance of the compact tile (position model error, 15-bit normals): in the level view at most 0.05 % of the pixels differ by more than 12 levels, in the relief and normals views at most 0.2 %, and on the steep stress field at most 1 %. With every vertex morphed to the parent grid the images SHALL still agree, so a wrong morph target in the shader fails. A tile below the render floor (a compact tile of level 2 or coarser) or a slot outside the pool SHALL be refused, not written. There is no fallback form: a tile whose relief the wide texel cannot hold is a generator concern (GEN-019), never drawn in another format. The product paths (`test-render` single views, `--stream`, `--pipelined`, `--flight`, `--perf`, the repro replay) SHALL build and draw the compact format only (there is no `--tile-format`); a flight SHALL equal the reference renderer's frames of the same plans within 0.1 % of the pixels over 12 levels. Shaders SHALL read only camera-relative and tile-local values (REND-005 scans the compact shader for every tile size). The allocation SHALL equal the pure model of the pool configuration: compact blocks (buffer bytes), index buffer, and node records, and the mid and high shares SHALL hold 15 and 23 blocks (212,190 and 325,358 slots at cells 32, STRM-006). The known-answer pixel hashes of the buffer path (the ten scripted views in every debug view, persistent renderers per slot and batched, the lit frame with the atmosphere, the lit water frames) SHALL be re-recorded for the wide texel under `MESH_VERSION` 10 on WARP and on the discrete GPU, each change through `golden-review` as rounding noise (ADR 0017), and the pixels SHALL equal them.

The node record's level lane SHALL carry, 256 times, the edge masks of the draw (ADR 0074: `stitch` in the low four bits, `fine` above them; 0 for a node with none, so its record and its pixels are what they were). The vertex stage SHALL morph a vertex on a stitched edge fully to the parent grid and one on a fine edge not at all, whatever its distance, the skirt vertices following the edge vertex they hang from (the vertex's grid coordinates `i` and `j`: east `i = cells`, west `i = 0`, north `j = cells`, south `j = 0`); the stale-tile overlay reads the state from the lane modulo 256. The test-only reference renderer, which morphs by the vertices of a CPU mesh, SHALL refuse a masked node (`ExecError::MaskedNode`).

Verify: A, C
Status: active
Source: ADR 0033 points 3-5, ADR 0054, SPIKE-06, CON-03, CON-11, GEN-019, GEN-021, REND-015, GEN-025, ADR 0062

#### Scenario: Images against the vertex path
- **WHEN** the ten scripted views are rendered in the level, relief and normals views from compact tiles and by the reference renderer from the meshes, on the software adapter (cells 16), and three views at cells 32 including the 1 m ground camera
- **THEN** the pixels that differ by more than 12 levels stay within the stated fractions, and compact tiles were drawn

#### Scenario: Default path and flight
- **WHEN** the scripted flight is rendered by the product path and, from the same plans, by the reference renderer
- **THEN** every frame differs in at most 0.1 % of its pixels by more than 12 levels; the golden views drawn from compact tiles agree with their goldens within rounding on the adapters that have them

#### Scenario: Morph target
- **WHEN** every node's morph interval is forced so each vertex is drawn at its parent-grid position and height, on the steep field, in the relief and morph-height views
- **THEN** the compact and reference images agree within the steep tolerance (the morph-height view shows variation, and a shader that morphs the height to the wrong target fails)

#### Scenario: Both submission paths, any placement
- **WHEN** a plan is drawn per-draw and indirect with the tiles placed by the free list and scattered over blocks
- **THEN** all four images are identical, and the scattered indirect path needs more than one multi-draw

#### Scenario: Fallback and coarse tiles beside compact tiles
- **WHEN** the steep stress field is planned with the finest level declared at 9 so finest tiles exceed 65.535 m of relief
- **THEN** every drawn tile is compact with the wide texel, none of level 0-2 is drawn, and the image equals the reference renderer's within the steep tolerance

#### Scenario: Pools and the share
- **WHEN** the tier pools are configured for cells 32 (and a real block of 128 MiB is drawn from, with tiles in its last slots)
- **THEN** the mid share holds 15 blocks (212,190 slots) and the high share 23 (325,358), the model's allocation stays under the share with the fixed items inside their reserve, a renderer's allocation equals the model for its live blocks, and no tile size overflows the reserve with records (the record is in the slot)

#### Scenario: The buffer path draws the recorded pixels
- **WHEN** the ten scripted views are drawn in all nine debug views from compact tiles (one-shot), persistent renderers draw four views per slot and batched, and the lit frame with the atmosphere and the lit water frames are drawn
- **THEN** the hash of the pixels equals the hash recorded for the wide texel under `MESH_VERSION` 10 for the adapter's driver (WARP, the discrete GPU), and a driver without a recorded hash prints its hash

#### Scenario: One slot, one write
- **WHEN** a tile is written to a block and the block is read back
- **THEN** the slot's bytes at its start equal the CPU tile's slot (GEN-025) from one `write_buffer` range, no record buffer exists, and the shader's header and climate reads equal the CPU mirror's
