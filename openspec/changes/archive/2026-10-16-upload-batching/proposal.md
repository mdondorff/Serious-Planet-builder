## Why

Perf window 1 (RTX, main ebd1fa4, dive): upload p99 4.00 ms for at most 0.31 MiB a frame, about ten times what the bytes justify (CON-21: uploads at most 2 ms). The tile writes were one `queue.write_buffer` per vertex tile and two calls per compact tile (a `write_texture` and a record `write_buffer`), up to 236 calls at the 1 MiB cap, each a staging allocation and a copy command; and the conversion of a tile to bytes went through byte-wise `flat_map` iterators (about 0.11 ms per vertex tile, about 8x a slice copy).

## What Changes

- render: `TerrainRenderer::upload_tiles_at` writes a frame's compact tiles with one `write_texture` per run of adjacent slots in one atlas row and one `write_buffer` per run of adjacent record indices (REND-018); vertex tiles stay one call each (measured: a coalescing copy adds a pass, no gain). `UploadMode::{PerSlot, Batched}` (default batched; `--upload per-slot|batched` for the reference), `UploadCounters` (`write_calls`, `staging_bytes`), `read_compact_pools` (test aid), faster byte conversion.
- frame: `TileSink::upload_batch` (default: the loop over `upload_at`, one call per tile), `UploadStats`; the managed loop hands the frame's tiles over as one batch in upload order; `FrameCounts::upload_calls` (a new last column `upload_calls`).
- app/perf/xtask: the renderer sink overrides `upload_batch`; `frames.txt` gets `upload_calls`; `--upload` is passed through `perf stream`.
- spec: REND-018 (new), TEST-012 (the new column).
- example: `crates/render/examples/upload_bench.rs`, the queue micro-benchmark (indicative).

Not in this change: the cap (1 MiB, unchanged), the manager's placement, any golden, pinned count or MESH_VERSION.
