## Context
Inputs: `openspec/changes/m8-chunk-field/design.md` sections 6 and 14 (the weight and its limit), the compact format (`crates/generators/src/compact.rs`: a 128-byte header, the climate block, a pad word at `SLOT_PAD_OFFSET`, 8-byte texels), the render pools and shader (`crates/render/src/compact.rs`, `terrain.rs`), the node record of the indirect draw (`frame::indirect_frame_bytes`), the reference renderer (REND-031).

## Decisions

### Built (step 3, 2026-10-25): where the plane lives on the GPU
The plane is not a second pool: the renderer keeps the planes of its resident tiles on the CPU (by slot) and, each frame, copies the plane of every node whose change is still fading in (`1 - w` above 0) into the frame's tile-reference buffer after the references (`FADE_PLANES` = 512 planes of `(cells + 1)^2` f32 padded to whole 16-byte entries); the node's reference holds the entry of its plane plus one, the node record the fade in the fourth lane of grid phase 6 (zero at weight 1, so the record does not grow and `NODE_RECORD_BYTES` stays). Why: a second pool binding would be a fifth storage buffer of the vertex stage (the adapter gate requires four, VIEW-001), a settled view reads and uploads nothing, and the bind groups of the pool blocks do not change when a chunk lands. The cost is that a fade moves up to 512 planes (2.2 MB at 32 cells) a frame for its 1.5 s; the overflow draws the final height.

**Known limit (seams, numerics review of step 3):** the plane is a tile's total change and the weight the minimum over the chunks it reads. Two tiles that share a border but read different sets of chunks (one reads C1 and C2, the other only C1, with C2 just landed) get different weights, so along their border the first draws the base where the second draws the final height: a crack of C1's change along the border, covered by the skirt, for 1.5 s. It needs a chunk to land while its neighbour's change is already in; the look review of the landing decides whether it matters (the remedy is a plane per chunk read, or the minimum over the chunks of the border cells only).

### 1. Where the change lives *(superseded by "Built (step 3)" above: the plane is in the tile-reference buffer, the fade is lane 6 of the node record, no header flag)*
A second plane of `(cells + 1)^2` f32 values per tile (metres, relative to the tile like the texel heights), in its own pool buffer indexed by the same slot index as the main pool. It exists only when the app has chunks on: without chunks the pool is a 4-byte dummy binding and the shader's branch is off, so the slot bytes, the pack payloads, the hashes of GEN-025 and the goldens of a world without chunks do not move (the pipeline layout gains one binding, which the golden run shows as pixel-identical). A tile without chunk detail writes zeros there (or is flagged off, see 3).

### 2. The weight *(superseded by "Built (step 3)" above: the plane is in the tile-reference buffer, the fade is lane 6 of the node record, no header flag)*
`ChunkFade::tile_weight(view, tile, now_ms)` (STRM-039) fills an f32 `chunk_weight` per node of the frame's plan record (one word more per node; the node bytes of the indirect path grow by 4, `node_bytes` in `FrameCounts` follows). The shader reads it with the node. The weight of a tile is its own chunk's; across a chunk border the two tiles' weights differ for up to `chunk_fade_ms`, so a vertex on the border can step by the weight difference times the change: the border vertices read the minimum of the weights of the chunks the tile reads (the cell reads already cross borders), which makes both sides rise together.

### 3. The shader *(superseded by "Built (step 3)" above: the plane is in the tile-reference buffer, the fade is lane 6 of the node record, no header flag)*
Position = base position from the texel height `h_q` minus `change` (the base height), plus `w x change`: `h = h_q - (1 - w) x change`. For `w = 1` or no change plane it is exactly the old arithmetic (bit for bit: `change x 0` adds 0.0). A flag bit in the slot header's flags word says "this tile has a change plane", so tiles without one skip the read. The CPU mirror in `generators::compact` forms the same f32 operations in the same order (CON-13: CPU and GPU form the same f32 values).

### 4. Normals
The texel normal is the normal of the final surface. During the fade the drawn surface lies between base and final; the first version keeps the final normal (the shading leads the geometry by the fade time, a few hundred milliseconds), and a look test of the landing (two stills at w 0.5) decides whether the normal is blended with the base normal (the base normal needs a second plane or a recompute from the faded heights in the shader). The choice is recorded, not assumed.

### 5. Carrying the change out
`HeightField::level_change(dirs, level) -> Option<Vec<f64>>` (default `None`), implemented by `ChunkField` (the pyramid read at the tile's level, the same as `change_at_level`) and forwarded by every wrapper that forwards `height_at_level` and `level_aware`. The compact builder (`compact_tile_field`) asks the field for it with the tile's directions and stores the plane.

### 6. Length
`chunk_fade_ms` default 1500 (owner, 2026-10-24, criterion 3 amended): the change of a landing chunk fades in over 1.5 s. The entry step of the A-net pyramid (top level 15) was measured in `m8-anet-chunk` section 16: p99 3.90 m, max 17.55 m against the base at the level-14 vertices. The step per frame of the fade is `max |change| / (fade s x fps)` = 17.55 / 90 = 0.19 m at 60 fps (p99 0.04 m), below the 0.25 m of criterion 3 *(that figure is for the 256 bake's river; on the shared store it is 0.65 m a frame and the criterion that counts is the screen step: section 6a)*; the screen step that criterion names is at most about 0.5 px p99 with blending, which the landing stills (task 3.3) measure at the review poses. The stand-in chunk of the tests (30 m) needs 2 s for 0.25 m per frame, so its tests state their own fade length.

### 6a. Criterion 3 measured on the shared store (2026-10-25)
Scene: the store `C:/DEV/SeriousPlanetData/global16384` (bake 16384 cells with the hill band, stored coupled run 2048 cells, 80 steps, fill only), the chunk of the largest river of the run at least 150 m high (the look review's anchor rule; face 0, 14.1 million km2 at its coarse cell), 8 m cells, 3 km halo, top level 15. `PLANET_ANET_MIN_HEIGHT_M=150 PLANET_ANET_STORE=C:/DEV/SeriousPlanetData/global16384 PLANET_ANET_RES=16384 cargo test -p planet --release --lib anet_steps_report -- --ignored --nocapture` (read only, 23 s).

| Quantity | Value |
|---|---|
| Change at the entry level (signed, 69,696 vertices) | p0 -58.34, p10 -49.64, p50 -25.93, p90 -0.44, p99 0.00, max +0.15 m (an incised valley, not an offset) |
| Entry step against the base | p99 56.91 m, max 58.34 m |
| Fade step per frame, 1.5 s at 60 fps (90 frames) | p99 0.63 m, max 0.65 m |
| Same on screen at the entry distance | about 4 km (level 14 spacing 19.1 m at tau 6 px, 1440p, 60 degrees: 3.2 m a pixel): max 0.20 px a frame, median 0.09 px |
| Step between level 15 and level 14 (parent) inside the range | p99 0.126 m, max 0.636 m |
| Step between level 16 and level 15 | p99 0.030 m, max 0.185 m |
| Step between level 14 and 13 / 13 and 12 (below the top, not drawn with the A-net's top at 15) | p99 0.317 / 0.719 m |

- The 256-bake figures of section 6 (p99 3.90 m, max 17.55 m, 0.19 m a frame) were for a smaller river; on the real run the largest inland river incises up to 58 m, so the 0.25 m a frame of the first statement needs a fade of 3.9 s at 60 fps (58.34 / 0.25 / 60), not 1.5 s.
- Criterion 3 as the owner amended it is a screen quantity: at 1.5 s a vertex of the deepest valley moves at most 0.20 px a frame at the distance where the chunk enters, inside the 0.5 px; p99 of the steps between pyramid levels (level 15 against 14: 0.126 m, 16 against 15: 0.030 m) is inside 0.25 m. These are computed from the field and the camera geometry, not read from rendered frames; the rendered agreement of the faded tile with the blended field is `gpu_chunk_fade` (REND-039).
- No frame waits for a chunk: the chunk path is asynchronous (CON-22, STRM-047: a tile reads only chunks that are ready, else the base; `managed.rs` and `horizon_world.rs` test it). Run on the shared store with the chunk stage on (windowed editor, the scripted flight from orbit to 1 m three times, about 4 minutes, RTX 3080 Ti, the title showing the stage on) the stall probe (VIEW-056) reported 0 frames over 1 s. Edits were not driven in that run (a background process cannot take the foreground to click), and the chunk counts are not in the title: the run shows that no frame stalled, not how many chunks landed.

### 6b. Known limits of the fade clock (2026-10-26)
- The clock of a chunk starts on the first drawn tile whose mesh key is the key the plan expects (`ChunkFade::drawn`): a plan made just before a landing can match the old tile for the frames the plan lags (about 30 ms of a 1.5 s fade, 2 % of the change).
- `upload_roots` starts no clock: the floor tiles it uploads lie above the chunk reads.
- A new version of a faded chunk keeps its weight and shows its difference at the swap; the increment plane is revisited at M8-7 (docs/status.md, Tracked).

### 7. Golden-review
The candidates are produced by `cargo xtask test gpu` (the lit and terrain views with no chunk ready) and the new landing stills. A change the policy classifies as rounding noise or a declared change of a debug or shaded view is blessed by the skill; anything the policy escalates (first goldens of a class, lit, sky and water views) stops the work and goes to the owner.

## Risks
- The render path: two bindings, one more node word, the shader; every pipeline layout changes.
- The reference renderer must mirror the shader or the differential test (REND-031) fails.
- The borders: two chunks landing at different times give a weight step at their border for as long as the minimum rule takes; the test measures it.
