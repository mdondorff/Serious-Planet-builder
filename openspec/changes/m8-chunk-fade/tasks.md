## 1. Carry the change out (STRM-047), about half a day
- [x] 1.1 Tests first (tier A): `ChunkField::level_change` equals `height_at_level - base height` at every direction and level; every wrapper over a chunk field returns it; a field without chunk detail returns `None` (spec: STRM-047)
- [x] 1.2 `HeightField::level_change`, `ChunkField`, the wrappers
- [x] 1.3 The compact builder stores the change plane (tile flag, plane of f32); numerics-reviewer

## 2. The node weight (STRM-047), about half a day
- [x] 2.1 Tests first: the draw plan's node carries `chunk_weight`; a tile near a chunk border takes the minimum of the weights of the chunks it reads (spec: STRM-047). The byte record (`NODE_RECORD_BYTES`, `node_bytes`) grows by 4 with the shader in step 3, where its layout is read
- [x] 2.2 `ChunkFade::tile_weight` (minimum rule, default fade 1,500 ms) into `TerrainDraw::chunk_weight` through `ManagedLoop::set_chunk_fade`; the window turns it on with the chunk view

## 3. The shader and its mirrors (REND-039), about a day
- [x] 3.1 Tests first: with `w = 1` or no change plane the CPU mirror equals the old arithmetic bit for bit; at `w = 0` the drawn height is the base's; the node record lane (spec: REND-039)
- [x] 3.2 The WGSL (the plane in the tile-reference buffer, the fade lane), the CPU mirror (`shade_vertex_faded`), the renderer's plane upload; the reference renderer draws a mesh of the blended field
- [x] 3.3 The faded tile on the software adapter at weights 0, 0.5 and 1 equals the reference renderer's image of the blended field (tier C, `gpu_chunk_fade`)

## 4. Length, normals, golden-review
- [x] 4.1 `chunk_fade_ms` default 1500 (criterion 3, amended 2026-10-24); the screen step at the entry level at the review poses at most about 0.5 px p99 with blending (computed from the field and the camera geometry; `gpu_chunk_fade` covers the draw agreement, not the pixel step); the look test of the normal choice (decision 4), recorded (numbers: design.md 6a, 2026-10-25; the look test of the normal choice moves to look review 2, the first version keeps the final normal)
- [x] 4.2 `golden-review`: the change moves no golden (no PR of it touched tests/goldens)
- [ ] 4.3 numerics-reviewer, spec-guardian, verifier; PR

## 5. Close
- [ ] 5.1 All requirements pass: statuses `planned` to `active`, archive, one line in `docs/status.md` via the planner

## Notes from step 1's numerics review (for steps 2 and 3)
- The stamp stage recomputes the change (a plateau or flatten absorbs part of it); every other forwarder is additive.
- A faded vertex `h_q - (1 - w) change` can lie below the tile's stated minimum (`min_height_m`, ATMO-020) and outside the culling and skirt bounds when `change > 0`: step 3 widens them by the largest change of the tile or takes the plane into the exact minimum.
- `TileSink::upload_bytes` defaults to `resident_bytes()`, which leaves the plane out: step 3 counts `4 (cells + 1)^2` bytes when a plane is uploaded.
- The texel normals include the change and the fade does not touch them (design decision 4).
- `compact_tile_field` evaluates the change twice per lattice point (in `heights_and_climate` and in `level_change`); measure before step 3 and fold into one pass if it shows.
