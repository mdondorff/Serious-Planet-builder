## Why
Tracked "Upload pacing after a teleport" (job-system PR 4): the lit teleport-2km frame p99 rose 4.5 -> 6.2 ms as tiles land 2.7x sooner. A per-frame upload budget already exists (ADR 0033 point 9, STRM-006: 1 MiB of tile bytes, class order, one-tile rule), so the question is whether the slow frames are capped ones and how much of the rise is upload. The frames log has `upload_bytes`, `uploaded` and `upload_calls`; it lacks whether the cap was reached and the node-record bytes.

## What Changes
- `frames.txt` gains the last columns `deferred` (finished tiles that waited for the budget) and `node_bytes` (the plan's node-record bytes), from `FrameCounts`; recorded only. TEST-031 modified; an old log is refused as old-format.
- No pacing change. The measurement runs in the next authorized window with SPIKE-90's second window; the choice (a lower cap, or an allowance that shrinks after an over-run) follows the split.

## Capabilities
### Modified Capabilities
- `testing`: TEST-031
