## Why
ADR 0071 (accepted 2026-10-20): coastal water is decided by the wet floor, the minimax elevation of a path from the ocean. The floor is level-free and costs 0.2 s (256 cells) to 9 min (16384 cells, 3 threads) (SPIKE-21). This change adds the channel and its store; the consumers follow in `wet-water`.

## What changes
- `planet_generators::wet_floor`: the three passes (per-block union-find with a virtual ocean node; one global pass over the terminal-merge edges; per-block bucketed priority flood), the integer codes `floor(2 e) + 24000` (0 ocean, 0xFFFF unreachable), `bound_code`, the serial reference, the in-memory `tiled_floors`, and the `FloorLattice` trait.
- `planet_cache::global_store`: record kind 6 (`Kind::GlobalWetFloor`), the block codec (`u16` low plane then high plane, LZ4), `GlobalWorld::floor_key` / `floor_record` (implementation id `cpu-wet-floor-v1+cpu-ocean-connectivity-v1`, keyed by the bake and the area threshold), liveness, `build_global` writing the missing floor blocks before the summary, `StoreWetFloorSource` (a `FloorLattice` over the store) and the floor records in `verify_global`.
- Nothing reads the channel yet: no golden, plan or frame changes. A store built before this change gets its floor blocks from the next `generate --global-dir` run.

Expected golden changes:
- none
