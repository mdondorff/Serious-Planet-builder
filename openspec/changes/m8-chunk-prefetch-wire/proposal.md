## Why
STRM-042 (chunk prefetch along travel) is implemented (`frame::chunkshare::prefetch_chunks`) and tested, but the viewer (`window.rs`), the stream scene and the envelope scene called `chunk_frame(tiles, camera, &[])`: no chunk was ever prefetched. Criterion 7 (floor 200 m/s at 300 m, measured 115) was measured without it.

## What Changes
- `chunkstage::chunk_prefetch(travel, position, radius)` names the chunks from the managed loop's travel estimator; the viewer and the stream and envelope scenes pass it to `chunk_frame`.
- The flight trace counts the chunks asked (`prefetch_asked`, in the envelope log).
- STRM-042 gets the sentence that the viewer and the scenes SHALL pass it.

## Impact
- Spec: STRM-042. Code: `app/chunkstage.rs`, `app/window.rs`, `app/streamperf.rs`, `app/envelopeperf.rs`. The 300 m figure changes: it is re-measured in the next window with prefetch on.
