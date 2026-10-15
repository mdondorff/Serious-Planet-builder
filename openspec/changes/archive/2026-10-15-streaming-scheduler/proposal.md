## Why

M4 item 7 (the scheduler, ADR 0031 points 2, 3 and 6). The streamer today wants exactly the nodes the frame draws and cancels everything else, so looking around is a cache miss. ADR 0031 splits the wanted set into the residency set R (a pure function of the camera position, the LOD parameters and the horizon cut) and the draw set D (R cut to the frustum), orders requests in three classes and cancels only for leaving R and the prefetch set. This change builds the scheduler on the injected clock, I/O and spawner (CON-04). The residency manager (pooled slots, the share, eviction order, degradation of point 4) and the prefetch source come in later changes.

## What Changes

- frame: `residency` module. `ResidencySet`, a pure function of `(position, LodParams, field)`: the position-only selection (no view, horizon cut from the parameters) plus all ancestors of its leaves, each with a distance (STRM-015). No view direction can enter: the signature takes a `PlanetFixed`.
- streaming: request classes in `Streamer`. `Wants { draw, residency, prefetch }` and `Streamer::begin_frame_wants`. Queue order: class (missing D, rest of R, prefetch), then ancestors before descendants / coarse before fine, then priority. Cancellation only for tiles in no class (never for the frustum); a queued prefetch request is cancelled first when the prefetch set changes; prefetch dispatches only while no higher class is queued and only into free capacity, and never evicts. `TileSource::load_staged` gives in-flight jobs a stage boundary at which they stop; a cancelled job's result is dropped, not made resident. `predicted_position` (translation only) for the later prefetch source.
- `Streamer::begin_frame(&[(tile, priority)])` stays: it is `begin_frame_wants` with only a draw class, so the `--stream` path and the dive test behave as before.
- streaming-cache spec: STRM-005 rewritten, STRM-015 (residency set) added, per ADR 0031's spec-delta section.

Not in this change: the residency manager and pooled slots (STRM-006/007 rewording), the share, degradation (point 4), the prefetch source (a stub: the empty set), frame pipelining, wiring R into the interactive loop.

No golden and no pinned count changes.
