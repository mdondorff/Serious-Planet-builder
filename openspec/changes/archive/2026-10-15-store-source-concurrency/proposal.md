## Why

M4 item 7b (SPIKE-05 follow-up). `StoreSource` keeps its decoded blocks in a `Mutex<Vec>` taken for every lattice read, so 20 threads generate eroded tiles over the store at 20-22 tiles/s against 111 on one thread (1,217 on 20 with an in-memory bake). A cold plan at altitude (orbit 6.1 s, high-2000km 4.9 s over the 16384 store, ad hoc) is the cost of decoding blocks: 33 ms per block, 188 distinct blocks (3.2 GB decoded) for the 52 nodes of the orbit plan. Measurements in `docs/learnings.md` (2026-10-15, 7b).

## What Changes

- cache: `StoreSource` reads without a shared lock on the hot path. Each thread keeps the block it read last (invalidated by an eviction epoch); the shared cache is a byte-budgeted LRU (default 512 MiB, ADR 0033 point 6's proposal for bake blocks, from 16 blocks of at most 16 MB) that is touched only when a thread changes block; a block being decoded is shared by every thread that wants it (one decode per block, not one per thread); a failed read is not kept.
- cache: block reads and decode are faster, bit for bit the same bytes: CRC-32C slicing-by-8 (same polynomial, same results), positional reads of a pack record without the file mutex, the planar-to-sample decode in one pass.
- streaming-cache spec: STRM-014.
- app: `store_blocks` measurement example (not product code).

Not in this change: key derivation (ADR 0018, 0023, 0026, 0027 unchanged, no ADR needed), the codec's payload layout, the frustum cut (7c). A cold plan at orbit still reads about 188 blocks that no 0.5 GB cache can hold; it gets faster by the cost per block only.
