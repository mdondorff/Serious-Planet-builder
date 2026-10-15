## Why

M4 item 7 (the residency manager with the share on pooled fixed-size slots; ADR 0031 point 4, ADR 0033 points 1-7 and 10; review item B). The scheduler (#127) decides what to request but counts capacity in tiles with a soft cap, keeps one GPU buffer per tile, and has no answer when the share is smaller than the residency set R. ADR 0033 gives terrain an explicit VRAM share counted in slots, a pool with a free list whose slots are reused only after the frames in flight retire, and ADR 0031 point 4 the order of loss. This change builds the accounting, the pools, the manager and the degradation, and puts the renderer's tile storage on pooled slots.

## What Changes

- streaming: `slots` (`SlotPool`, `SlotPools`: fixed capacity, lowest-free-first free list, a released slot is reusable `retire_frames` later; the small vertex pool for levels 0-2), `budget` (`Tier`, `PoolConfig`: a pure `allocated_bytes` model with the share as a ceiling, 2 GiB mid, 3 GiB high, 0.125 GiB reserved for the fixed items), `manager` (`degrade`, a pure function of `(R, D, capacity)`; `ResidencyManager` driving the `Streamer`).
- streaming: `Streamer` capacity is in slots and hard. A slot is reserved at dispatch and given back when the tile is evicted, cancelled or failed. A request with no free slot makes room by eviction (prefetch and tiles outside R first, least recently used; then, for draw requests only, R tiles farthest then finest first; never D with its ancestors or a root) or waits. Resident plus in-flight never exceed the capacity. `StreamConfig` gains `vertex_pool_slots` and `retire_frames`; `Streamer` exposes `slot_of`, `resolve`, `pools`.
- frame: `pool` (the vertex-format slot, 53,724 B at cells 32, and the pools of a tier's share).
- render: the terrain tile storage is pooled. Tiles live in slots of a few large vertex buffers (blocks of 256 slots, allocated on first use) and a small vertex pool for levels 0-2, no per-tile buffer. `upload` places tiles from the renderer's own free list, `upload_at` executes a placement chosen by the manager. Same vertex format, same shaders, same indices, same draws: no image changes. New errors `PoolFull`, `BadSlot`, `TooManyNodes`.
- spec: STRM-006 rewritten (terrain share, cap and order of loss, slot accounting, headroom), STRM-007 reworded (capacity in slots), REND-013 gains the pooled-slot scenarios.
- tests: slot accounting and share (tier A), degradation and the Small share scenario on fakes and on the real R and D (tier A), headroom pins on the ten standard views over the default world (tier A), pooled slots against the free-list and scattered placements and the allocation model on WARP (tier C).

Not in this change: the compact tile format and atlases (ADR 0033 point 4, a later change that swaps the main pool's format), indirect multi-draw, frame pipelining, the prefetch source, the upload cap (point 9) and the RAM budget (point 8), both still proposed; wiring the manager into an interactive loop (there is none yet; the `--stream` path keeps its one-shot use).

No `MESH_VERSION`, golden, plan snapshot or pinned count changes.
