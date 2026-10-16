## Design

**Measured first** (docs/learnings.md, F5): per-call overhead is real but small on WARP and Vulkan/RTX (236 calls: 0.74 ms WARP, 0.40 ms RTX; 9 calls: 0.12 / 0.18 ms), and the byte-wise conversion of a vertex tile cost 0.11 ms (19 tiles: 2 ms). So the change does both: fewer calls for the small writes where the call is the cost, and slice copies for the conversion.

**Compact tiles.** The manager's slots are close to adjacent (lowest-free-slot): at 118 tiles a frame about 22 runs. The renderer holds the batch's texel and record writes in ordered maps keyed by slot (a later write to the same slot replaces an earlier one, as in the per-slot order), then issues: texels, one rectangle per run of consecutive slots of one block in one atlas row (`k*33 x 33` texels, rows interleaved on the CPU, `bytes_per_row = k*264`; `write_texture` needs no 256-byte alignment); records, one `write_buffer` per run of consecutive slot indices. No staging buffer of our own and no encoder: `write_*` already stages, and the saving is in the number of staging allocations and copy commands.

**Vertex tiles.** Written at once (one call, 53,724 bytes): coalescing would copy the megabyte again (micro-benchmark: 19 tiles per slot 0.12 ms, concatenated 0.27 ms on WARP; equal on RTX).

**Managed loop.** The frame's allowed tiles (budget order, unchanged) are collected and handed over as one `upload_batch(&[(tile, slot)])`; the default implementation is the loop over `upload_at`, so fakes and test sinks keep working; the loop's bookkeeping (resident, slots, counts) happens after the batch in the same order. `FrameCounts::upload_calls` is what the sink reports.

**Reference path.** `UploadMode::PerSlot` keeps the old one/two calls per slot; tests compare pool contents (read back), images and bookkeeping between the modes.

**Bound.** Compact calls per batch = runs of adjacent slots (texel runs per atlas row, record runs), at most the tiles written; asserted at tier A on a coalescing fake sink for N = 1, 10, 118 and at tier C on the renderer.

**Open for window 2.** The p99 of `upload_ms` on the RTX (the target is 2 ms at the cap); a first-use atlas allocation inside a frame is not part of this change (REND-017 warms block 0 only).

**Deviation from F5's wording (vertex pool).** F5 asked for one write per contiguous run for the vertex pool too. Measured, it does not help: 19 vertex tiles at the cap cost 0.12 ms as 19 `write_buffer` calls and 0.27 ms concatenated into one on WARP (equal, 0.14 / 0.15 ms, on the RTX), because a call carries 53,724 bytes and the concatenation adds a pass over them. Vertex tiles stay one call each; the compact atlas batching is the fix. The real saving for vertex tiles is the byte conversion (0.11 -> 0.012 ms a tile).

**Errors.** `upload_batch` failing aborts `begin_frame` with the error (as `upload_at` failing did); the tiles the renderer wrote before the failing one are written but the loop records none of the batch, which is fine because the loop does not continue. The scheduler-equivalence digests fold the first `FrameCounts::DIGESTED_COLUMNS` (12) columns, the width they were taken with; no pinned value changed.
