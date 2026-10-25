## Context
Inputs: `openspec/changes/m8-chunk-field/design.md` sections 6 and 14 (the weight and its limit), the compact format (`crates/generators/src/compact.rs`: a 128-byte header, the climate block, a pad word at `SLOT_PAD_OFFSET`, 8-byte texels), the render pools and shader (`crates/render/src/compact.rs`, `terrain.rs`), the node record of the indirect draw (`frame::indirect_frame_bytes`), the reference renderer (REND-031).

## Decisions

### Built (step 3, 2026-10-25): where the plane lives on the GPU
The plane is not a second pool: the renderer keeps the planes of its resident tiles on the CPU (by slot) and, each frame, copies the plane of every node whose change is still fading in (`1 - w` above 0) into the frame's tile-reference buffer after the references (`FADE_PLANES` = 512 planes of `(cells + 1)^2` f32 padded to whole 16-byte entries); the node's reference holds the entry of its plane plus one, the node record the fade in the fourth lane of grid phase 6 (zero at weight 1, so the record does not grow and `NODE_RECORD_BYTES` stays). Why: a second pool binding would be a fifth storage buffer of the vertex stage (the adapter gate requires four, VIEW-001), a settled view reads and uploads nothing, and the bind groups of the pool blocks do not change when a chunk lands. The cost is that a fade moves up to 512 planes (2.2 MB at 32 cells) a frame for its 1.5 s; the overflow draws the final height.

**Known limit (seams, numerics review of step 3):** the plane is a tile's total change and the weight the minimum over the chunks it reads. Two tiles that share a border but read different sets of chunks (one reads C1 and C2, the other only C1, with C2 just landed) get different weights, so along their border the first draws the base where the second draws the final height: a crack of C1's change along the border, covered by the skirt, for 1.5 s. It needs a chunk to land while its neighbour's change is already in; the look review of the landing decides whether it matters (the remedy is a plane per chunk read, or the minimum over the chunks of the border cells only).

### 1. Where the change lives
A second plane of `(cells + 1)^2` f32 values per tile (metres, relative to the tile like the texel heights), in its own pool buffer indexed by the same slot index as the main pool. It exists only when the app has chunks on: without chunks the pool is a 4-byte dummy binding and the shader's branch is off, so the slot bytes, the pack payloads, the hashes of GEN-025 and the goldens of a world without chunks do not move (the pipeline layout gains one binding, which the golden run shows as pixel-identical). A tile without chunk detail writes zeros there (or is flagged off, see 3).

### 2. The weight
`ChunkFade::tile_weight(view, tile, now_ms)` (STRM-039) fills an f32 `chunk_weight` per node of the frame's plan record (one word more per node; the node bytes of the indirect path grow by 4, `node_bytes` in `FrameCounts` follows). The shader reads it with the node. The weight of a tile is its own chunk's; across a chunk border the two tiles' weights differ for up to `chunk_fade_ms`, so a vertex on the border can step by the weight difference times the change: the border vertices read the minimum of the weights of the chunks the tile reads (the cell reads already cross borders), which makes both sides rise together.

### 3. The shader
Position = base position from the texel height `h_q` minus `change` (the base height), plus `w x change`: `h = h_q - (1 - w) x change`. For `w = 1` or no change plane it is exactly the old arithmetic (bit for bit: `change x 0` adds 0.0). A flag bit in the slot header's flags word says "this tile has a change plane", so tiles without one skip the read. The CPU mirror in `generators::compact` forms the same f32 operations in the same order (CON-13: CPU and GPU form the same f32 values).

### 4. Normals
The texel normal is the normal of the final surface. During the fade the drawn surface lies between base and final; the first version keeps the final normal (the shading leads the geometry by the fade time, a few hundred milliseconds), and a look test of the landing (two stills at w 0.5) decides whether the normal is blended with the base normal (the base normal needs a second plane or a recompute from the faded heights in the shader). The choice is recorded, not assumed.

### 5. Carrying the change out
`HeightField::level_change(dirs, level) -> Option<Vec<f64>>` (default `None`), implemented by `ChunkField` (the pyramid read at the tile's level, the same as `change_at_level`) and forwarded by every wrapper that forwards `height_at_level` and `level_aware`. The compact builder (`compact_tile_field`) asks the field for it with the tile's directions and stores the plane.

### 6. Length
`chunk_fade_ms` default 1500 (owner, 2026-10-24, criterion 3 amended): the change of a landing chunk fades in over 1.5 s. The entry step of the A-net pyramid (top level 15) was measured in `m8-anet-chunk` section 16: p99 3.90 m, max 17.55 m against the base at the level-14 vertices. The step per frame of the fade is `max |change| / (fade s x fps)` = 17.55 / 90 = 0.19 m at 60 fps (p99 0.04 m), below the 0.25 m of criterion 3; the screen step that criterion names is at most about 0.5 px p99 with blending, which the landing stills (task 3.3) measure at the review poses. The stand-in chunk of the tests (30 m) needs 2 s for 0.25 m per frame, so its tests state their own fade length.

### 7. Golden-review
The candidates are produced by `cargo xtask test gpu` (the lit and terrain views with no chunk ready) and the new landing stills. A change the policy classifies as rounding noise or a declared change of a debug or shaded view is blessed by the skill; anything the policy escalates (first goldens of a class, lit, sky and water views) stops the work and goes to the owner.

## Risks
- The render path: two bindings, one more node word, the shader; every pipeline layout changes.
- The reference renderer must mirror the shader or the differential test (REND-031) fails.
- The borders: two chunks landing at different times give a weight step at their border for as long as the minimum rule takes; the test measures it.
