## Why
Criterion 7 (owner, 2026-10-26): floor 200 m/s at 300 m, met in M8; measured 145, 95, 115, 115 m/s at 30, 100, 300, 1000 m (viewer pools). Draft: to be finalised against the limiter line of the 300 m flight (chunk wait, plan or workers) from the next perf window.

Findings from the code (2026-10-26 and 2026-10-27, read only):
| Fact | Where |
|---|---|
| The viewer and the scenes passed `&[]` to `chunk_frame`: no chunk was prefetched. Wired in #782; the 300 m figure stayed 115 m/s (window 8, about 840 chunks asked per trial) | `app/window.rs`, `app/streamperf.rs` |
| `prefetch_chunks` (STRM-042: one chunk edge ahead along travel, and the predicted chunk) exists and is tested, but nothing in a binary calls it | `frame/chunkshare.rs:171`, `frame/tests/chunk_share.rs` |
| Chunks start in order of distance to the camera; prefetch gets only workers no wanted or stale chunk needs | `streaming/chunks.rs` `update_with` |
| Lead needed: job latency L (queue + solve 3.3 s + horizon about 2 s + tile refresh, seconds) times speed v; at 200 m/s and L = 10 s that is 2 km, under half of a 4.9 km chunk edge | arithmetic |
So at 115 m/s the camera meets a missing chunk at the edge of the wanted set, and every new chunk row costs the full job latency in stand-in drawing.

## The trial measures the start-up, not a steady flight
The envelope trial settles at the start view with the camera standing still, then flies 12 s from a start moved so that the flight crosses a chunk edge half way (`trial_start`, `EDGE_FRACTION` 0.5). Let D be the distance at which a chunk becomes wanted (the plan reads it) and J the job latency of a chunk (5 to 7 s, window 8: two chunks land per trial after that wait).
| Quantity | Value |
|---|---|
| Edge distance at the start | 6 s x v |
| Chunk wanted from | t = 6 s - D / v after the start; at t <= 0 it was wanted during the settle and is ready when the flight starts |
| Chunk ready at | t + J |
| Stand-in share above the bound | whenever t > 0 and the flight is shorter than t + J + 1 s: at any speed above v* = D / 6 |
v* = D / 6 is the measured figure: 115 m/s gives D = 690 m, and 145, 95, 115, 115 m/s at 30, 100, 300, 1000 m give D = 870, 570, 690, 690 m. Check: the chunk trace stamps the first time a chunk is wanted; the model predicts a first-wanted time of 6 s - D / v and a landing J later. Prefetch cannot help in this trial: the settle is stationary, the travel estimator has no velocity, and the chunk is wanted within J of the first moving frame at every speed above v*. That is why 115 m/s did not move with prefetch (nor with 4 against 6 workers).

## Lead in a steady flight (300 m, 200 m/s)
| Quantity | Value |
|---|---|
| Chunk edge | 4.9 km |
| Camera at fraction f of chunk A, next chunk B wanted when the edge is within D (700 m) | (4,900 (1 - f) - 700) / 200 = 21 (1 - f) - 3.5 s |
| B asked by the prefetch (one edge ahead along travel) | from the first moving frame in A: the point p + 4.9 km is in B for every f > 0 |
| Lead of the ask over the need | at most 21 s, at least (4,900 - 700) / 200 = 21 s from entering A |
| Supported speed with one chunk ahead and J = 7 s | v <= (4,900 - 700) / 7 = 600 m/s |
So in a steady flight the existing prefetch already leads by about 3 x J at 200 m/s along the track. Open: chunks to the side of the track (a track off the grid axis needs the diagonal and lateral chunks: the tile reads include the 8 neighbours), and the tile refresh after a landing (learning 2026-10-26), which adds seconds to J.

## What Changes
1. **Fix the trial, not the streamer** (M8-V, crusader): the envelope trial gets a run-in. After the settle the camera flies the speed under test for at least `J + 5 s` (15 s at 200 m/s, 3 km) with the prefetch active, then the 12 s measured window starts with the next chunk edge half way into it, over ground the run-in did not cover. A second figure keeps the cold case: the same trial without the run-in, reported as "first visit after a stop" (limited by J, so by criterion 6, not by prefetch). TEST-047 names which one the 200 m/s floor is judged on: the run-in one.
2. **Corridor prefetch.** `prefetch_chunks` also names the chunks at the sides of the track: the chunk one edge ahead and its two lateral neighbours (the tile reads include the 8 neighbours), ordered by distance along the track divided by speed. Camera that does not travel: none (STRM-042 unchanged).
3. **Order of the wanted chunks by time-to-need** where the camera moves: distance divided by the closing speed along the line to the chunk. At rest the distance order of today. Built on the job priority classes of crusader's pool PR (horizon yields to chunks; prefetch is below wanted), not on a second mechanism.
4. **Measurement.** One window: the envelope at 300 m and 1 km with the run-in, prefetch on and off, and the cold figure; the log line names the limiter per trial (chunk wait, plan, workers, tile refresh) and the first-wanted and landing times of the new chunks.

## Questions
- If with the run-in the limiter is the tile refresh after a landing, the lever is the refresh priority, not the prefetch.
- J is a measured number: the landing time minus the request time in the log (median and worst). The run-in length follows it.

## Impact
- Spec: TEST-047 (the run-in and the two figures), STRM-042 (lateral chunks, order), STRM-040 (wanted order).
- Code: `app/envelopeperf.rs` (run-in), `frame/chunkshare.rs` (lateral chunks), `streaming/chunks.rs` (order, on crusader's priority classes; not the stale handling or `anet.rs`).
