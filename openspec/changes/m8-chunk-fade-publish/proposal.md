## Why
After `m8-chunk-fade-increment` (#711) a tile draws `drawn_old + w x (final' - drawn_old)`: exact for one fade at a time. Two things still crack (numerics review of #711, 2026-10-26):
- Chunks that land within the fade length of each other (a river: the normal case) give a tile that reads both two clocks at once; it keeps the weight of the first at the second's landing while its neighbour that reads only the first goes on to 1: a crack up to the rest of the first change (up to 39 m).
- A fresh tile (an LOD child, never uploaded before) built during a fade has its plane over the base, so at weight 0 it drops the changes of earlier epochs that its neighbours still show.
One weight and one plane per tile cannot describe two fades at once, so there must never be two.

## What Changes
- **Epochs are sequential.** A landed chunk is ready in the `ChunkManager` as now, but it is published to the tile pipeline (`snapshot`, `is_ready`, `result`, `result_hash`, so the field, the keys, the horizon jobs and the weights) only when no fade is running; everything that landed meanwhile is published together, one epoch. New on `ChunkManager`: `publish(&[ChunkKey])`, `unpublished() -> Vec<ChunkKey>`; the four readers above see published chunks only; a chunk evicted is unpublished. `ChunkView` gains `publish` (ManagerView forwards it).
- **The loop publishes.** `ManagedLoop::chunk_landed` queues the landed keys; each frame, when every chunk clock is at 1 (`ChunkFade::idle`) and no stale tile is drawn, it publishes the queue and runs today's landing handling (refresh, held weights) on it. Nothing else about the increment planes changes.
- **Fresh tiles wait.** A tile with no predecessor that reads a chunk whose fade is running is not uploaded until the fade ends (`ChunkFade::is_fading`); its parent stays drawn (CON-22: no frame waits, the plan draws the coarser tile as for any tile that is not resident yet). The alternative, a plane of the running epoch's increment only, needs the per-chunk change to be separable in the compact builder; it is not (`level_change` returns the sum over the chunks a texel reads), so the wait is chosen.

## Latency
- A chunk's landing (the manager's `ready`, criterion 7's "within 10 s at a 300 m pan") is unchanged. What the eye sees is later by at most one fade (1.5 s) plus the rebuild of its tiles (the upload budget, tens of ms): the chunk waits behind the running epoch, then fades in.
- Along a river chunks come in a stream: they are published in batches, one per fade length at most (every 1.5 s at the busiest), each batch fading in together. A batch is every chunk that landed since the last, so no chunk waits more than one fade for the next batch.
- Fresh tiles under a fading chunk appear when it ends (at most 1.5 s late); the parent is drawn meanwhile.

## Impact
- Spec: STRM-054 (publication), STRM-053 and STRM-052 unchanged in meaning (their invariant now holds by construction). Tier A.
- Tests: two chunks landing 0.5 s apart under tiles that read both: the shared edge stays closed at every frame (real CompactTiles); a fresh tile next to a fading one is not uploaded until the fade ends; no landed chunk waits more than one fade for publication; the manager's readers return published chunks only.
- Code: `streaming/src/chunks.rs` (the gate; explorer is told which functions), `frame/chunkshare.rs`, `frame/managed.rs`, `app/window.rs` (the landed keys are queued, not applied).
- Cost about one day.
