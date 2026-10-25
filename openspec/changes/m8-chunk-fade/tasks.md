## 1. Carry the change out (STRM-047), about half a day
- [x] 1.1 Tests first (tier A): `ChunkField::level_change` equals `height_at_level - base height` at every direction and level; every wrapper over a chunk field returns it; a field without chunk detail returns `None` (spec: STRM-047)
- [x] 1.2 `HeightField::level_change`, `ChunkField`, the wrappers
- [x] 1.3 The compact builder stores the change plane (tile flag, plane of f32); numerics-reviewer

## 2. The node weight (STRM-047), about half a day
- [ ] 2.1 Tests first: the node record carries `chunk_weight`; a tile near a chunk border takes the minimum of the weights of the chunks it reads; `node_bytes` grows by 4 (spec: STRM-047)
- [ ] 2.2 `ChunkFade::tile_weight` into the plan's node record; `FrameCounts` follows

## 3. The shader and its mirrors (REND-039), about a day
- [ ] 3.1 Tests first: with `w = 1` or no change plane the CPU mirror equals the old arithmetic bit for bit; at `w = 0` the drawn height is the base's; the shader scan finds the new binding and word offsets (spec: REND-039)
- [ ] 3.2 The second pool binding (dummy when chunks are off), the WGSL, the CPU mirror, the reference renderer
- [ ] 3.3 The landing on the software adapter: a tile's drawn height steps by at most the weight step times the change per frame (tier C, spec: STRM-039 fade scenario as REND-039)

## 4. Length, normals, golden-review
- [ ] 4.1 `chunk_fade_ms` default 1500 (criterion 3, amended 2026-10-24); the screen step at the entry level at the review poses at most about 0.5 px p99 with blending (measured on the landing stills); the look test of the normal choice (decision 4), recorded
- [ ] 4.2 `golden-review` of the candidates; stop and report only if one escalates to the owner's /bless
- [ ] 4.3 numerics-reviewer, spec-guardian, verifier; PR

## 5. Close
- [ ] 5.1 All requirements pass: statuses `planned` to `active`, archive, one line in `docs/status.md` via the planner
