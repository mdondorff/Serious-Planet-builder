## Why
Window 13, part B (`edit-latency`, 2026-10-19) panicked at `crates/streaming/src/layer.rs:214` ("a member of a closed layer is listed") in the second altitude. Cause: a refresh request (`Streamer::refresh`, STRM-026) is queued under the class of the resident tile; when the wants change before the request starts and the tile leaves every class, `class_changed` returned early for a resident tile and left the request in the bucket of its old class. The next read of that bucket asked the old class's layer for the key of a tile it no longer lists. The editor can hit this by moving the camera right after an edit.

## What Changes
- A refresh request still queued for a tile that left every class is dropped and counted (`cancelled`, `cancelled_prefetch` for the prefetch class). The tile stays resident and marked stale and is requested again when something wants it (STRM-026 already says so).
- STRM-026 gets the sentence and a scenario; one regression test.

## Impact
- Specs: STRM-026 (modified, streaming-cache).
- Code: `streaming` (`Streamer::class_changed`).
- Goldens: none. Perf: none (one queue lookup on a class change of a resident tile).
