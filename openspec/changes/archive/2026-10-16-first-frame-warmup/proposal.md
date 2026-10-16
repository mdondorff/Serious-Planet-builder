## Why

Perf window 1 (2026-10-16, RTX): the first frame of the dive records in 58 ms (`record` column 58.4 ms at frame 1; dive max 58.46 ms over the 40 ms cap, dive-fast max 45.6 ms at frame 1). Frame 0 does one-time work inside the measured frame. ADR 0032 keeps frame 0 in the measured frames, so the work has to move out of it, not the frame out of the measurement.

Measured before any fix (software adapter, release, `crates/render/tests/gpu_warmup.rs`, docs/learnings.md): the renderer builds both pipelines in its constructor already (compact renderer 259 ms the first time in a process, 3.5 ms after), so the one-time work that lands in a frame is: the first draw through each pipeline (driver state and shader work: compact frame 0 9.4 ms against a median of 5.7), the lazy zeroing of a buffer or atlas at its first use in a submission, the allocation of the vertex pool (whole, on the first level 0-2 upload), of block 0 or atlas 0 (on the first level 3+ tile; in the dive that is frame 15 of 24, whose `record` was 5.2 ms against 0.6) and the growth of the per-frame ring from its 256-node start when a plan is larger.

## What Changes

- render: `TerrainRenderer::warm_up(ctx)`: allocates the vertex pool and the first block (block 0, or atlas 0 with its bind group), grows the ring to `WARM_NODES` (2048, within the share's node-record bytes), and submits one throwaway frame (a zeroed tile through each pipeline of the format, in the active draw mode, buffers cleared, polled to completion). No tile becomes resident and no frame counter moves. The bytes allocated are the pure model's for one block plus the vertex pool.
- render: `InitCounts` (`init_counts()`): pipelines created, pool allocations (vertex pool, blocks, atlases) and ring builds, the counters tier B compares around frames.
- app: `Rig::new` (the pipelined loop and every streaming perf scene) calls `warm_up` before the first frame; the call is outside the timed loop, frame 0 stays a measured frame.
- spec: REND-017 (new): no one-time initialisation inside a measured frame.

Not in this change: upload batching (F5), test-fast time (F6), the on-demand growth beyond block 0 (a second block still allocates when the pool needs it: bytes follow demand, the share stays a ceiling). No golden, pinned count or mesh version changes.
