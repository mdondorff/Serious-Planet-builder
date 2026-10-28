## Why
The horizon maps of the default viewer (a bake, no chunk stage, STRM-064) are covered in `planet_streaming` with a fake baker and by the real-store gate (never in CI). No CI test wired a world's own lattice, wanted set and baker without a stage (planner, 2026-10-28). It is the safety net for the parallel-bake change that follows.

## What Changes
1. Tier B test `horizon_world.rs`: a world with an in-memory bake and no chunk stage; `enable_horizon` on a `ManualSpawner`; `horizon_wanted` returns the level-11 tile's chunk, nearest first; `horizon_stats().baked > 0` and the tile's own map lands.

## Impact
- No spec change; the test cites STRM-064. Code: `crates/app/tests/horizon_world.rs` only.
