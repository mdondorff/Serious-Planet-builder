## Why
Window 13 (2026-10-19): the dive's gated measures `frame max 70.9 ms at frame 2` and `upload bytes 3.357 MiB per frame (cap 1 MiB)` were over budget in a valid session (window 12: max 85 ms). 3.357 MiB is the start-up load of the 384 root tiles of the render floor 3. STRM-001 and ADR 0064 say the roots load before frame 0, outside any measured frame; the harness loaded them into CPU memory before frame 0 but wrote them to the GPU in frame 0, where the exemption of the per-frame cap lets them through and frame 0 pays 3.4 MiB of upload (3.74 ms upload, 70 ms wall at frame 2 in the dive).

## What Changes
- `ManagedLoop::upload_roots(sink)` (STRM-030): writes the resident tiles at or above the floor to the sink in one batch, outside the cap, and records them as resident, as the first frame did. A caller that does not call it gets the old behaviour (the first frame writes them).
- The perf rig (`pipelined::Rig`, used by the streaming scenes, the lit streaming scenes, the edit-latency scene and the window's script runs) calls it at the end of its start-up, after the roots load, so frame 0 is a steady frame.
- Not changed: the metric, the cap, the budgets, the exemption of ADR 0064 (it still covers a caller that does not call `upload_roots`).

## Impact
- Specs: STRM-030 (added, streaming-cache).
- Code: `frame` (`ManagedLoop::upload_roots`, `RootUpload`), `app` (`pipelined::Rig`).
- Goldens: none. Perf: the dive's frame max and upload bytes at frame 0 (to be confirmed in the next window).
- Tests: tier A (the roots are in the sink before frame 0 and frame 0 uploads none; without the call nothing changes), tier C (the smoke streaming scenes' frame 0 writes at most the cap).
