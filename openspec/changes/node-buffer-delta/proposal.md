## Why
- Inbox #35 (planner, 2026-11-02): the lit teleport frames cost 2.1-5.1 ms in `record_ms` and one frame in every round (frame 14, 6-7 ms) falls outside the logged phases; `upload_ms` is only 0.23-0.27 ms.
- The first idea, writing only the changed node ranges, does not work: the node record (208 B) carries the camera-relative origin of every node, so any camera move changes essentially every record (CPU measurement of `node_record_bytes`: 0.01 m moves change 15211 of 15666 records, 0.5 m and more change all of them; a changed run is one `write_buffer` call each).

## What Changes
- PR A (this change): `TerrainRenderer::render` times its CPU phases (`FrameTiming::cpu_phases`: node build, node write, tile references and draw arguments, recording, submit) and the streaming perf frames log gets five more columns after `node_bytes` (`node_build_ms node_write_ms refs_ms encode_ms submit_ms`). Recorded only, not judged; an older log is refused as old-format as with every earlier column change.
- The Tracked line "Upload pacing after a teleport" in `docs/status.md` is rewritten with these numbers and the decision rule.
- PR B (not here, after the next window's readings): split the record into a static part and a per-frame origin buffer (208 -> 16 B per node) if the phases show the node build and write are the cost.

## Impact
- Code: `render::terrain` (five `Instant` reads per frame, tens of nanoseconds each), `perf::{streaming, accept}` (log format), `app::streamperf`.
- No requirement changes; no pixel changes; no new goldens.
