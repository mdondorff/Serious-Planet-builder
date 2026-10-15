## Why

M4 item 7. The scheduler has a prefetch class and a `predicted_position` (#127) but the app passes an empty prefetch set (#133), so a camera moving at speed meets every new tile as a missing one. ADR 0031 point 2.3 defines the source: R at a position predicted from the camera's translation only, requested only into free share.

## What Changes

- frame: `prefetch` module (GPU-free): `TravelEstimator` (velocity from the position history on the injected clock; teleport reset), `PrefetchConfig` (look-ahead and caps, tunable constants), `prefetch_set` (R ahead minus R here, coarse before fine, nearer first), `cap_to_free`.
- frame: the plan worker computes the prefetch set after R, on the plan's cache (`PlanWorker::submit_predicted`, `ResidencyOutcome::prefetch`); `PlanPipeline::begin_frame_predicted`.
- frame: `ManagedLoop::with_prefetch` feeds the set, capped by the free share of the main pool, into the manager's wants; `FrameCounts` gains `prefetch` and `cancelled_prefetch` (not log columns).
- app: the pipelined loop turns it on with the streamer's clock. `--stream` (one camera, no travel) keeps an empty prefetch set.
- streaming-cache spec: STRM-018 new, STRM-017 reworded.

No golden, plan count, pin or `MESH_VERSION` change.
