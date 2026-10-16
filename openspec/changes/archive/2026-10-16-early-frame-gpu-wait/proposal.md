## Why

Perf window 2 (2026-10-16, RTX): the dive's slowest frame is frame 2 at 41.01 ms (cap 40) and dive-fast's is frame 1 (about 48 ms in window 1), while the CPU columns of those frames are under 1 ms: the frame thread waits on the GPU at the frames-in-flight bound, so GPU work submitted in the first frames takes tens of milliseconds. That is lazy first-use work of wgpu and the driver. `warm_up` (first-frame-warmup, F4) already submits and waits for a throwaway frame, but that frame touches less than the measured frames do: it records only ring set 0 (a frame uses `frame % 2`, the first frame set 1), has no timestamp pass, draws a single indirect record, and nothing counts what it touched, so a part left out cannot be seen by a test.

## What Changes

- render: the warm-up frame draws from every set of the per-frame ring (each set's buffers and bind groups are written, bound and drawn from), runs the timed pass with its query resolve and readback map where the adapter has timestamps, and draws two indirect records (a multi-draw with a count above one) in the active draw mode; it is still one submission, polled to completion, before frame 0. The vertex pool is cleared whole and block 0 / atlas 0 is bound as before.
- render: `FirstUse` (`first_use()`, `RING_SETS`): distinct ring sets, pipelines, timestamp passes and draw modes a submission has used, and the warm-up's waited submissions. Real frames update the same masks, so a test asserts by counts that frames 0..3 first-use nothing after the warm-up (a plan beyond the ring builds a new ring and resets its mask).
- spec: REND-017 modified (the throwaway frame is representative: every ring set, every pipeline, the timed pass, the multi-draw path; the first-use counters).

Not in this change: pool regions beyond the first block / atlas (a second atlas still allocates and is first used when the manager places a tile there: bytes follow demand, STRM-006), the data written by the first uploads (it travels with the first submission and is real work), any perf-window measurement. Frame 0 stays a measured frame. Images, goldens, digests and `MESH_VERSION` are unchanged.
