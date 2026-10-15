## Why

M4 item 7, the last streaming item before the perf scenes: CON-21 limits GPU uploads to 2 ms per frame and ADR 0032 makes the upload bytes a hard measure in every case, but the managed loop writes every tile the streamer finishes in the frame it arrives (a teleport finishes hundreds). ADR 0033 point 9 sets a starting cap of 1 MiB per frame on tile data, deferred in ADR 0031's class order; nothing applies it yet.

## What Changes

- frame: `planet_frame::upload` (GPU-free): `UploadBudget` (`tile_bytes_per_frame`, default 1 MiB; `node_records_in_cap`, default off), the class order (`UploadClass`, `sort_waiting`) and the rule `upload_count` (the longest prefix that fits, at least one tile).
- `ManagedLoop`: finished tiles wait in a pending set, are ranked by the wants (draw set with ancestors and the roots, R, prefetch, rest; coarse first, then plan order) and written through `upload_at` while their bytes fit; the rest stay finished-but-not-placed. `TileSink::upload_bytes` gives each tile's size in the sink's format (default: the vertex format), so the compact sink of the compact switch-over only overrides one method. `ManagedLoop::with_upload_budget` configures it; `pending()` reports the waiting tiles.
- Per-frame counts and log columns after the scheduler counts: `upload_bytes`, `deferred`, `node_bytes` (STRM-017, TEST-012).
- Specs: STRM-006 gains the upload cap (the "proposed" caveat is replaced; the value stays provisional), STRM-017 and TEST-012 the columns.

## Reading of the cap (decided here)

The 1 MiB covers **tile data only**, as ADR 0033 point 9 says ("applied to tile data", "about 118 compact slots"). The node records (614,164 B at ground-1m, #134) are a CPU write the frame needs and cannot defer; they count against the 2 ms (owner's decision, ADR 0033 status) as their own reported line, not as part of the cap. `node_records_in_cap` lets a perf window fold them into the cap by configuration if 1 MiB plus the records exceeds 2 ms.

## Not in this change

- The cap value (ADR 0033 point 9 stays proposed; a perf window measures the time it costs). No golden, pinned count or MESH_VERSION change; the renderer and `skeleton.rs` are untouched, the settled image equals the synchronous frame.
- The compact sink's `upload_bytes` (the compact switch-over, #137, overrides it).
