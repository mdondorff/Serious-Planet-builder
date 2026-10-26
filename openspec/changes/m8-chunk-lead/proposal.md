## Why
Criterion 7 (owner, 2026-10-26): floor 200 m/s at 300 m, met in M8; measured 145, 95, 115, 115 m/s at 30, 100, 300, 1000 m (viewer pools). Draft: to be finalised against the limiter line of the 300 m flight (chunk wait, plan or workers) from the next perf window.

Findings from the code (2026-10-26, read only):
| Fact | Where |
|---|---|
| The viewer, the stream scene and the envelope scene call `chunk_frame(tiles, camera, &[])`: no chunk is ever prefetched | `app/window.rs:744`, `app/streamperf.rs:277` |
| `prefetch_chunks` (STRM-042: one chunk edge ahead along travel, and the predicted chunk) exists and is tested, but nothing in a binary calls it | `frame/chunkshare.rs:171`, `frame/tests/chunk_share.rs` |
| Chunks start in order of distance to the camera; prefetch gets only workers no wanted or stale chunk needs | `streaming/chunks.rs` `update_with` |
| Lead needed: job latency L (queue + solve 3.3 s + horizon about 2 s + tile refresh, seconds) times speed v; at 200 m/s and L = 10 s that is 2 km, under half of a 4.9 km chunk edge | arithmetic |
So at 115 m/s the camera meets a missing chunk at the edge of the wanted set, and every new chunk row costs the full job latency in stand-in drawing.

## What Changes
Each lever is built only if the limiter line says its phase is the limit.
1. **Wire the chunk prefetch.** The three callers pass `prefetch_chunks(position, predicted, R)` with the predicted position of the travel estimator the tile prefetch already has (`ManagedLoop`, `PrefetchConfig`). Expected: the next chunk row is requested a chunk's travel time (24 s at 200 m/s) ahead, more than L.
2. **Prefetch by time-to-need.** Replace "one edge ahead and the predicted chunk" by the chunks inside the corridor travelled in `lead_ms` (default: the measured L plus a margin, about 12 s) and the view's width, ordered by distance along the track divided by speed, not by distance. A camera that does not travel prefetches nothing (STRM-042 unchanged).
3. **Order the wanted chunks by time-to-need** where the camera moves: distance divided by the closing speed along the line to the chunk, so a chunk ahead goes before an equally distant one behind. At rest the order is the distance order of today.
4. **Priority classes and share** (ADR 0083 points 2 and 3, accepted 2026-10-26): wanted chunks and edits are "visible streaming and edits", prefetch is "prefetch"; the chunk pool is 6 workers x 2 base threads in the viewer. Prefetch uses at most the workers no wanted or stale chunk needs (as today) and leaves the cores terrain's share keeps free for the frame. Eviction counts prefetched chunks as wanted (as today).
5. **Measurement.** The envelope scene at 300 m (and 1 km) with the chunk prefetch on and off, one window; the log line names the limiter per trial (chunk wait, plan, workers, tile refresh).

## Questions
- If the limiter is the tile refresh after a landing (learning 2026-10-26, the rebuild of the tiles under a landing takes seconds), prefetch only moves it earlier; the lever is then the refresh priority, not this change.
- `lead_ms` is a measured number, not a guess: it comes from the job latency in the log (landing time minus request time, median and worst).

## Impact
- Spec: STRM-042 (corridor and time-to-need ordering), STRM-018 (the chunk set from the same estimator), STRM-040 (wanted order), TEST-047 (limiter line).
- Code: `app/window.rs`, `app/streamperf.rs`, `app/envelopeperf.rs` (the call), `frame/chunkshare.rs` (the corridor), `streaming/chunks.rs` (the order). Not `chunks.rs` stale handling or `anet.rs` (crusader, criterion 6).
