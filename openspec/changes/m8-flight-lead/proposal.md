## Why
The speed-envelope trace of window 14 (`target/perf-w14`, criterion 7) shows late chunks asked about 4.7 s before they were wanted at 340 to 360 m/s, against a 12 s prefetch lead and a solve of 6.1 to 6.7 s (up to 10 s). The track asks a chunk when its centre is within the plan's reach of a sample, but the plan wants a chunk when its nearest tile is read, half an edge (2.4 km, 7 s at 340 m/s) nearer. Above 440 m/s, chunks wanted at the window's first frame were never asked in the 12 s run-in.

## What Changes
- `track_chunks` asks every chunk whose centre is within `max(reach, one chunk edge) + half an edge` of a track sample: half an edge further than the plan's reach (the lead is the 12 s it was meant to be), and the next chunk is asked on entering the current one.
- Tier A test of the floor and of the asked-at-once case; the effect on the late chunks is measured in the acceptance window (`perf stream envelope --only-steady`).

## Impact
- Spec: STRM-042 (the ask distance); a larger prefetch set (about one more ring of chunks along the track), within the prefetch class of STRM-060.
- Code: `frame/chunkshare.rs`; test `frame/tests/chunk_share.rs`.
