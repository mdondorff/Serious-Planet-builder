## Why
Window 9 (steady 300 m, 155 m/s): the limiter is chunk wait; the prefetch asked the chunk across the edge and the predicted one, not the chunks the plan also reads (#780 added the pair beside the one ahead). A fixed strip does not follow the view (1000 m sees more chunks), a turn or the altitude.

## What Changes
- The prefetch is the set of chunks the plan would read along the camera's travel: `frame::chunkshare::track_chunks` takes samples of the track half a chunk edge apart out to 12 s of travel and, at each, every chunk whose centre is within the reach of the plan now (the farthest wanted chunk centre). Nearest sample first, nearest chunk first within a sample; empty without travel over the ground.
- `app::chunkstage::chunk_prefetch_track` makes it from the travel estimator's velocity and the wanted chunks (the wanted ones are left out); `WorldTerrain::chunk_frame_with` hands the wanted chunks to it; the viewer and the stream and envelope scenes use it. `prefetch_chunks` (STRM-042's first form) stays for its tests.
- The 12 s lead (`PREFETCH_LEAD_S`) is a measured number: a chunk job takes 5 to 7 s and its tiles a few seconds more to refresh; it is changed from the log (`first_wanted_s`, `prefetch_asked`).

## Impact
- Spec: STRM-042. Code: `frame/chunkshare.rs`, `app/chunkstage.rs`, `app/worldfield.rs`, `app/window.rs`, `app/streamperf.rs`. No golden. Measured in the next window (steady curve, four altitudes).
