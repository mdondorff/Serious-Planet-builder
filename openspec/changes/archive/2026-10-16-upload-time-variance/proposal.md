## Why

Perf window 2 (2026-10-16, RTX, 16384 store): in the dive 712 frames had `upload_ms` over 2 ms (median 2.69 ms) at the same median 53 KB in 2 calls that other frames write in 0.59 ms, mostly in the low bands where tile generation is busy. The bytes were never the cost (F5); something else in the upload column varies with the load.

The measurement (docs/learnings.md, 2026-10-16, "Upload time varies with the wants, not the bytes"): `upload_ms` is the whole of the managed loop's second half, not only the queue writes. In a frame with a tile waiting it rebuilds the class sets of the upload ranking (`Classes::of`: every draw tile with all its ancestors, no early stop, plus the residency closure) whenever the draw or residency wants changed, and the dive's camera moves, so the plan changes nearly every frame; it also rebuilds the prefetch closure from scratch every uploading frame. On top of that, any step of the frame can lose the CPU to the 20 generation threads that share the machine with the frame thread, the plan worker and the driver, and the batched path allocates its staging data on the heap the generation threads allocate from.

## What Changes

- frame (managed loop): the class sets are built with walks that stop at an ancestor already holding a priority at most the walked tile's (identical sets, a fraction of the hash operations); the prefetch closure is rebuilt only when the prefetch wants change; the waiting list and the batch are kept between frames; `ManagedLoop::ranking_rebuilds` counts the rebuilds.
- render: the batched upload path stages everything in buffers the renderer keeps (`BatchScratch`: converted texels, write lists, sort orders, run rectangle, record run, vertex-tile bytes), so a frame allocates nothing there after warm-up; `batch_scratch_capacity` is the count the tests watch. Pools, images, calls and bytes are unchanged.
- streaming/app: the default generation pool is the machine's hardware threads less `GEN_RESERVE` = 2 (18 on the 20-thread reference machine), `--gen-reserve N`; `--gen-workers N` still fixes the size. A pure function `GenWorkers::workers_for(cores, reserve)`.
- perf: `protocol.gen` (`workers`, `ahead`, `reserve`) recorded in streaming logs; logs without it read and are accepted; a baseline that recorded other settings is not compared as equal (`gen settings differ`), one that recorded none is compared and the pools are named.
- example: `crates/render/examples/upload_contention.rs` (the real upload path with N busy workers beside it) and the ablation table in docs/learnings.md.
- spec: REND-018, STRM-020 and TEST-012 modified (no new ids).

Not in this change: F9 (pre-filled dive), F6 (slow pre-existing tests), thread priorities (would need an unsafe FFI call or a new direct dependency; not taken), any change of what is uploaded or in which order. No golden, pinned count or mesh version changes.
